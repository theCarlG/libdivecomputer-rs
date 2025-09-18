mod common;
mod context;
mod descriptor;
mod device;
pub mod error;
pub mod iterator;
mod parser;
mod version;

use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};

use btleplug::platform::PeripheralId;
use serde::{Deserialize, Serialize};

pub use crate::common::*;
use crate::context::Context;
pub use crate::context::LogLevel;
use crate::descriptor::Descriptor;
use crate::device::ble::KNOWN_SERVICES;
pub use crate::device::{ConnectionInfo, DeviceInfo, Family, Transport};
pub use crate::device::{Device, DeviceConnected};
pub use crate::error::{LibError, Result};
use crate::iterator::DcIterator;
use crate::parser::Parser;
pub use crate::parser::{
    Deco, DecoKind, DecoModel, Dive, DiveEvent, DiveMode, DiveSample, Fingerprint, GasUsage,
    Gasmix, Ppo2, Sensor, Tank, TankKind, TankUsage,
};

pub static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();

#[cfg(target_os = "android")]
use device::ble::android;

#[cfg(target_os = "android")]
pub use android::*;

#[cfg(target_os = "android")]
#[expect(unsafe_code)]
pub(crate) fn get_runtime() -> Result<&'static tokio::runtime::Runtime> {
    JAVAVM.get_or_init(|| {
        let vm_ptr = ndk_context::android_context().vm();
        unsafe { jni::JavaVM::from_raw(vm_ptr as *mut _).expect("Invalid JavaVM") }
    });

    Ok(RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name_fn(|| {
                use std::sync::atomic::{AtomicUsize, Ordering};

                static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                format!("libdc-thread-{}", id)
            })
            .on_thread_stop(move || {
                JNI_ENV.with(|f| *f.borrow_mut() = None);
            })
            .on_thread_start(move || {
                if let Some(vm) = JAVAVM.get()
                    && let Ok(env) = vm.attach_current_thread()
                {
                    JNI_ENV.with(|f| *f.borrow_mut() = Some(env));
                }
            })
            .build()
            .unwrap()
    }))
}

#[cfg(not(target_os = "android"))]
pub(crate) fn get_runtime() -> Result<&'static tokio::runtime::Runtime> {
    Ok(RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name_fn(|| {
                use std::sync::atomic::{AtomicUsize, Ordering};

                static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                format!("libdc-thread-{}", id)
            })
            .build()
            .unwrap()
    }))
}

#[cfg(target_os = "android")]
pub use device::ble::init as ble_android_init;

#[derive(Clone, Debug)]
pub struct DiveComputer {
    state: Arc<RwLock<DiveComputerState>>,
    context: Arc<Context>,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
    // log: Arc<mpsc::Receiver<(LogLevel, String)>>,
}

impl Default for DiveComputer {
    fn default() -> Self {
        Self::new()
    }
}

impl DiveComputer {
    pub fn version() -> String {
        version::version()
    }

    /// Create a new DiveComputer instance with its own runtime thread
    pub fn new() -> Self {
        let state = Arc::new(RwLock::new(DiveComputerState::Idle));

        // let (tx, rx) = mpsc::channel();
        let mut context = Context::default();
        context.set_loglevel(LogLevel::Debug).unwrap();
        context
            .set_logfunc(move |level, msg| {
                println!("{level}: {msg}");
                // if let Err(err) = tx.send((level, msg.to_string())) {
                //     eprintln!("failed to send log to channel: {err}");
                // }
            })
            .unwrap();

        Self {
            state,
            // log: Arc::new(rx),
            context: Arc::new(context),
            cancel_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    pub fn parse(&self, product: &Product, data: Vec<u8>) -> Result<Dive> {
        let mut descriptors = Descriptor::from(&self.context);
        let item = descriptors
            .find(|item| item.product() == product.name && item.vendor() == product.vendor)
            .ok_or_else(|| LibError::Other("Invalid product".to_string()))?;

        Parser::parse_standalone(&self.context, &item, data)
    }

    /// Get a sorted list of supported vendors
    pub fn vendors(&self) -> Result<Vec<Vendor>> {
        let descriptors = Descriptor::from(&self.context);

        // Group products by vendor
        let mut vendor_map: BTreeMap<String, Vec<Product>> = BTreeMap::new();

        for desc in descriptors {
            let vendor_name = desc.vendor();
            let product = Product {
                vendor: vendor_name.clone(),
                name: desc.product(),
                model: desc.model(),
                family: desc.family(),
                transports: desc.transports(),
            };

            vendor_map.entry(vendor_name).or_default().push(product);
        }

        // Convert to sorted vendor list
        let vendors: Vec<Vendor> = vendor_map
            .into_iter()
            .map(|(name, mut products)| {
                products.sort_by(|a, b| a.name.cmp(&b.name));
                Vendor { name, products }
            })
            .collect();

        Ok(vendors)
    }

    async fn connect_device(
        &self,
        product: &Product,
        device_info: &DeviceInfo,
        tx: mpsc::Sender<Dive>,
        cancel_flag: Arc<std::sync::atomic::AtomicBool>,
        state: Arc<RwLock<DiveComputerState>>,
    ) -> Result<Device<DeviceConnected>> {
        let item = Descriptor::from(&self.context)
            .find(|item| item.vendor() == product.vendor && item.product() == product.name)
            .ok_or(LibError::Other(
                "failed to find Descriptor item".to_string(),
            ))?;

        Device::new(
            &self.context,
            &device_info.connection_info.clone(),
            item,
            tx,
            cancel_flag,
            state,
        )?
        .connect()
        .await
    }

    async fn scan_impl(
        &self,
        _product: &Product,
        transport: Transport,
    ) -> Result<mpsc::Receiver<DeviceInfo>> {
        let (tx, rx) = mpsc::channel();

        self.set_state(DiveComputerState::Scanning {
            transport: transport.clone(),
        });

        let context = self.context.clone();
        let state = self.state.clone();
        let cancel_flag = self.cancel_flag.clone();

        get_runtime()?.spawn(async move {
            let result = match transport {
                //@TODO proper error
                Transport::None => return Err(LibError::Other("Invalid transport".into())),
                Transport::Ble => scan_ble_devices_impl(tx.clone(), cancel_flag.clone()).await,
                Transport::Serial => {
                    scan_serial_devices_impl(tx.clone(), &context, cancel_flag.clone()).await
                }
                Transport::Usb => {
                    scan_usb_devices_impl(tx.clone(), &context, cancel_flag.clone()).await
                }
                Transport::UsbHid => {
                    scan_usbhid_devices_impl(tx.clone(), &context, cancel_flag.clone()).await
                }
                Transport::Bluetooth => {
                    scan_bluetooth_devices_impl(tx.clone(), &context, cancel_flag.clone()).await
                }
                Transport::Irda => {
                    scan_irda_devices_impl(tx.clone(), &context, cancel_flag.clone()).await
                }
            };

            // Set state back to idle when done
            *state.write().unwrap() = DiveComputerState::Idle;

            match result {
                Ok(_) => Ok(()),
                Err(err) => Err(err),
            }
        });

        Ok(rx)
    }

    pub async fn scan(
        &self,
        product: &Product,
        transport: Transport,
    ) -> Result<DcIterator<DeviceInfo>> {
        let rx = self.scan_impl(product, transport).await?;

        Ok(DcIterator::new(rx))
    }

    /// Download dives from a device (returns an async iterator)
    async fn download_impl(
        &self,
        product: &Product,
        device: DeviceInfo,
        fingerprint: Option<String>,
    ) -> Result<mpsc::Receiver<Dive>> {
        let (tx, rx) = mpsc::channel();

        self.set_state(DiveComputerState::Connecting {
            device: device.name.clone(),
        });

        let cancel_flag = self.cancel_flag.clone();
        let state = self.state.clone();

        let mut device_handle = self
            .connect_device(
                &product,
                &device,
                tx.clone(),
                cancel_flag.clone(),
                state.clone(),
            )
            .await?;

        if let Some(fingerprint) = fingerprint {
            device_handle.set_fingerprint(&fingerprint)?
        }
        self.set_state(DiveComputerState::Downloading {
            device: device.name.clone(),
            progress: DownloadProgress {
                current: 0,
                total: 0,
            },
            current_task: Some("Starting".to_string()),
        });

        std::thread::spawn(move || {
            get_runtime().unwrap().spawn_blocking(move || {
                if let Err(err) = device_handle.start_download() {
                    eprintln!("Download error: {err:?}");
                }
            });
        });

        Ok(rx)
    }

    pub async fn download(
        &self,
        product: &Product,
        device: DeviceInfo,
        fingerprint: Option<String>,
    ) -> Result<DcIterator<Dive>> {
        let rx = self.download_impl(product, device, fingerprint).await?;

        Ok(DcIterator::new(rx))
    }

    /// Cancel any ongoing operation
    pub async fn cancel(&self) -> Result<()> {
        self.cancel_flag.store(true, Ordering::Relaxed);
        *self.state.write().unwrap() = DiveComputerState::Idle;
        Ok(())
    }

    /// Get the current state of the dive computer
    pub fn state(&self) -> DiveComputerState {
        self.state.read().unwrap().clone()
    }

    fn set_state(&self, state: DiveComputerState) {
        *self.state.write().unwrap() = state;
    }
}

#[derive(Debug)]
pub struct DiveComputerSync {
    inner: DiveComputer,
}

impl Default for DiveComputerSync {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DiveComputerSync {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl DiveComputerSync {
    /// Create a new synchronous DiveComputer with its own runtime
    pub fn new() -> Self {
        let inner = DiveComputer::new();

        Self { inner }
    }

    /// Get libdivecomputer version
    pub fn version(&self) -> String {
        version::version()
    }

    /// Get the current state
    pub fn state(&self) -> DiveComputerState {
        self.inner.state()
    }

    /// Get supported vencors
    pub fn vendors(&self) -> Result<Vec<Vendor>> {
        self.inner.vendors()
    }

    /// Scan for devices
    pub fn scan(&self, product: &Product, transport: Transport) -> Result<DcIterator<DeviceInfo>> {
        let inner = self.inner.clone();
        let product = product.clone();

        let rx =
            get_runtime()?.block_on(async move { inner.scan_impl(&product, transport).await })?;

        Ok(DcIterator::new(rx))
    }

    /// Parse a binary dive blob
    pub fn parse(&self, product: &Product, data: Vec<u8>) -> Result<Dive> {
        self.inner.parse(product, data)
    }

    /// Download dives from device
    pub fn download(
        &self,
        product: &Product,
        device: DeviceInfo,
        fingerprint: Option<String>,
    ) -> Result<DcIterator<Dive>> {
        let inner = self.inner.clone();
        let product = product.clone();

        let rx = get_runtime()?
            .block_on(async move { inner.download_impl(&product, device, fingerprint).await })?;

        Ok(DcIterator::new(rx))
    }

    /// Cancel current operation
    pub fn cancel(&self) -> Result<()> {
        get_runtime()?.block_on(self.inner.cancel())
    }
}

async fn scan_ble_devices_impl(
    tx: mpsc::Sender<DeviceInfo>,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
    use btleplug::platform::Manager;
    use std::time::Duration;

    let known_uuids: Vec<uuid::Uuid> = KNOWN_SERVICES.iter().map(|(uuid, _)| *uuid).collect();

    let manager = Manager::new()
        .await
        .map_err(|err| LibError::Other(err.to_string()))?;
    let adapters = manager
        .adapters()
        .await
        .map_err(|err| LibError::Other(err.to_string()))?;
    let adapter = adapters
        .into_iter()
        .next()
        .ok_or_else(|| LibError::Other("No Bluetooth adapter found".to_string()))?;

    let scan_filter = ScanFilter {
        services: known_uuids.clone(),
    };

    adapter
        .start_scan(scan_filter)
        .await
        .map_err(|err| LibError::Other(err.to_string()))?;

    // Scan for a duration, checking cancel flag periodically
    let scan_duration = Duration::from_secs(5);
    let start = tokio::time::Instant::now();

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            adapter.stop_scan().await.ok();
            return Err(LibError::Cancelled);
        }

        let peripherals = adapter
            .peripherals()
            .await
            .map_err(|err| LibError::Other(err.to_string()))?;

        let mut filtered_peripherals = Vec::new();
        for peripheral in peripherals {
            if let Ok(Some(props)) = peripheral.properties().await {
                for service_uuid in &props.services {
                    if let Some(idx) = known_uuids.iter().position(|&u| u == *service_uuid) {
                        let service_name = KNOWN_SERVICES[idx].1;
                        filtered_peripherals.push((
                            props.local_name.clone(),
                            service_name.to_string(),
                            peripheral.clone(),
                        ));
                    }
                }
            }
        }

        let found_periphals = !filtered_peripherals.is_empty();

        for (local_name, service_name, peripheral) in filtered_peripherals {
            let peripheral_id = peripheral.id();
            let address_string = peripheral_id.to_string();
            let address = peripheral_id_to_address(&peripheral_id)
                .ok_or(btleplug::Error::Other("invalid peripheral id".into()))?;

            let device = DeviceInfo {
                name: local_name
                    .clone()
                    .map(|local_name| format!("{local_name} - {service_name}"))
                    .unwrap_or(service_name.clone()),
                transport: Transport::Ble,
                connection_info: ConnectionInfo::Ble {
                    address,
                    address_string,
                    service_name,
                    local_name,
                },
            };

            if tx.send(device).is_err() {
                adapter.stop_scan().await.ok();
                return Ok(());
            }
        }

        if found_periphals || start.elapsed() >= scan_duration {
            break;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    adapter
        .stop_scan()
        .await
        .map_err(|err| LibError::Other(err.to_string()))?;

    Ok(())
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

async fn scan_serial_devices_impl(
    tx: mpsc::Sender<DeviceInfo>,
    context: &Context,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    use std::ffi::CStr;
    use std::ptr;

    get_runtime()?
        .spawn_blocking({
            let tx = tx.clone();
            let context = context.clone();
            let cancel_flag = cancel_flag.clone();

            move || {
                let mut iterator = ptr::null_mut();

                let status = unsafe {
                    libdivecomputer_sys::dc_serial_iterator_new(
                        &mut iterator,
                        context.ptr(),
                        ptr::null_mut(),
                    )
                };

                if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                    return Err(LibError::Other(format!(
                        "Failed to create serial iterator: {}",
                        status
                    )));
                }

                loop {
                    if cancel_flag.load(Ordering::Relaxed) {
                        unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                        return Err(LibError::Cancelled);
                    }

                    let mut device: *mut libdivecomputer_sys::dc_serial_device_t = ptr::null_mut();
                    let status = unsafe {
                        libdivecomputer_sys::dc_iterator_next(
                            iterator,
                            &mut device as *mut _ as *mut std::ffi::c_void,
                        )
                    };

                    if status == libdivecomputer_sys::DC_STATUS_DONE {
                        break;
                    }

                    if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                        break;
                    }

                    if device.is_null() {
                        continue;
                    }

                    let name_ptr =
                        unsafe { libdivecomputer_sys::dc_serial_device_get_name(device) };
                    let path = if name_ptr.is_null() {
                        "Unknown".to_string()
                    } else {
                        unsafe { CStr::from_ptr(name_ptr).to_string_lossy().to_string() }
                    };

                    let name = extract_device_name(&path);
                    let device_info = DeviceInfo {
                        name: name.clone(),
                        transport: Transport::Serial,
                        connection_info: ConnectionInfo::Serial { name, path },
                    };

                    // Use blocking send since we're in a blocking context
                    if tx.send(device_info).is_err() {
                        unsafe {
                            libdivecomputer_sys::dc_serial_device_free(device);
                            libdivecomputer_sys::dc_iterator_free(iterator);
                        }
                        return Ok(());
                    }

                    unsafe { libdivecomputer_sys::dc_serial_device_free(device) };
                }

                unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                Ok(())
            }
        })
        .await
        .map_err(|err| LibError::Other(err.to_string()))?
}

async fn scan_usb_devices_impl(
    tx: mpsc::Sender<DeviceInfo>,
    context: &Context,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    use std::ptr;

    get_runtime()?
        .spawn_blocking({
            let tx = tx.clone();
            let context = context.clone();
            let cancel_flag = cancel_flag.clone();

            move || {
                let mut iterator = ptr::null_mut();

                let status = unsafe {
                    libdivecomputer_sys::dc_usb_iterator_new(
                        &mut iterator,
                        context.ptr(),
                        ptr::null_mut(),
                    )
                };

                if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                    return Err(LibError::Other(format!(
                        "Failed to create USB iterator: {}",
                        status
                    )));
                }

                loop {
                    if cancel_flag.load(Ordering::Relaxed) {
                        unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                        return Err(LibError::Cancelled);
                    }

                    let mut device: *mut libdivecomputer_sys::dc_usb_device_t = ptr::null_mut();
                    let status = unsafe {
                        libdivecomputer_sys::dc_iterator_next(
                            iterator,
                            &mut device as *mut _ as *mut std::ffi::c_void,
                        )
                    };

                    if status == libdivecomputer_sys::DC_STATUS_DONE {
                        break;
                    }

                    if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                        break;
                    }

                    if device.is_null() {
                        continue;
                    }

                    let vid = unsafe { libdivecomputer_sys::dc_usb_device_get_vid(device) } as u16;
                    let pid = unsafe { libdivecomputer_sys::dc_usb_device_get_pid(device) } as u16;

                    let name = get_usb_device_name(vid, pid)
                        .unwrap_or_else(|| format!("USB Device {:04X}:{:04X}", vid, pid));

                    let device_info = DeviceInfo {
                        name,
                        transport: Transport::Usb,
                        connection_info: ConnectionInfo::Usb {
                            vendor_id: vid,
                            product_id: pid,
                            device_path: None,
                        },
                    };

                    if tx.send(device_info).is_err() {
                        unsafe {
                            libdivecomputer_sys::dc_usb_device_free(device);
                            libdivecomputer_sys::dc_iterator_free(iterator);
                        }
                        return Ok(());
                    }

                    unsafe { libdivecomputer_sys::dc_usb_device_free(device) };
                }

                unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                Ok(())
            }
        })
        .await
        .map_err(|err| LibError::Other(err.to_string()))?
}

async fn scan_usbhid_devices_impl(
    tx: mpsc::Sender<DeviceInfo>,
    context: &Context,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    use std::ptr;

    get_runtime()?
        .spawn_blocking({
            let tx = tx.clone();
            let context = context.clone();
            let cancel_flag = cancel_flag.clone();

            move || {
                let mut iterator = ptr::null_mut();

                let status = unsafe {
                    libdivecomputer_sys::dc_usbhid_iterator_new(
                        &mut iterator,
                        context.ptr(),
                        ptr::null_mut(),
                    )
                };

                if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                    return Err(LibError::Other(format!(
                        "Failed to create USB HID iterator: {}",
                        status
                    )));
                }

                loop {
                    if cancel_flag.load(Ordering::Relaxed) {
                        unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                        return Err(LibError::Cancelled);
                    }

                    let mut device: *mut libdivecomputer_sys::dc_usbhid_device_t = ptr::null_mut();
                    let status = unsafe {
                        libdivecomputer_sys::dc_iterator_next(
                            iterator,
                            &mut device as *mut _ as *mut std::ffi::c_void,
                        )
                    };

                    if status == libdivecomputer_sys::DC_STATUS_DONE {
                        break;
                    }

                    if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                        break;
                    }

                    if device.is_null() {
                        continue;
                    }

                    let vid =
                        unsafe { libdivecomputer_sys::dc_usbhid_device_get_vid(device) } as u16;
                    let pid =
                        unsafe { libdivecomputer_sys::dc_usbhid_device_get_pid(device) } as u16;

                    let name = get_usb_device_name(vid, pid)
                        .unwrap_or_else(|| format!("USB HID Device {:04X}:{:04X}", vid, pid));

                    let device_info = DeviceInfo {
                        name,
                        transport: Transport::UsbHid,
                        connection_info: ConnectionInfo::UsbHid {
                            vendor_id: vid,
                            product_id: pid,
                            device_path: None,
                        },
                    };

                    if tx.send(device_info).is_err() {
                        unsafe {
                            libdivecomputer_sys::dc_usbhid_device_free(device);
                            libdivecomputer_sys::dc_iterator_free(iterator);
                        }
                        return Ok(());
                    }

                    unsafe { libdivecomputer_sys::dc_usbhid_device_free(device) };
                }

                unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                Ok(())
            }
        })
        .await
        .map_err(|err| LibError::Other(err.to_string()))?
}

async fn scan_bluetooth_devices_impl(
    tx: mpsc::Sender<DeviceInfo>,
    context: &Context,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    use std::ffi::CStr;
    use std::ptr;

    get_runtime()?
        .spawn_blocking({
            let tx = tx.clone();
            let context = context.clone();
            let cancel_flag = cancel_flag.clone();

            move || {
                let mut iterator = ptr::null_mut();

                let status = unsafe {
                    libdivecomputer_sys::dc_bluetooth_iterator_new(
                        &mut iterator,
                        context.ptr(),
                        ptr::null_mut(),
                    )
                };

                if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                    return Err(LibError::Other(format!(
                        "Failed to create Bluetooth iterator: {}",
                        status
                    )));
                }

                loop {
                    if cancel_flag.load(Ordering::Relaxed) {
                        unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                        return Err(LibError::Cancelled);
                    }

                    let mut device: *mut libdivecomputer_sys::dc_bluetooth_device_t =
                        ptr::null_mut();
                    let status = unsafe {
                        libdivecomputer_sys::dc_iterator_next(
                            iterator,
                            &mut device as *mut _ as *mut std::ffi::c_void,
                        )
                    };

                    if status == libdivecomputer_sys::DC_STATUS_DONE {
                        break;
                    }

                    if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                        break;
                    }

                    if device.is_null() {
                        continue;
                    }

                    let address =
                        unsafe { libdivecomputer_sys::dc_bluetooth_device_get_address(device) };
                    let name_ptr =
                        unsafe { libdivecomputer_sys::dc_bluetooth_device_get_name(device) };

                    let name = if name_ptr.is_null() {
                        "Unknown Bluetooth Device".to_string()
                    } else {
                        unsafe { CStr::from_ptr(name_ptr).to_string_lossy().to_string() }
                    };

                    let address_string = format_bluetooth_address(address);

                    let device_info = DeviceInfo {
                        name: name.clone(),
                        transport: Transport::Bluetooth,
                        connection_info: ConnectionInfo::Bluetooth {
                            address,
                            address_string,
                            name,
                        },
                    };

                    if tx.send(device_info).is_err() {
                        unsafe {
                            libdivecomputer_sys::dc_bluetooth_device_free(device);
                            libdivecomputer_sys::dc_iterator_free(iterator);
                        }
                        return Ok(());
                    }

                    unsafe { libdivecomputer_sys::dc_bluetooth_device_free(device) };
                }

                unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                Ok(())
            }
        })
        .await
        .map_err(|err| LibError::Other(err.to_string()))?
}

async fn scan_irda_devices_impl(
    tx: mpsc::Sender<DeviceInfo>,
    context: &Context,
    cancel_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    use std::ffi::CStr;
    use std::ptr;

    get_runtime()?
        .spawn_blocking({
            let tx = tx.clone();
            let context = context.clone();
            let cancel_flag = cancel_flag.clone();

            move || {
                let mut iterator = ptr::null_mut();

                let status = unsafe {
                    libdivecomputer_sys::dc_irda_iterator_new(
                        &mut iterator,
                        context.ptr(),
                        ptr::null_mut(),
                    )
                };

                if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                    return Err(LibError::Other(format!(
                        "Failed to create IrDA iterator: {}",
                        status
                    )));
                }

                loop {
                    if cancel_flag.load(Ordering::Relaxed) {
                        unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                        return Err(LibError::Cancelled);
                    }

                    let mut device: *mut libdivecomputer_sys::dc_irda_device_t = ptr::null_mut();
                    let status = unsafe {
                        libdivecomputer_sys::dc_iterator_next(
                            iterator,
                            &mut device as *mut _ as *mut std::ffi::c_void,
                        )
                    };

                    if status == libdivecomputer_sys::DC_STATUS_DONE {
                        break;
                    }

                    if status != libdivecomputer_sys::DC_STATUS_SUCCESS {
                        break;
                    }

                    if device.is_null() {
                        continue;
                    }

                    let address =
                        unsafe { libdivecomputer_sys::dc_irda_device_get_address(device) };
                    let name_ptr = unsafe { libdivecomputer_sys::dc_irda_device_get_name(device) };

                    let name = if name_ptr.is_null() {
                        "Unknown IrDA Device".to_string()
                    } else {
                        unsafe { CStr::from_ptr(name_ptr).to_string_lossy().to_string() }
                    };

                    let device_info = DeviceInfo {
                        name: name.clone(),
                        transport: Transport::Irda,
                        connection_info: ConnectionInfo::Irda { address, name },
                    };

                    if tx.send(device_info).is_err() {
                        unsafe {
                            libdivecomputer_sys::dc_irda_device_free(device);
                            libdivecomputer_sys::dc_iterator_free(iterator);
                        }
                        return Ok(());
                    }

                    unsafe { libdivecomputer_sys::dc_irda_device_free(device) };
                }

                unsafe { libdivecomputer_sys::dc_iterator_free(iterator) };
                Ok(())
            }
        })
        .await
        .map_err(|err| LibError::Other(err.to_string()))?
}

/// Format a Bluetooth address as a string
fn format_bluetooth_address(address: u64) -> String {
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        (address >> 40) & 0xFF,
        (address >> 32) & 0xFF,
        (address >> 24) & 0xFF,
        (address >> 16) & 0xFF,
        (address >> 8) & 0xFF,
        address & 0xFF
    )
}

/// Extract a friendly device name from a path
fn extract_device_name(path: &str) -> String {
    if let Some(name) = path.split('/').next_back() {
        name.to_string()
    } else {
        path.to_string()
    }
}

/// Get a friendly name for a USB device based on VID/PID
fn get_usb_device_name(vid: u16, pid: u16) -> Option<String> {
    match (vid, pid) {
        (0x1493, 0x0030) => Some("Suunto EON Steel".to_string()),
        (0x1493, 0x0031) => Some("Suunto EON Core".to_string()),
        (0x2E6A, 0x0005) => Some("Uwatec Smart".to_string()),
        (0x2E6A, 0x0003) => Some("Shearwater Petrel/Perdix".to_string()),
        (0x0403, 0x6001) => Some("FTDI-based Dive Computer".to_string()),
        (0x0403, 0x6015) => Some("Atomic Aquatics Cobalt".to_string()),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Vendor {
    pub name: String,
    pub(crate) products: Vec<Product>,
}

impl Vendor {
    /// Get all products from this vendor
    pub fn products(&self) -> Vec<Product> {
        self.products.clone()
    }
}

impl Display for Vendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Represents a specific dive computer product/model
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct Product {
    pub vendor: String,
    pub name: String,
    pub model: u32,
    pub family: device::Family,
    pub transports: Vec<Transport>,
}

impl Display for Product {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// State of the dive computer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiveComputerState {
    Idle,
    WaitingForUser,
    Scanning {
        transport: Transport,
    },
    Connecting {
        device: String,
    },
    Downloading {
        device: String,
        progress: DownloadProgress,
        current_task: Option<String>,
    },
    Error(String),
}

impl Display for DiveComputerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::WaitingForUser => write!(f, "Waiting for user input"),
            Self::Scanning { transport } => write!(f, "Scanning for {transport} devices"),
            Self::Connecting { device } => write!(f, "Connecting to {device}"),
            Self::Downloading {
                device, progress, ..
            } => {
                write!(f, "Dowloading dives from {device}: {progress}")
            }
            Self::Error(err) => write!(f, "Dive computer failed: {err:?}"),
        }
    }
}

/// Download progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgress {
    pub current: u32,
    pub total: u32,
}

impl Display for DownloadProgress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.1}%",
            100.0 * (self.current as f64) / (self.total as f64)
        )
    }
}
