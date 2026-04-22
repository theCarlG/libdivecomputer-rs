/// Known BLE service and characteristic UUIDs for supported dive computers.
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
use tracing::instrument;
use uuid::Uuid;

use crate::device::{ConnectionInfo, DeviceInfo};
use crate::error::{LibError, Result};
use crate::iostream::IoStream;
use crate::scanner::mac_string_to_u64;
use crate::transport::Transport;

use services::KNOWN_SERVICES;
#[cfg(target_os = "android")]
use services::use_random_address;

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

#[instrument(fields(timeout_ms = timeout.as_millis() as u64))]
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
    // Declared before `worker` so the sender's Drop runs first when this
    // struct is dropped: closing the channel is the backstop that lets the
    // worker exit even if the `Disconnect` signal in `impl Drop` was lost.
    event_tx: mpsc::Sender<BleEvent>,
    device_name: String,
    worker: Option<std::thread::JoinHandle<()>>,
}

/// Capacity of the FFI-to-worker event channel. Each sync FFI call is
/// strictly request-reply (send one event, block on oneshot), so from the
/// caller's side at most one event is in flight. 8 is headroom for
/// `set_timeout` or stray `Disconnect` events that could pile up during
/// shutdown.
const BLE_EVENT_CHANNEL_CAPACITY: usize = 8;

/// Render a `catch_unwind` panic payload as a human-readable string. Panics
/// that cross the FFI boundary come back as `Box<dyn Any + Send>` whose payload
/// is almost always a `&'static str` (bare `panic!("msg")`) or `String`
/// (formatted). Anything else — typically an externally constructed payload —
/// we surface as a placeholder rather than silently swallowing.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

/// Maximum number of attempts at the connect/discover/subscribe portion of
/// opening a BLE session. The retry loop only re-runs the per-session work,
/// not the upfront 5-second peripheral scan, so the entire budget is spent on
/// giving the OS BLE stack room to settle on a fresh device (notably
/// Shearwater on first connect).
const BLE_CONNECT_MAX_ATTEMPTS: u32 = 5;

/// Backoff between session-open retry attempts.
const BLE_CONNECT_RETRY_DELAY: Duration = Duration::from_secs(3);

/// Cap on unread notifications queued in the event loop. Under normal operation
/// reads drain the queue faster than notifications arrive, so this is purely a
/// safety net against runaway memory growth if the protocol layer stops
/// consuming for any reason. With typical BLE MTU (~20-244 bytes), 1024 packets
/// is at most a few hundred KB.
const MAX_BUFFERED_PACKETS: usize = 1024;

/// Push a packet onto the buffer, dropping the oldest if the cap is hit. The
/// drop is loud on purpose — in a well-behaved session we never hit this.
fn buffer_push(buffer: &mut VecDeque<Vec<u8>>, packet: Vec<u8>) {
    if buffer.len() >= MAX_BUFFERED_PACKETS {
        tracing::warn!(
            cap = MAX_BUFFERED_PACKETS,
            "ble: received-packet buffer at cap; dropping oldest packet"
        );
        buffer.pop_front();
    }
    buffer.push_back(packet);
}

impl BleTransport {
    /// Find the peripheral once, then retry only the session-open portion.
    /// Rescanning on every retry (the previous behavior) ate ~5s of every
    /// attempt for no benefit.
    #[instrument(skip_all, fields(mac_address = %mac_address, service_name = %service_name))]
    async fn connect(mac_address: &str, service_name: &str) -> Result<Self> {
        tracing::debug!("ble: scanning for peripheral");

        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or(LibError::NoBluetoothAdapter)?;

        let peripheral = Self::find_peripheral(&adapter, mac_address, service_name).await?;
        let device_name = peripheral
            .properties()
            .await?
            .unwrap_or_default()
            .local_name
            .unwrap_or_else(|| "Unknown".to_string());

        tracing::debug!(device_name = %device_name, "ble: found peripheral, opening session");

        let mut last_err = None;
        for attempt in 1..=BLE_CONNECT_MAX_ATTEMPTS {
            if attempt > 1 {
                tracing::debug!(
                    attempt,
                    max_attempts = BLE_CONNECT_MAX_ATTEMPTS,
                    delay_ms = BLE_CONNECT_RETRY_DELAY.as_millis() as u64,
                    "ble: retrying session open"
                );
                tokio::time::sleep(BLE_CONNECT_RETRY_DELAY).await;
            }
            match Self::open_session(&peripheral, device_name.clone(), attempt).await {
                Ok(transport) => return Ok(transport),
                Err(err) => {
                    tracing::warn!(
                        attempt,
                        max_attempts = BLE_CONNECT_MAX_ATTEMPTS,
                        error = %err,
                        "ble: session open attempt failed"
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
    #[instrument(skip(peripheral), fields(device_name = %device_name, attempt = attempt))]
    async fn open_session(
        peripheral: &Peripheral,
        device_name: String,
        attempt: u32,
    ) -> Result<Self> {
        let started = Instant::now();
        tracing::debug!("ble: connecting");
        peripheral.connect().await?;

        tracing::debug!("ble: discovering services");
        peripheral.discover_services().await?;

        let (service, write_char, read_char) =
            Self::find_preferred_service_and_characteristics(peripheral).await?;

        // IMPORTANT: get the notification stream BEFORE enabling the GATT
        // subscription. If we subscribe first, any notification that arrives
        // in the window before we obtain the stream can be dropped on backends
        // whose internal channel buffers nothing for a zero-subscriber
        // broadcast — which is exactly the kind of single-packet loss that can
        // wedge a Shearwater first-sync handshake.
        let (event_tx, event_rx) = mpsc::channel::<BleEvent>(BLE_EVENT_CHANNEL_CAPACITY);
        let notification_stream = peripheral.notifications().await?;

        tracing::debug!("ble: subscribing to notifications");
        peripheral.subscribe(&read_char).await?;

        // Let the CCCD descriptor write fully complete before the first
        // protocol command goes out. Cheap; only matters on the first session
        // for a given physical connection.
        tokio::time::sleep(Duration::from_millis(200)).await;

        tracing::debug!(
            elapsed_ms = started.elapsed().as_millis() as u64,
            "ble: session ready"
        );

        // Clone what the spawned thread needs.
        let peripheral_owned = peripheral.clone();

        // Startup handshake: the spawned thread must confirm it attached JNI
        // and built its runtime before we hand a `BleTransport` back to the
        // caller. Without this, a JNI/tokio failure inside the thread would
        // leave `event_tx` orphaned and every subsequent FFI call would block
        // on a oneshot whose sender was dropped.
        let (startup_tx, startup_rx) = oneshot::channel::<Result<()>>();

        // Capture the JoinHandle so Drop can join the worker on shutdown.
        // The outer `catch_unwind` turns a panic inside the worker into a
        // logged error + clean exit instead of a silent thread death that
        // wedges the next FFI call. If the panic happens before startup is
        // signalled, the unwinding drops `startup_tx`, which closes the
        // channel and surfaces as the "exited before signalling startup"
        // error path below.
        let worker = std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                #[cfg(target_os = "android")]
                let _jni_guard = match android::attach_current_thread() {
                    Ok(g) => g,
                    Err(e) => {
                        let _ = startup_tx.send(Err(LibError::DeviceError(format!(
                            "JNI attach failed: {e}"
                        ))));
                        return;
                    }
                };

                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,
                    Err(e) => {
                        let _ = startup_tx.send(Err(LibError::DeviceError(format!(
                            "failed to build BLE runtime: {e}"
                        ))));
                        return;
                    }
                };

                if startup_tx.send(Ok(())).is_err() {
                    // Parent already gave up waiting — nothing more to do.
                    return;
                }

                rt.block_on(Self::event_loop(
                    service,
                    peripheral_owned,
                    event_rx,
                    notification_stream,
                    write_char,
                ));
            }));

            if let Err(payload) = result {
                tracing::error!(
                    panic = %panic_message(payload.as_ref()),
                    "ble: worker thread panicked"
                );
            }
        });

        match startup_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(LibError::DeviceError(
                    "BLE event loop thread exited before signalling startup".to_string(),
                ));
            }
        }

        Ok(Self {
            event_tx,
            device_name,
            worker: Some(worker),
        })
    }

    #[instrument(skip_all, fields(peripheral_id = %peripheral.id()))]
    async fn event_loop(
        service: Service,
        peripheral: Peripheral,
        mut event_rx: mpsc::Receiver<BleEvent>,
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
                            buffer_push(&mut received_packets, remainder);
                            let _ = response.send(Ok(packet));
                        }
                    } else {
                        buffer_push(&mut received_packets, value);
                    }
                    poll_manager.notify_all();
                },

                event = event_rx.recv() => {
                    // `None` means the parent `BleTransport` was dropped
                    // without sending `Disconnect` — treat channel close as
                    // an implicit shutdown. Without this explicit branch the
                    // `tokio::select!` arm would silently skip on `None` and
                    // the loop would spin on the 10 ms sleep branch forever.
                    let Some(event) = event else { break };
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

    #[instrument(skip(adapter), fields(mac_address = %mac_address, service_name = %service_name))]
    async fn find_peripheral(
        adapter: &Adapter,
        mac_address: &str,
        service_name: &str,
    ) -> Result<Peripheral> {
        let target = mac_address.to_lowercase();
        let _ = service_name; // only read on Android below; silence warnings elsewhere

        // Tier 1: cached peripherals already known to this Manager session.
        // After the first sync of a process this is enough to avoid the
        // 5-second scan; on droidplug the GLOBAL_ADAPTER persists peripherals
        // across calls within the same process.
        if let Ok(peripherals) = adapter.peripherals().await {
            for peripheral in peripherals {
                if Self::peripheral_matches(&peripheral, &target).await {
                    tracing::debug!("ble: resolved via cached peripheral lookup (no scan)");
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
        //
        // For devices that advertise with a random static address
        // (Shearwater/Garmin — see [`use_random_address`]) go through the
        // address-type aware API so Android's `BluetoothAdapter.getRemoteLeDevice`
        // is called with ADDRESS_TYPE_RANDOM. This mirrors Subsurface's
        // `setRemoteAddressType(RandomAddress)` workaround in
        // `core/qt-ble.cpp` and prevents `connectGatt()` from racing service
        // discovery with the wrong link-layer address type.
        #[cfg(target_os = "android")]
        if let Ok(addr) = mac_address.parse::<btleplug::api::BDAddr>() {
            let id: btleplug::platform::PeripheralId = addr.into();
            let result = if use_random_address(service_name) {
                tracing::debug!(
                    "ble: using ADDRESS_TYPE_RANDOM per Subsurface's use_random_address()"
                );
                adapter
                    .add_peripheral_with_address_type(&id, btleplug::api::AddressType::Random)
                    .await
            } else {
                adapter.add_peripheral(&id).await
            };
            match result {
                Ok(peripheral) => {
                    tracing::debug!(
                        "ble: resolved via add_peripheral (no scan, no advertising required)"
                    );
                    return Ok(peripheral);
                }
                Err(err) => {
                    tracing::debug!(
                        error = %err,
                        "ble: add_peripheral failed; falling back to scan"
                    );
                }
            }
        }

        // Tier 3: existing 5-second active scan. Required for cold-start
        // discovery on backends that don't support direct add_peripheral
        // (BlueZ, CoreBluetooth) and as a safety net on Android.
        tracing::debug!("ble: cached lookup failed, falling back to 5s active scan");
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

    #[instrument(skip_all)]
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

    /// Generic request/reply over the event channel: build an event that
    /// carries a `oneshot::Sender` for the reply, send it to the worker, and
    /// block on the response. Collapses the three failure axes (channel
    /// closed on send, channel closed on recv, worker-side error) into a
    /// single `LibError::DeviceError`.
    ///
    /// `BleEvent::Poll` doesn't fit this shape because its reply is `bool`
    /// rather than `Result<_, String>`, so `poll_blocking` stays custom.
    fn request<R, F>(&self, make_event: F) -> Result<R>
    where
        F: FnOnce(oneshot::Sender<std::result::Result<R, String>>) -> BleEvent,
    {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .blocking_send(make_event(tx))
            .map_err(|_| LibError::DeviceError("BLE event channel closed".to_string()))?;
        match rx.blocking_recv() {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(err)) => Err(LibError::DeviceError(err)),
            Err(_) => Err(LibError::DeviceError("BLE channel closed".to_string())),
        }
    }

    fn write_blocking(&self, data: &[u8]) -> Result<usize> {
        self.request(|response| BleEvent::Write {
            data: data.to_vec(),
            response,
        })
    }

    fn read_blocking(&self, buffer: &mut [u8]) -> Result<usize> {
        let data = self.request(|response| BleEvent::Read {
            size: buffer.len(),
            response,
        })?;
        let n = std::cmp::min(data.len(), buffer.len());
        buffer[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }

    fn read_characteristic_blocking(&self, uuid: Uuid, buffer: &mut [u8]) -> Result<usize> {
        let data = self.request(|response| BleEvent::ReadCharacteristic { uuid, response })?;
        let n = std::cmp::min(data.len(), buffer.len());
        buffer[..n].copy_from_slice(&data[..n]);
        Ok(n)
    }

    fn poll_blocking(&self, timeout: Duration) -> Result<bool> {
        let (tx, rx) = oneshot::channel();
        self.event_tx
            .blocking_send(BleEvent::Poll {
                timeout,
                response: tx,
            })
            .map_err(|_| LibError::DeviceError("BLE event channel closed".to_string()))?;
        rx.blocking_recv()
            .map_err(|_| LibError::DeviceError("BLE channel closed".to_string()))
    }

    fn set_timeout(&self, timeout: Duration) {
        let _ = self.event_tx.blocking_send(BleEvent::SetTimeout { timeout });
    }

    fn get_name(&self) -> &str {
        &self.device_name
    }
}

impl Drop for BleTransport {
    fn drop(&mut self) {
        // Graceful shutdown: the worker handles `Disconnect` by returning from
        // the event loop. `try_send` because we must not block Drop on a full
        // bounded channel — if the send fails (full or receiver gone), the
        // worker exits via the channel-close branch we added in `event_loop`
        // once `event_tx` drops at the end of Drop.
        let _ = self.event_tx.try_send(BleEvent::Disconnect);
        if let Some(worker) = self.worker.take()
            && let Err(payload) = worker.join()
        {
            tracing::error!(
                panic = %panic_message(payload.as_ref()),
                "ble: worker thread panicked during shutdown"
            );
        }
    }
}

// --- FFI callback functions ---

extern "C" fn ble_close(io: *mut c_void) -> ffi::dc_status_t {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if !io.is_null() {
            // SAFETY: libdivecomputer invokes this close callback exactly once
            // per successful open, passing back the same `userdata` pointer we
            // gave to `dc_custom_open` via `Box::into_raw(Box::new(BleTransport))`.
            // `Box::from_raw` reclaims that unique allocation and drops it,
            // which runs `BleTransport::Drop` to signal the worker thread.
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
/// behavior lives inside `BleTransport::connect` so that retries don't waste
/// time re-running the upfront peripheral scan.
///
/// `service_name` is the stored service name from [`services::KNOWN_SERVICES`]
/// and is used to pick the LE address type on Android — see
/// [`services::use_random_address`].
#[instrument(skip(ctx), fields(mac_address = %mac_address, service_name = %service_name))]
pub fn ble_iostream_open(
    ctx: &crate::context::Context,
    mac_address: &str,
    service_name: &str,
) -> Result<IoStream> {
    #[cfg(target_os = "android")]
    let _jni_guard = android::attach_current_thread()
        .map_err(|e| LibError::DeviceError(format!("JNI attach failed: {e}")))?;

    // Create a temporary runtime for the async connection.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| LibError::DeviceError(e.to_string()))?;

    let addr = mac_address.strip_prefix("LE:").unwrap_or(mac_address);

    let transport = rt.block_on(BleTransport::connect(addr, service_name))?;
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
        // SAFETY: `dc_custom_open` does not retain `userdata` on non-success
        // status, so the Box we handed over is still the unique owner. The
        // pointer was produced by `Box::into_raw(Box::new(BleTransport { ... }))`
        // earlier in this function with the same type, so reclaiming via
        // `Box::from_raw` reconstructs the original allocation.
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
