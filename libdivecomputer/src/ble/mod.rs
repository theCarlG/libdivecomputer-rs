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
            default_timeout: Duration::from_millis(1200),
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

impl BleTransport {
    async fn connect(mac_address: &str) -> Result<Self> {
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

        peripheral.connect().await?;
        peripheral.discover_services().await?;

        let (service, write_char, read_char) =
            Self::find_preferred_service_and_characteristics(&peripheral).await?;

        peripheral.subscribe(&read_char).await?;

        let (event_tx, event_rx) = mpsc::unbounded_channel::<BleEvent>();
        let notification_stream = peripheral.notifications().await?;

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
                peripheral,
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
        let known_uuids: Vec<Uuid> = KNOWN_SERVICES.iter().map(|(uuid, _)| *uuid).collect();
        let scan_filter = ScanFilter {
            services: known_uuids,
        };

        adapter.start_scan(scan_filter).await?;
        tokio::time::sleep(Duration::from_secs(5)).await;
        adapter.stop_scan().await?;

        let peripherals = adapter.peripherals().await?;
        for peripheral in peripherals {
            if let Some(props) = peripheral.properties().await?
                && props.address.to_string().to_lowercase() == mac_address.to_lowercase()
            {
                return Ok(peripheral);
            }
        }

        Err(LibError::BleDeviceNotFound(format!(
            "device {mac_address} not found after 5s scan"
        )))
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
    if !io.is_null() {
        let _transport = unsafe { Box::from_raw(io as *mut BleTransport) };
    }
    ffi::DC_STATUS_SUCCESS
}

extern "C" fn ble_read(
    io: *mut c_void,
    data: *mut c_void,
    size: usize,
    actual: *mut usize,
) -> ffi::dc_status_t {
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
}

extern "C" fn ble_write(
    io: *mut c_void,
    data: *const c_void,
    size: usize,
    actual: *mut usize,
) -> ffi::dc_status_t {
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
}

extern "C" fn ble_poll(io: *mut c_void, timeout: i32) -> ffi::dc_status_t {
    if io.is_null() {
        return ffi::DC_STATUS_IO;
    }

    let transport = unsafe { &*(io as *const BleTransport) };
    match transport.poll_blocking(Duration::from_millis(timeout as u64)) {
        Ok(true) => ffi::DC_STATUS_SUCCESS,
        Ok(false) => ffi::DC_STATUS_TIMEOUT,
        Err(_) => ffi::DC_STATUS_IO,
    }
}

extern "C" fn ble_set_timeout(io: *mut c_void, timeout: i32) -> ffi::dc_status_t {
    if io.is_null() {
        return ffi::DC_STATUS_IO;
    }

    let transport = unsafe { &*(io as *const BleTransport) };
    transport.set_timeout(Duration::from_millis(timeout as u64));
    ffi::DC_STATUS_SUCCESS
}

extern "C" fn ble_ioctl(
    io: *mut c_void,
    request: u32,
    data: *mut c_void,
    size: usize,
) -> ffi::dc_status_t {
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
}

/// Open a BLE iostream for the given MAC address.
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
        // Reclaim the transport to avoid a leak.
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
    pub static JAVAVM: std::sync::OnceLock<jni::JavaVM> = std::sync::OnceLock::new();

    std::thread_local! {
        pub static JNI_ENV: std::cell::RefCell<Option<jni::AttachGuard<'static>>> =
            std::cell::RefCell::new(None);
    }

    pub fn init(env: jni::JNIEnv) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let vm = env.get_java_vm()?;
        let _ = JAVAVM.set(vm);
        jni_utils::init(&env)?;
        btleplug::platform::init(&env)?;
        Ok(())
    }

    /// Attach the current thread to the JVM and return a guard that detaches on drop.
    /// Must be called on any spawned thread before using btleplug APIs.
    pub fn attach_current_thread(
    ) -> std::result::Result<jni::AttachGuard<'static>, Box<dyn std::error::Error>> {
        let vm = JAVAVM
            .get()
            .ok_or("JavaVM not initialized — call init() first")?;
        Ok(vm.attach_current_thread()?)
    }
}
