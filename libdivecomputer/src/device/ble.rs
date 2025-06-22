/// This module is a altered Rust version of the BLE code found in Subsurface
/// https://github.com/subsurface/subsurface/blob/b46b3f5a7912658f62a8f2ab72892cbab3e640b4/core/qt-ble.cpp
///
use std::collections::VecDeque;
use std::ffi::{CStr, c_char, c_void};
use std::ptr;
use std::time::Duration;

use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, Service,
    ValueNotification, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral, PeripheralId};
use libdivecomputer_sys as ffi;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tokio_stream::StreamExt;
use uuid::{Uuid, uuid};

use crate::device::DeviceTransport;
use crate::error;

pub use ffi::{dc_context_t, dc_custom_cbs_t, dc_iostream_t, dc_status_t};

const KNOWN_SERVICES: &[(Uuid, &str)] = &[
    (
        uuid!("0000fefb-0000-1000-8000-00805f9b34fb"),
        "Heinrichs-Weikamp (Telit/Stollmann)",
    ),
    (
        uuid!("2456e1b9-26e2-8f83-e744-f34f01e9d701"),
        "Heinrichs-Weikamp (U-Blox)",
    ),
    (
        uuid!("544e326b-5b72-c6b0-1c46-41c1bc448118"),
        "Mares BlueLink Pro",
    ),
    (
        uuid!("98ae7120-e62e-11e3-badd-0002a5d5c51b"),
        "Suunto (EON Steel/Core, G5)",
    ),
    (
        uuid!("cb3c4555-d670-4670-bc20-b61dbc851e9a"),
        "Pelagic (i770R, i200C, Pro Plus X, Geo 4.0)",
    ),
    (
        uuid!("ca7b0001-f785-4c38-b599-c7c5fbadb034"),
        "Pelagic (i330R, DSX)",
    ),
    (
        uuid!("fdcdeaaa-295d-470e-bf15-04217b7aa0a0"),
        "ScubaPro (G2, G3)",
    ),
    (
        uuid!("fe25c237-0ece-443c-b0aa-e02033e7029d"),
        "Shearwater (Perdix/Teric/Peregrine/Tern)",
    ),
    (uuid!("0000fcef-0000-1000-8000-00805f9b34fb"), "Divesoft"),
    (uuid!("6e400001-b5a3-f393-e0a9-e50e24dc10b8"), "Cressi"),
    (
        uuid!("6e400001-b5a3-f393-e0a9-e50e24dcca9e"),
        "Nordic Semi UART",
    ),
    (
        uuid!("00000001-8c3b-4f2c-a59e-8c08224f3253"),
        "Halcyon Symbios",
    ),
];

// BLE communication commands
#[derive(Debug)]
enum BleEvent {
    Write {
        data: Vec<u8>,
        response: oneshot::Sender<Result<usize, String>>,
    },
    Read {
        size: usize,
        response: oneshot::Sender<Result<Vec<u8>, String>>,
    },
    Poll {
        timeout: Duration,
        response: oneshot::Sender<bool>,
    },

    ReadCharacteristic {
        uuid: Uuid,
        response: oneshot::Sender<Result<Vec<u8>, String>>,
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

    pub fn set_default_timeout(&mut self, timeout: Duration) {
        self.default_timeout = timeout;
    }

    fn add_poll(&mut self, timeout: Duration, response: oneshot::Sender<bool>) {
        let timeout = if timeout.as_millis() == 0 {
            self.default_timeout
        } else {
            timeout
        };
        let deadline = Instant::now() + timeout;
        self.pending.push((deadline, response));
    }

    /// Notify all pending polls that data is available
    fn notify_all(&mut self) {
        for (_, response) in self.pending.drain(..) {
            let _ = response.send(true);
        }
    }

    /// Check for expired polls and notify them
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

// Main BLE transport structure
pub(crate) struct BleTransport {
    event_tx: mpsc::UnboundedSender<BleEvent>,
    device_name: String,
    #[expect(dead_code)]
    runtime_handle: tokio::runtime::Handle,
}

impl BleTransport {
    pub fn scan(duration: Duration) -> error::Result<Vec<DeviceTransport>> {
        let mut devices = Vec::new();

        let known_uuids: Vec<Uuid> = KNOWN_SERVICES
            .iter()
            .filter_map(|(uuid, _)| Some(*uuid))
            .collect();

        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(_) => return Err(error::LibError::status(ffi::DC_STATUS_IO)),
        };

        rt.block_on(async move {
            let manager = Manager::new().await?;
            let adapters = manager.adapters().await?;
            let adapter = adapters
                .into_iter()
                .next()
                .ok_or(btleplug::Error::Other("No Bluetooth adapter found".into()))?;

            let scan_filter = ScanFilter {
                services: known_uuids.clone(),
            };

            adapter.start_scan(scan_filter).await?;

            tokio::time::sleep(duration).await;

            let peripherals = adapter.peripherals().await?;

            for peripheral in peripherals {
                if let Ok(Some(props)) = peripheral.properties().await {
                    let mut service_name = None;

                    for service_uuid in &props.services {
                        if let Some(idx) = known_uuids.iter().position(|&u| u == *service_uuid) {
                            service_name = Some(KNOWN_SERVICES[idx].1);
                            break;
                        }
                    }

                    let device_name = if let Some(name) = service_name {
                        if let Some(local_name) = props.local_name {
                            format!("{} - {}", local_name, name)
                        } else {
                            name.to_string()
                        }
                    } else {
                        // Skip unknown devices;
                        continue;
                    };

                    let peripheral_id = peripheral.id();
                    let address_string = peripheral_id.to_string();

                    let address = peripheral_id_to_address(&peripheral_id)
                        .ok_or(btleplug::Error::Other("invalid peripheral id".into()))?;

                    devices.push(DeviceTransport::Ble {
                        address,
                        name: device_name,
                        address_string,
                    });
                }
            }

            adapter.stop_scan().await?;

            Ok(devices)
        })
        .map_err(|err| crate::error::LibError::Btleplug(err))
    }

    pub async fn connect(
        mac_address: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or("No Bluetooth adapter found")?;

        let peripheral = Self::find_peripheral(&adapter, mac_address).await?;
        let device_name = peripheral
            .clone()
            .properties()
            .await?
            .unwrap_or_default()
            .local_name
            .unwrap_or_else(|| "Unknown".to_string())
            .clone();

        peripheral.connect().await?;
        peripheral.discover_services().await?;

        let (service, write_char, read_char) =
            Self::find_preferred_service_and_characteristics(&peripheral).await?;

        peripheral.subscribe(&read_char).await?;

        let (event_tx, event_rx) = mpsc::unbounded_channel::<BleEvent>();
        let notification_stream = peripheral.notifications().await?;

        tokio::spawn(Self::event_loop(
            service,
            peripheral,
            event_rx,
            notification_stream,
            write_char,
        ));

        Ok(Self {
            event_tx,
            device_name,
            runtime_handle: tokio::runtime::Handle::current(),
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
        let mut pending_reads: Vec<(usize, oneshot::Sender<Result<Vec<u8>, String>>)> = Vec::new();
        let mut poll_manager = PollManager::new();

        loop {
            tokio::select! {
                Some(ValueNotification{value, .. }) = notification_stream.next() => {
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
                        &mut poll_manager
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
        pending_reads: &mut Vec<(usize, oneshot::Sender<Result<Vec<u8>, String>>)>,
        poll_manager: &mut PollManager,
    ) -> bool {
        match event {
            BleEvent::Write { data, response } => {
                let result = match peripheral
                    .write(&write_char, &data, WriteType::WithoutResponse)
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
                if let Some(char) = service.characteristics.iter().find(|c| c.uuid == uuid) {
                    match peripheral.read(char).await {
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

    async fn find_peripheral(
        adapter: &Adapter,
        mac_address: &str,
    ) -> Result<Peripheral, Box<dyn std::error::Error + Send + Sync>> {
        adapter.start_scan(ScanFilter::default()).await?;
        tokio::time::sleep(Duration::from_secs(5)).await;
        adapter.stop_scan().await?;

        let peripherals = adapter.peripherals().await?;
        for peripheral in peripherals {
            if let Some(props) = peripheral.properties().await? {
                if props.address.to_string().to_lowercase() == mac_address.to_lowercase() {
                    return Ok(peripheral);
                }
            }
        }

        Err(format!("Device {mac_address} not found").into())
    }

    async fn find_preferred_service_and_characteristics(
        peripheral: &Peripheral,
    ) -> Result<
        (btleplug::api::Service, Characteristic, Characteristic),
        Box<dyn std::error::Error + Send + Sync>,
    > {
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

        Err("No suitable service found".into())
    }

    fn write(&mut self, data: &[u8]) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let (tx, rx) = oneshot::channel();

        self.event_tx.send(BleEvent::Write {
            data: data.to_vec(),
            response: tx,
        })?;

        match rx.blocking_recv() {
            Ok(Ok(size)) => Ok(size),
            Ok(Err(err)) => Err(err.into()),
            Err(_) => Err("Channel closed".into()),
        }
    }

    fn read_charecteristics(
        &self,
        uuid: Uuid,
        buffer: &mut [u8],
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let (tx, rx) = oneshot::channel();

        self.event_tx
            .send(BleEvent::ReadCharacteristic { uuid, response: tx })?;

        match rx.blocking_recv() {
            Ok(Ok(data)) => {
                let copy_size = std::cmp::min(data.len(), buffer.len());
                buffer[..copy_size].copy_from_slice(&data[..copy_size]);
                Ok(copy_size)
            }
            Ok(Err(err)) => Err(err.into()),
            Err(_) => Err("No data available".into()),
        }
    }

    fn read(&self, buffer: &mut [u8]) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let (tx, rx) = oneshot::channel();

        self.event_tx.send(BleEvent::Read {
            size: buffer.len(),
            response: tx,
        })?;

        match rx.blocking_recv() {
            Ok(Ok(data)) => {
                let copy_size = std::cmp::min(data.len(), buffer.len());
                buffer[..copy_size].copy_from_slice(&data[..copy_size]);
                Ok(copy_size)
            }
            Ok(Err(err)) => Err(err.into()),
            Err(_) => Err("No data available".into()),
        }
    }

    fn poll(&self, timeout: Duration) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let (tx, rx) = oneshot::channel();

        self.event_tx.send(BleEvent::Poll {
            timeout,
            response: tx,
        })?;

        Ok(rx.blocking_recv()?)
    }

    fn set_timeout(&mut self, timeout: Duration) {
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

#[unsafe(no_mangle)]
extern "C" fn ble_open(
    io: *mut *mut c_void,
    _context: *mut dc_context_t,
    devaddr: *const c_char,
    _userdata: *mut c_void,
) -> dc_status_t {
    let addr_str = unsafe { CStr::from_ptr(devaddr) }.to_str().unwrap();

    // Skip "LE:" prefix if present
    let addr = if addr_str.starts_with("LE:") {
        &addr_str[3..]
    } else {
        addr_str
    };

    // Create a new runtime for this connection
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_err) => {
            // @TODO Store error in userdata?
            return ffi::DC_STATUS_IO;
        }
    };

    let transport = match rt.block_on(BleTransport::connect(addr)) {
        Ok(t) => Box::new(t),
        Err(_err) => {
            // @TODO Store error in userdata?
            return ffi::DC_STATUS_IO;
        }
    };

    unsafe {
        *io = Box::into_raw(transport) as *mut c_void;
    }

    // Keep the runtime alive by leaking it (we'll clean up in close)
    Box::leak(Box::new(rt));

    ffi::DC_STATUS_SUCCESS
}

#[unsafe(no_mangle)]
extern "C" fn ble_close(io: *mut c_void) -> dc_status_t {
    if !io.is_null() {
        let _transport = unsafe { Box::from_raw(io as *mut BleTransport) };
        // Transport will be dropped here, cleaning up the connection
    }
    ffi::DC_STATUS_SUCCESS
}

#[unsafe(no_mangle)]
extern "C" fn ble_read(
    io: *mut c_void,
    data: *mut c_void,
    size: usize,
    actual: *mut usize,
) -> dc_status_t {
    if io.is_null() || data.is_null() {
        return ffi::DC_STATUS_IO;
    }

    let transport = unsafe { &*(io as *const BleTransport) };
    let buffer = unsafe { std::slice::from_raw_parts_mut(data as *mut u8, size) };

    match transport.read(buffer) {
        Ok(bytes_read) => {
            if !actual.is_null() {
                unsafe {
                    *actual = bytes_read;
                }
            }
            ffi::DC_STATUS_SUCCESS
        }
        Err(_err) => {
            // @TODO Store error in io?
            return ffi::DC_STATUS_IO;
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn ble_write(
    io: *mut c_void,
    data: *const c_void,
    size: usize,
    actual: *mut usize,
) -> dc_status_t {
    if io.is_null() || data.is_null() {
        return ffi::DC_STATUS_IO;
    }

    let transport = unsafe { (io as *mut BleTransport).as_mut() }
        .ok_or("Null pointer")
        .unwrap();
    let data_slice = unsafe { std::slice::from_raw_parts(data as *const u8, size) };

    match transport.write(data_slice) {
        Ok(bytes_written) => {
            if !actual.is_null() {
                unsafe {
                    *actual = bytes_written;
                }
            }
            ffi::DC_STATUS_SUCCESS
        }
        Err(_err) => {
            // @TODO Store error in io?
            return ffi::DC_STATUS_IO;
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn ble_poll(io: *mut c_void, timeout: i32) -> dc_status_t {
    if io.is_null() {
        return ffi::DC_STATUS_IO;
    }

    let transport = unsafe { &*(io as *const BleTransport) };

    match transport.poll(Duration::from_millis(timeout as u64)) {
        Ok(true) => ffi::DC_STATUS_SUCCESS,
        Ok(false) => ffi::DC_STATUS_TIMEOUT,
        Err(_err) => {
            // @TODO Store error in io?
            return ffi::DC_STATUS_IO;
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn ble_set_timeout(io: *mut c_void, timeout: i32) -> dc_status_t {
    if io.is_null() {
        return ffi::DC_STATUS_IO;
    }

    let transport = unsafe { &mut *(io as *mut BleTransport) };
    transport.set_timeout(Duration::from_millis(timeout as u64));
    ffi::DC_STATUS_SUCCESS
}

#[unsafe(no_mangle)]
pub extern "C" fn ble_ioctl(
    io: *mut c_void,
    request: u32,
    data: *mut c_void,
    size: usize,
) -> dc_status_t {
    if io.is_null() {
        return ffi::DC_STATUS_IO;
    }

    let transport = unsafe { &*(io as *const BleTransport) };

    match request {
        ffi::DC_IOCTL_BLE_GET_NAME => {
            if data.is_null() {
                // @TODO Store error in io?
                return ffi::DC_STATUS_IO;
            }
            let name = transport.get_name();
            let buffer = unsafe { std::slice::from_raw_parts_mut(data as *mut u8, size) };
            let name_bytes = name.as_bytes();
            let copy_size = std::cmp::min(name_bytes.len(), buffer.len() - 1);
            buffer[..copy_size].copy_from_slice(&name_bytes[..copy_size]);
            buffer[copy_size] = 0; // Null terminate
            //
            ffi::DC_STATUS_SUCCESS
        }
        ffi::DC_IOCTL_BLE_CHARACTERISTIC_READ => {
            let (uuid, p) = unsafe {
                let data_ptr = data as *mut u8;

                if size < 16 {
                    // @TODO Store error in io?
                    return ffi::DC_STATUS_INVALIDARGS;
                }

                let uuid_bytes = std::slice::from_raw_parts(data_ptr, 16);
                let Ok(uuid) = Uuid::from_slice(uuid_bytes) else {
                    // @TODO Store error in io?
                    return ffi::DC_STATUS_INVALIDARGS;
                };

                let readsize = size - 16;

                let p = std::slice::from_raw_parts_mut(data_ptr.add(16), readsize);

                (uuid, p)
            };

            if transport.read_charecteristics(uuid, p).is_err() {
                return ffi::DC_STATUS_INVALIDARGS;
            }

            ffi::DC_STATUS_SUCCESS
        }
        _ => ffi::DC_STATUS_UNSUPPORTED,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ble_packet_open(
    iostream: *mut *mut dc_iostream_t,
    context: *mut dc_context_t,
    devaddr: *const c_char,
    userdata: *mut c_void,
) -> dc_status_t {
    let mut io = ptr::null_mut();

    let rc = ble_open(&mut io, context, devaddr, userdata);
    if rc != ffi::DC_STATUS_SUCCESS {
        // @TODO Store error in io?
        return rc;
    }

    let callbacks = dc_custom_cbs_t {
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

    unsafe { ffi::dc_custom_open(iostream, context, ffi::DC_TRANSPORT_BLE, &callbacks, io) }
}
// Parse the BlueZ format to extract the MAC address
fn parse_bluez_address(address_string: &str) -> Option<u64> {
    // Format: "hci0/dev_XX_XX_XX_XX_XX_XX"
    let parts: Vec<&str> = address_string.split('/').collect();
    if parts.len() != 2 {
        return None;
    }

    let dev_part = parts[1];
    if !dev_part.starts_with("dev_") {
        return None;
    }

    // Extract the MAC address part: "EB_41_89_AF_7E_5D"
    let mac_part = &dev_part[4..];
    let mac_with_colons = mac_part.replace('_', ":");

    // Parse MAC address to u64
    mac_string_to_u64(&mac_with_colons)
}

// Convert MAC address string to u64
fn mac_string_to_u64(mac: &str) -> Option<u64> {
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() != 6 {
        return None;
    }

    let mut address: u64 = 0;
    for (i, part) in parts.iter().enumerate() {
        let byte = u8::from_str_radix(part, 16).ok()?;
        address |= (byte as u64) << (40 - i * 8);
    }

    Some(address)
}

fn peripheral_id_to_address(id: &PeripheralId) -> Option<u64> {
    let id_str = id.to_string();

    // Linux/BlueZ format: "hci0/dev_XX_XX_XX_XX_XX_XX"
    if id_str.contains("/dev_") {
        return parse_bluez_address(&id_str);
    }

    // Standard MAC address format: "AA:BB:CC:DD:EE:FF"
    if id_str.contains(':') {
        return mac_string_to_u64(&id_str);
    }

    // Windows/other format might use hyphens: "AA-BB-CC-DD-EE-FF"
    if id_str.contains('-') {
        let with_colons = id_str.replace('-', ":");
        return mac_string_to_u64(&with_colons);
    }

    None
}
