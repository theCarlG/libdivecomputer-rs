pub mod services;

use std::collections::VecDeque;
use std::ffi::c_void;
use std::ptr;
use std::time::Duration;

use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, Service,
    ValueNotification, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use libdivecomputer_sys as ffi;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::device::{ConnectionInfo, DeviceInfo};
use crate::error::{LibError, Result};
use crate::iostream::IoStream;
use crate::scanner::mac_string_to_u64;
use crate::transport::Transport;

use services::KNOWN_SERVICES;

type PendingReads = Vec<(usize, oneshot::Sender<std::result::Result<Vec<u8>, String>>)>;

/// Scan for BLE dive computer devices.
pub fn scan_ble(timeout: Duration) -> Result<Vec<DeviceInfo>> {
    #[cfg(target_os = "android")]
    let _jni_guard = android::attach_current_thread()
        .map_err(|e| LibError::DeviceError(format!("JNI attach failed: {e}")))?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| LibError::DeviceError(e.to_string()))?;

    rt.block_on(scan_ble_async(timeout))
}

async fn scan_ble_async(timeout: Duration) -> Result<Vec<DeviceInfo>> {
    let known_uuids: Vec<Uuid> = KNOWN_SERVICES.iter().map(|(uuid, _)| *uuid).collect();

    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let adapter = adapters
        .into_iter()
        .next()
        .ok_or(LibError::NoBluetoothAdapter)?;

    let scan_filter = ScanFilter {
        services: known_uuids.clone(),
    };

    adapter.start_scan(scan_filter).await?;

    let start = tokio::time::Instant::now();
    let mut devices = Vec::new();

    loop {
        let peripherals = adapter.peripherals().await?;

        for peripheral in peripherals {
            if let Ok(Some(props)) = peripheral.properties().await {
                for service_uuid in &props.services {
                    if let Some(idx) = known_uuids.iter().position(|u| u == service_uuid) {
                        let service_name = KNOWN_SERVICES[idx].1;
                        let peripheral_id = peripheral.id();
                        let address_string = peripheral_id.to_string();
                        let address = peripheral_id_to_address(&address_string).unwrap_or(0);

                        let device = DeviceInfo {
                            name: props
                                .local_name
                                .as_ref()
                                .map(|n| format!("{n} - {service_name}"))
                                .unwrap_or_else(|| service_name.to_string()),
                            transport: Transport::Ble,
                            connection: ConnectionInfo::Ble {
                                address,
                                address_string,
                                service_name: service_name.to_string(),
                                local_name: props.local_name.clone(),
                            },
                        };

                        if !devices.iter().any(|d: &DeviceInfo| d.name == device.name) {
                            devices.push(device);
                        }
                    }
                }
            }
        }

        if !devices.is_empty() || start.elapsed() >= timeout {
            break;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    adapter.stop_scan().await?;
    Ok(devices)
}

fn peripheral_id_to_address(id_str: &str) -> Option<u64> {
    // Linux/BlueZ: "hci0/dev_XX_XX_XX_XX_XX_XX"
    if id_str.contains("/dev_") {
        let parts: Vec<&str> = id_str.split('/').collect();
        if parts.len() == 2 {
            let mac_part = parts[1].strip_prefix("dev_")?;
            let mac_with_colons = mac_part.replace('_', ":");
            return mac_string_to_u64(&mac_with_colons);
        }
    }

    // Standard MAC: "AA:BB:CC:DD:EE:FF"
    if id_str.contains(':') {
        return mac_string_to_u64(id_str);
    }

    // Hyphen format: "AA-BB-CC-DD-EE-FF"
    if id_str.contains('-') {
        return mac_string_to_u64(&id_str.replace('-', ":"));
    }

    None
}

// --- BLE Transport (iostream implementation) ---

enum BleEvent {
    Write {
        data: Vec<u8>,
        response: oneshot::Sender<std::result::Result<usize, String>>,
    },
    Read {
        size: usize,
        response: oneshot::Sender<std::result::Result<Vec<u8>, String>>,
    },
    Poll {
        timeout: Duration,
        response: oneshot::Sender<bool>,
    },
    ReadCharacteristic {
        uuid: Uuid,
        response: oneshot::Sender<std::result::Result<Vec<u8>, String>>,
    },
    SetTimeout {
        timeout: Duration,
    },
    Disconnect,
}

struct PollManager {
    default_timeout: Duration,
    pending: Vec<(Instant, oneshot::Sender<bool>)>,
}

impl PollManager {
    fn new() -> Self {
        Self {
            pending: Vec::new(),
            // Generous default for the very first reads on a fresh BLE session,
            // before libdivecomputer's protocol layer narrows the timeout via
            // BleEvent::SetTimeout. On a never-bonded Shearwater the host BLE
            // stack often takes well over a second to deliver the first
            // notification, and the previous 1200ms default was too tight to
            // survive that initial window.
            default_timeout: Duration::from_secs(5),
        }
    }

    fn set_default_timeout(&mut self, timeout: Duration) {
        self.default_timeout = timeout;
    }

    fn add_poll(&mut self, timeout: Duration, response: oneshot::Sender<bool>) {
        let timeout = if timeout.as_millis() == 0 {
            self.default_timeout
        } else {
            timeout
        };
        self.pending.push((Instant::now() + timeout, response));
    }

    fn notify_all(&mut self) {
        for (_, response) in self.pending.drain(..) {
            let _ = response.send(true);
        }
    }

    fn check_timeouts(&mut self) {
        let now = Instant::now();
        let mut remaining = Vec::new();
        for (deadline, response) in self.pending.drain(..) {
            if now >= deadline {
                let _ = response.send(false);
            } else {
                remaining.push((deadline, response));
            }
        }
        self.pending = remaining;
    }
}

struct BleTransport {
    event_tx: mpsc::UnboundedSender<BleEvent>,
    device_name: String,
}

/// Maximum number of attempts at the connect/discover/subscribe portion of
/// opening a BLE session. The retry loop only re-runs the per-session work,
/// not the upfront 5-second peripheral scan, so the entire budget is spent on
/// giving the OS BLE stack room to settle on a fresh device (notably
/// Shearwater on first connect).
const BLE_CONNECT_MAX_ATTEMPTS: u32 = 5;

/// Backoff between session-open retry attempts.
const BLE_CONNECT_RETRY_DELAY: Duration = Duration::from_secs(3);

impl BleTransport {
    /// Find the peripheral once, then retry only the session-open portion.
    /// Rescanning on every retry (the previous behavior) ate ~5s of every
    /// attempt for no benefit.
    async fn connect(mac_address: &str) -> Result<Self> {
        log::debug!("ble: scanning for peripheral {mac_address}");

        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or(LibError::NoBluetoothAdapter)?;

        let peripheral = Self::find_peripheral(&adapter, mac_address).await?;
        let device_name = peripheral
            .properties()
            .await?
            .unwrap_or_default()
            .local_name
            .unwrap_or_else(|| "Unknown".to_string());

        log::debug!("ble: found peripheral {device_name:?}, opening session");

        let mut last_err = None;
        for attempt in 1..=BLE_CONNECT_MAX_ATTEMPTS {
            if attempt > 1 {
                log::debug!(
                    "ble: retry attempt {attempt}/{BLE_CONNECT_MAX_ATTEMPTS} after {:?}",
                    BLE_CONNECT_RETRY_DELAY
                );
                tokio::time::sleep(BLE_CONNECT_RETRY_DELAY).await;
            }
            match Self::open_session(&peripheral, device_name.clone(), attempt).await {
                Ok(transport) => return Ok(transport),
                Err(err) => {
                    log::warn!(
                        "ble: session open attempt {attempt}/{BLE_CONNECT_MAX_ATTEMPTS} failed: {err}"
                    );
                    last_err = Some(err);
                    // Make sure we are fully disconnected before the next
                    // attempt so the OS stack can fully reset its bond state.
                    let _ = peripheral.disconnect().await;
                }
            }
        }

        Err(last_err
            .unwrap_or_else(|| LibError::DeviceError("BLE session open failed".to_string())))
    }

    /// One pass at connect → discover services → subscribe → spawn event loop.
    /// Called from the retry loop in [`Self::connect`].
    async fn open_session(
        peripheral: &Peripheral,
        device_name: String,
        attempt: u32,
    ) -> Result<Self> {
        let started = Instant::now();
        log::debug!("ble: attempt {attempt}: connecting");
        peripheral.connect().await?;

        log::debug!("ble: attempt {attempt}: discovering services");
        peripheral.discover_services().await?;

        let (service, write_char, read_char) =
            Self::find_preferred_service_and_characteristics(peripheral).await?;

        // IMPORTANT: get the notification stream BEFORE enabling the GATT
        // subscription. If we subscribe first, any notification that arrives
        // in the window before we obtain the stream can be dropped on backends
        // whose internal channel buffers nothing for a zero-subscriber
        // broadcast — which is exactly the kind of single-packet loss that can
        // wedge a Shearwater first-sync handshake.
        let (event_tx, event_rx) = mpsc::unbounded_channel::<BleEvent>();
        let notification_stream = peripheral.notifications().await?;

        log::debug!("ble: attempt {attempt}: subscribing to notifications");
        peripheral.subscribe(&read_char).await?;

        // Let the CCCD descriptor write fully complete before the first
        // protocol command goes out. Cheap; only matters on the first session
        // for a given physical connection.
        tokio::time::sleep(Duration::from_millis(200)).await;

        log::debug!(
            "ble: attempt {attempt}: session ready in {:?}",
            started.elapsed()
        );

        // Clone what the spawned thread needs.
        let peripheral_owned = peripheral.clone();

        // Spawn the event loop on a dedicated thread with its own runtime.
        std::thread::spawn(move || {
            #[cfg(target_os = "android")]
            let _jni_guard = android::attach_current_thread()
                .expect("Failed to attach JNI to BLE event loop thread");

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create BLE runtime");

            rt.block_on(Self::event_loop(
                service,
                peripheral_owned,
                event_rx,
                notification_stream,
                write_char,
            ));
        });

        Ok(Self {
            event_tx,
            device_name,
        })
    }

    async fn event_loop(
        service: Service,
        peripheral: Peripheral,
        mut event_rx: mpsc::UnboundedReceiver<BleEvent>,
        mut notification_stream: impl StreamExt<Item = ValueNotification> + Unpin,
        write_char: Characteristic,
    ) {
        let mut received_packets: VecDeque<Vec<u8>> = VecDeque::new();
        let mut pending_reads: PendingReads = Vec::new();
        let mut poll_manager = PollManager::new();

        loop {
            tokio::select! {
                Some(ValueNotification { value, .. }) = notification_stream.next() => {
                    if let Some((size, response)) = pending_reads.pop() {
                        if value.len() <= size {
                            let _ = response.send(Ok(value));
                        } else {
                            let mut packet = value;
                            let remainder = packet.split_off(size);
                            received_packets.push_back(remainder);
                            let _ = response.send(Ok(packet));
                        }
                    } else {
                        received_packets.push_back(value);
                    }
                    poll_manager.notify_all();
                },

                Some(event) = event_rx.recv() => {
                    if !Self::handle_event(
                        event,
                        &service,
                        &peripheral,
                        &write_char,
                        &mut received_packets,
                        &mut pending_reads,
                        &mut poll_manager,
                    ).await {
                        break;
                    }
                }

                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    poll_manager.check_timeouts();
                }
            }
        }
    }

    async fn handle_event(
        event: BleEvent,
        service: &Service,
        peripheral: &Peripheral,
        write_char: &Characteristic,
        received_packets: &mut VecDeque<Vec<u8>>,
        pending_reads: &mut PendingReads,
        poll_manager: &mut PollManager,
    ) -> bool {
        match event {
            BleEvent::Write { data, response } => {
                let result = match peripheral
                    .write(write_char, &data, WriteType::WithoutResponse)
                    .await
                {
                    Ok(_) => Ok(data.len()),
                    Err(err) => Err(format!("Write error: {err}")),
                };
                let _ = response.send(result);
            }

            BleEvent::Read { size, response } => {
                if let Some(packet) = received_packets.pop_front() {
                    if packet.len() <= size {
                        let _ = response.send(Ok(packet));
                    } else {
                        let mut result = packet;
                        let remainder = result.split_off(size);
                        received_packets.push_front(remainder);
                        let _ = response.send(Ok(result));
                    }
                } else {
                    pending_reads.push((size, response));
                }
            }

            BleEvent::Poll { timeout, response } => {
                if !received_packets.is_empty() {
                    let _ = response.send(true);
                } else {
                    poll_manager.add_poll(timeout, response);
                }
            }

            BleEvent::SetTimeout { timeout } => {
                poll_manager.set_default_timeout(timeout);
            }

            BleEvent::ReadCharacteristic { uuid, response } => {
                if let Some(c) = service.characteristics.iter().find(|c| c.uuid == uuid) {
                    match peripheral.read(c).await {
                        Ok(data) => {
                            let _ = response.send(Ok(data));
                        }
                        Err(err) => {
                            let _ = response.send(Err(format!("Read characteristic error: {err}")));
                        }
                    }
                } else {
                    let _ = response.send(Err("Characteristic not found".to_string()));
                }
            }

            BleEvent::Disconnect => {
                let _ = peripheral.disconnect().await;
                return false;
            }
        }
        true
    }

    async fn find_peripheral(adapter: &Adapter, mac_address: &str) -> Result<Peripheral> {
        let target = mac_address.to_lowercase();

        // Tier 1: cached peripherals already known to this Manager session.
        // After the first sync of a process this is enough to avoid the
        // 5-second scan; on droidplug the GLOBAL_ADAPTER persists peripherals
        // across calls within the same process.
        if let Ok(peripherals) = adapter.peripherals().await {
            for peripheral in peripherals {
                if Self::peripheral_matches(&peripheral, &target).await {
                    log::debug!(
                        "ble: {mac_address} resolved via cached peripheral lookup (no scan)"
                    );
                    return Ok(peripheral);
                }
            }
        }

        // Tier 2: Android-only direct lookup by MAC. Wraps
        // BluetoothAdapter.getRemoteDevice(addr), which manufactures a
        // connectable peripheral handle without scanning and without needing
        // the device to be currently advertising. This is the path that
        // bypasses Shearwater's "stops advertising when it sees its previous
        // peer phone" radio behavior. Requires the local btleplug fork that
        // exposes `From<BDAddr>` for the droidplug PeripheralId.
        #[cfg(target_os = "android")]
        if let Ok(addr) = mac_address.parse::<btleplug::api::BDAddr>() {
            let id: btleplug::platform::PeripheralId = addr.into();
            match adapter.add_peripheral(&id).await {
                Ok(peripheral) => {
                    log::debug!(
                        "ble: {mac_address} resolved via add_peripheral (no scan, no advertising required)"
                    );
                    return Ok(peripheral);
                }
                Err(err) => {
                    log::debug!(
                        "ble: add_peripheral({mac_address}) failed: {err}; falling back to scan"
                    );
                }
            }
        }

        // Tier 3: existing 5-second active scan. Required for cold-start
        // discovery on backends that don't support direct add_peripheral
        // (BlueZ, CoreBluetooth) and as a safety net on Android.
        log::debug!("ble: cached lookup failed for {mac_address}, falling back to 5s active scan");
        let known_uuids: Vec<Uuid> = KNOWN_SERVICES.iter().map(|(uuid, _)| *uuid).collect();
        let scan_filter = ScanFilter {
            services: known_uuids,
        };
        adapter.start_scan(scan_filter).await?;
        tokio::time::sleep(Duration::from_secs(5)).await;
        adapter.stop_scan().await?;

        let peripherals = adapter.peripherals().await?;
        for peripheral in peripherals {
            if Self::peripheral_matches(&peripheral, &target).await {
                return Ok(peripheral);
            }
        }

        Err(LibError::BleDeviceNotFound(format!(
            "device {mac_address} not found after cached lookup and 5s scan"
        )))
    }

    /// Match a [`Peripheral`] against a lowercase target MAC/id string,
    /// trying both the platform peripheral id (needed on iOS where
    /// CoreBluetooth uses opaque UUIDs instead of MAC addresses) and the
    /// underlying BD address advertised in `properties()` (Linux/Android).
    async fn peripheral_matches(peripheral: &Peripheral, target: &str) -> bool {
        if peripheral.id().to_string().to_lowercase() == target {
            return true;
        }
        if let Ok(Some(props)) = peripheral.properties().await
            && props.address.to_string().to_lowercase() == target
        {
            return true;
        }
        false
    }

    async fn find_preferred_service_and_characteristics(
        peripheral: &Peripheral,
    ) -> Result<(Service, Characteristic, Characteristic)> {
        let services = peripheral.services();

        for (uuid, _name) in KNOWN_SERVICES {
            if let Some(service) = services.iter().find(|s| s.uuid == *uuid) {
                let mut write_char = None;
                let mut read_char = None;

                for characteristic in &service.characteristics {
                    let props = characteristic.properties;
                    if (props.contains(CharPropFlags::WRITE)
                        || props.contains(CharPropFlags::WRITE_WITHOUT_RESPONSE))
                        && write_char.is_none()
                    {
                        write_char = Some(characteristic.clone());
                    }
                    if (props.contains(CharPropFlags::NOTIFY)
                        || props.contains(CharPropFlags::INDICATE))
                        && read_char.is_none()
                    {
                        read_char = Some(characteristic.clone());
                    }
                }

                if let (Some(write), Some(read)) = (write_char, read_char) {
                    return Ok((service.clone(), write, read));
                }
            }
        }

        let discovered: Vec<String> = services.iter().map(|s| s.uuid.to_string()).collect();
        Err(LibError::BleServiceNotFound(format!(
            "no compatible GATT service found (discovered: [{}])",
            discovered.join(", ")
        )))
    }

    fn write_blocking(&self, data: &[u8]) -> Result<usize> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(BleEvent::Write {
                data: data.to_vec(),
                response: tx,
            })
            .map_err(|_| LibError::DeviceError("BLE event channel closed".to_string()))?;
        match rx.blocking_recv() {
            Ok(Ok(size)) => Ok(size),
            Ok(Err(err)) => Err(LibError::DeviceError(err)),
            Err(_) => Err(LibError::DeviceError("BLE channel closed".to_string())),
        }
    }

    fn read_blocking(&self, buffer: &mut [u8]) -> Result<usize> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(BleEvent::Read {
                size: buffer.len(),
                response: tx,
            })
            .map_err(|_| LibError::DeviceError("BLE event channel closed".to_string()))?;
        match rx.blocking_recv() {
            Ok(Ok(data)) => {
                let n = std::cmp::min(data.len(), buffer.len());
                buffer[..n].copy_from_slice(&data[..n]);
                Ok(n)
            }
            Ok(Err(err)) => Err(LibError::DeviceError(err)),
            Err(_) => Err(LibError::DeviceError("BLE channel closed".to_string())),
        }
    }

    fn read_characteristic_blocking(&self, uuid: Uuid, buffer: &mut [u8]) -> Result<usize> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(BleEvent::ReadCharacteristic { uuid, response: tx })
            .map_err(|_| LibError::DeviceError("BLE event channel closed".to_string()))?;
        match rx.blocking_recv() {
            Ok(Ok(data)) => {
                let n = std::cmp::min(data.len(), buffer.len());
                buffer[..n].copy_from_slice(&data[..n]);
                Ok(n)
            }
            Ok(Err(err)) => Err(LibError::DeviceError(err)),
            Err(_) => Err(LibError::DeviceError("BLE channel closed".to_string())),
        }
    }

    fn poll_blocking(&self, timeout: Duration) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .send(BleEvent::Poll {
                timeout,
                response: tx,
            })
            .map_err(|_| LibError::DeviceError("BLE event channel closed".to_string()))?;
        rx.blocking_recv()
            .map_err(|_| LibError::DeviceError("BLE channel closed".to_string()))
    }

    fn set_timeout(&self, timeout: Duration) {
        let _ = self.event_tx.send(BleEvent::SetTimeout { timeout });
    }

    fn get_name(&self) -> &str {
        &self.device_name
    }
}

impl Drop for BleTransport {
    fn drop(&mut self) {
        let _ = self.event_tx.send(BleEvent::Disconnect);
    }
}

// --- FFI callback functions ---

extern "C" fn ble_close(io: *mut c_void) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !io.is_null() {
            let _transport = unsafe { Box::from_raw(io as *mut BleTransport) };
        }
        ffi::DC_STATUS_SUCCESS
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

extern "C" fn ble_read(
    io: *mut c_void,
    data: *mut c_void,
    size: usize,
    actual: *mut usize,
) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() || data.is_null() {
            return ffi::DC_STATUS_IO;
        }

        let transport = unsafe { &*(io as *const BleTransport) };
        let buffer = unsafe { std::slice::from_raw_parts_mut(data as *mut u8, size) };

        match transport.read_blocking(buffer) {
            Ok(bytes_read) => {
                if !actual.is_null() {
                    unsafe { *actual = bytes_read };
                }
                ffi::DC_STATUS_SUCCESS
            }
            Err(_) => ffi::DC_STATUS_IO,
        }
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

extern "C" fn ble_write(
    io: *mut c_void,
    data: *const c_void,
    size: usize,
    actual: *mut usize,
) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() || data.is_null() {
            return ffi::DC_STATUS_IO;
        }

        let transport = unsafe { &*(io as *const BleTransport) };
        let data_slice = unsafe { std::slice::from_raw_parts(data as *const u8, size) };

        match transport.write_blocking(data_slice) {
            Ok(bytes_written) => {
                if !actual.is_null() {
                    unsafe { *actual = bytes_written };
                }
                ffi::DC_STATUS_SUCCESS
            }
            Err(_) => ffi::DC_STATUS_IO,
        }
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

extern "C" fn ble_poll(io: *mut c_void, timeout: i32) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() {
            return ffi::DC_STATUS_IO;
        }

        let transport = unsafe { &*(io as *const BleTransport) };
        let millis = if timeout < 0 { 0 } else { timeout as u64 };
        match transport.poll_blocking(Duration::from_millis(millis)) {
            Ok(true) => ffi::DC_STATUS_SUCCESS,
            Ok(false) => ffi::DC_STATUS_TIMEOUT,
            Err(_) => ffi::DC_STATUS_IO,
        }
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

extern "C" fn ble_set_timeout(io: *mut c_void, timeout: i32) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() {
            return ffi::DC_STATUS_IO;
        }

        let transport = unsafe { &*(io as *const BleTransport) };
        let millis = if timeout < 0 { 0 } else { timeout as u64 };
        transport.set_timeout(Duration::from_millis(millis));
        ffi::DC_STATUS_SUCCESS
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

extern "C" fn ble_ioctl(
    io: *mut c_void,
    request: u32,
    data: *mut c_void,
    size: usize,
) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if io.is_null() {
            return ffi::DC_STATUS_IO;
        }

        let transport = unsafe { &*(io as *const BleTransport) };

        match request {
            ffi::DC_IOCTL_BLE_GET_NAME => {
                if data.is_null() {
                    return ffi::DC_STATUS_IO;
                }
                let name = transport.get_name();
                let buffer = unsafe { std::slice::from_raw_parts_mut(data as *mut u8, size) };
                let name_bytes = name.as_bytes();
                let n = std::cmp::min(name_bytes.len(), buffer.len() - 1);
                buffer[..n].copy_from_slice(&name_bytes[..n]);
                buffer[n] = 0;
                ffi::DC_STATUS_SUCCESS
            }
            ffi::DC_IOCTL_BLE_CHARACTERISTIC_READ => {
                if data.is_null() || size < 16 {
                    return ffi::DC_STATUS_INVALIDARGS;
                }
                unsafe {
                    let data_ptr = data as *mut u8;
                    let uuid_bytes = std::slice::from_raw_parts(data_ptr, 16);
                    let Ok(uuid) = Uuid::from_slice(uuid_bytes) else {
                        return ffi::DC_STATUS_INVALIDARGS;
                    };
                    let readsize = size - 16;
                    let buf = std::slice::from_raw_parts_mut(data_ptr.add(16), readsize);

                    if transport.read_characteristic_blocking(uuid, buf).is_err() {
                        return ffi::DC_STATUS_INVALIDARGS;
                    }
                }
                ffi::DC_STATUS_SUCCESS
            }
            _ => ffi::DC_STATUS_UNSUPPORTED,
        }
    }));
    match result {
        Ok(status) => status,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

/// Open a BLE iostream for the given MAC address. The retry-on-first-connect
/// behavior lives inside [`BleTransport::connect`] so that retries don't waste
/// time re-running the upfront peripheral scan.
pub fn ble_iostream_open(ctx: &crate::context::Context, mac_address: &str) -> Result<IoStream> {
    #[cfg(target_os = "android")]
    let _jni_guard = android::attach_current_thread()
        .map_err(|e| LibError::DeviceError(format!("JNI attach failed: {e}")))?;

    // Create a temporary runtime for the async connection.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| LibError::DeviceError(e.to_string()))?;

    let addr = mac_address.strip_prefix("LE:").unwrap_or(mac_address);

    let transport = rt.block_on(BleTransport::connect(addr))?;
    let io_ptr = Box::into_raw(Box::new(transport)) as *mut c_void;

    let callbacks = ffi::dc_custom_cbs_t {
        set_timeout: Some(ble_set_timeout),
        set_break: None,
        set_dtr: None,
        set_rts: None,
        get_lines: None,
        get_available: None,
        configure: None,
        poll: Some(ble_poll),
        read: Some(ble_read),
        write: Some(ble_write),
        ioctl: Some(ble_ioctl),
        flush: None,
        purge: None,
        sleep: None,
        close: Some(ble_close),
    };

    let mut iostream_ptr = ptr::null_mut();
    let status = unsafe {
        ffi::dc_custom_open(
            &mut iostream_ptr,
            ctx.ptr(),
            ffi::DC_TRANSPORT_BLE,
            &callbacks,
            io_ptr,
        )
    };

    if status != ffi::DC_STATUS_SUCCESS {
        unsafe { drop(Box::from_raw(io_ptr as *mut BleTransport)) };
        return Err(LibError::status_with_context(
            status,
            "failed to open BLE iostream",
        ));
    }

    Ok(IoStream::from_raw(iostream_ptr))
}

#[cfg(target_os = "android")]
pub mod android {
    pub use crate::android::*;
}
