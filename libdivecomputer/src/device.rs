mod ble;

use std::{
    ffi::{CStr, CString, c_int, c_uchar, c_uint, c_void},
    fmt,
    marker::PhantomData,
    ptr,
    time::Duration,
};

use libdivecomputer_sys as ffi;
use serde::Serialize;
use serde_repr::Deserialize_repr;

use crate::{
    Context, DiveComputer, cast_void,
    common::Status,
    descriptor::DescriptorItem,
    device::ble::BleTransport,
    error::{LibError, Result},
    parser::{Dive, Parser},
    void_ptr,
};

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr)]
pub enum Transport {
    None = 0,
    Serial = 1 << 0,
    Usb = 1 << 1,
    UsbHid = 1 << 2,
    Irda = 1 << 3,
    Bluetooth = 1 << 4,
    Ble = 1 << 5,
}

impl From<u32> for Transport {
    fn from(value: u32) -> Self {
        match value {
            0x00000001 => Self::Serial,
            0x00000010 => Self::Usb,
            0x00000100 => Self::UsbHid,
            0x00001000 => Self::Irda,
            0x00010000 => Self::Bluetooth,
            0x00100000 => Self::Ble,
            _ => Self::None,
        }
    }
}

impl Transport {
    pub fn vec_from_bitflag(value: u32) -> Vec<Transport> {
        let mut transports = Vec::new();

        if value & (Transport::Usb as u32) != 0 {
            transports.push(Self::Usb);
        }
        if value & (Self::UsbHid as u32) != 0 {
            transports.push(Self::UsbHid);
        }
        if value & (Self::Ble as u32) != 0 {
            transports.push(Self::Ble);
        }
        if value & (Self::Bluetooth as u32) != 0 {
            transports.push(Self::Bluetooth);
        }
        if value & (Self::Serial as u32) != 0 {
            transports.push(Self::Serial);
        }
        if value & (Self::Irda as u32) != 0 {
            transports.push(Self::Irda);
        }

        transports
    }
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr, Default)]
pub enum Family {
    #[default]
    None = 0,

    // Suunto
    SuuntoSolution = 1 << 16,
    SuuntoEon,
    SuuntoVyper,
    SuuntoVyper2,
    SuuntoD9,
    SuuntoEonSteel,

    // Reefnet
    ReefnetSensus = 2 << 16,
    ReefnetSensusPro,
    ReefnetSensusUltra,

    // Uwatec
    UwatecAladin = 3 << 16,
    UwatecMemoMouse,
    UwatecSmart,
    UwatecMeridian,
    UwatecG2,

    // Oceanic
    OceanicVtPro = 4 << 16,
    OceanicVeo250,
    OceanicAtom2,

    // Mares
    MaresNemo = 5 << 16,
    MaresPuck,
    MaresDarwin,
    MaresIconHD,

    // Heinrichs Weikamp
    HwOstc = 6 << 16,
    HwFrog,
    HwOstc3,

    // Cressi
    CressiEdy = 7 << 16,
    CressiLeonardo,
    CressiGoa,

    // Zeagle
    ZeagleN2ition3 = 8 << 16,

    // Atomic Aquatics
    AtomicsCobalt = 9 << 16,

    // Shearwater
    ShearwaterPredator = 10 << 16,
    ShearwaterPetrel,

    // Dive Rite
    DiveRiteNitekQ = 11 << 16,

    // Citizen
    CitizenAqualand = 12 << 16,

    // DiveSystem
    DiveSystemIDive = 13 << 16,

    // Cochran
    CochranCommander = 14 << 16,

    // Tecdiving
    TecdivingDivecomputerEu = 15 << 16,

    // McLean
    McLeanExtreme = 16 << 16,

    // Liquivision
    LiquivisionLynx = 17 << 16,

    // Sporasub
    SporasubSp2 = 18 << 16,

    // Deep Six
    DeepSixExcursion = 19 << 16,

    // Seac Screen
    SeacScreen = 20 << 16,

    // Deepblu Cosmiq
    DeepbluCosmiq = 21 << 16,

    // Oceans S1
    OceansS1 = 22 << 16,

    // Divesoft Freedom
    DivesoftFreedom = 23 << 16,

    // Halcyon
    HalcyonSymbios = 24 << 16,
}

impl From<u32> for Family {
    fn from(value: u32) -> Self {
        match value {
            0 => Family::None,

            // Suunto
            0x00010000 => Family::SuuntoSolution,
            0x00010001 => Family::SuuntoEon,
            0x00010002 => Family::SuuntoVyper,
            0x00010003 => Family::SuuntoVyper2,
            0x00010004 => Family::SuuntoD9,
            0x00010005 => Family::SuuntoEonSteel,

            // Reefnet
            0x00020000 => Family::ReefnetSensus,
            0x00020001 => Family::ReefnetSensusPro,
            0x00020002 => Family::ReefnetSensusUltra,

            // Uwatec
            0x00030000 => Family::UwatecAladin,
            0x00030001 => Family::UwatecMemoMouse,
            0x00030002 => Family::UwatecSmart,
            0x00030003 => Family::UwatecMeridian,
            0x00030004 => Family::UwatecG2,

            // Oceanic
            0x00040000 => Family::OceanicVtPro,
            0x00040001 => Family::OceanicVeo250,
            0x00040002 => Family::OceanicAtom2,

            // Mares
            0x00050000 => Family::MaresNemo,
            0x00050001 => Family::MaresPuck,
            0x00050002 => Family::MaresDarwin,
            0x00050003 => Family::MaresIconHD,

            // Heinrichs Weikamp
            0x00060000 => Family::HwOstc,
            0x00060001 => Family::HwFrog,
            0x00060002 => Family::HwOstc3,

            // Cressi
            0x00070000 => Family::CressiEdy,
            0x00070001 => Family::CressiLeonardo,
            0x00070002 => Family::CressiGoa,

            // Zeagle
            0x00080000 => Family::ZeagleN2ition3,

            // Atomic Aquatics
            0x00090000 => Family::AtomicsCobalt,

            // Shearwater
            0x000A0000 => Family::ShearwaterPredator,
            0x000A0001 => Family::ShearwaterPetrel,

            // Dive Rite
            0x000B0000 => Family::DiveRiteNitekQ,

            // Citizen
            0x000C0000 => Family::CitizenAqualand,

            // DiveSystem
            0x000D0000 => Family::DiveSystemIDive,

            // Cochran
            0x000E0000 => Family::CochranCommander,

            // Tecdiving
            0x000F0000 => Family::TecdivingDivecomputerEu,

            // McLean
            0x00100000 => Family::McLeanExtreme,

            // Liquivision
            0x00110000 => Family::LiquivisionLynx,

            // Sporasub
            0x00120000 => Family::SporasubSp2,

            // Deep Six
            0x00130000 => Family::DeepSixExcursion,

            // Seac Screen
            0x00140000 => Family::SeacScreen,

            // Deepblu Cosmiq
            0x00150000 => Family::DeepbluCosmiq,

            // Oceans S1
            0x00160000 => Family::OceansS1,

            // Divesoft Freedom
            0x00170000 => Family::DivesoftFreedom,

            // Halcyon Symbios
            0x00180000 => Family::HalcyonSymbios,

            _ => Family::None, // Default for unknown values
        }
    }
}

#[derive(Debug, Clone)]
#[repr(C)]
pub(crate) struct DeviceData {
    pub(crate) dives: Vec<Dive>,
}

impl Default for DeviceData {
    fn default() -> Self {
        Self { dives: Vec::new() }
    }
}

pub trait DeviceState {}

pub struct DeviceConnected;
impl DeviceState for DeviceConnected {}

pub struct DeviceDisconnected;
impl DeviceState for DeviceDisconnected {}

#[derive(Debug, Clone)]
pub struct Device<'ctx, 'item, T: DeviceState> {
    pub(crate) ptr: *mut ffi::dc_device_t,
    pub(crate) item: &'item DescriptorItem,
    pub(crate) context: &'ctx Context,
    iostream: *mut ffi::dc_iostream_t,

    pub(crate) computer: DiveComputer,
    progress: (u32, u32),

    pub(crate) data: DeviceData,
    transport: DeviceTransport,

    cancel: bool,

    _panthom: PhantomData<T>,
}

impl<'ctx, 'item> Device<'ctx, 'item, DeviceDisconnected> {
    pub fn new(
        context: &'ctx Context,
        transport: DeviceTransport,
        item: &'item DescriptorItem,
    ) -> Result<Self> {
        let computer = DiveComputer::try_from(item)?;
        Ok(Self {
            ptr: ptr::null_mut(),
            context,
            transport,
            computer,
            item,
            iostream: ptr::null_mut(),
            data: DeviceData::default(),
            progress: (0, 0),
            cancel: false,
            _panthom: PhantomData,
        })
    }

    pub fn connect(mut self) -> Result<Device<'ctx, 'item, DeviceConnected>> {
        self.transport.connect(
            self.context.ptr,
            &mut self.iostream,
            void_ptr!(&mut self.data),
        )?;
        unsafe {
            let status = ffi::dc_device_open(
                &mut self.ptr,
                self.context.ptr,
                self.item.ptr,
                self.iostream,
            );
            if status != ffi::DC_STATUS_SUCCESS {
                return Err(LibError::status_with_context(
                    status,
                    "failed to open device",
                ));
            }

            let events = ffi::DC_EVENT_WAITING
                | ffi::DC_EVENT_PROGRESS
                | ffi::DC_EVENT_DEVINFO
                | ffi::DC_EVENT_CLOCK
                | ffi::DC_EVENT_VENDOR;

            let status = ffi::dc_device_set_events(
                self.ptr,
                events,
                Some(event_callback),
                void_ptr!(&mut self),
            );
            if status != ffi::DC_STATUS_SUCCESS {
                return Err(LibError::status_with_context(
                    status,
                    "failed to set event handler",
                ));
            }

            let status =
                ffi::dc_device_set_cancel(self.ptr, Some(cancel_callback), void_ptr!(&mut self));
            if status != ffi::DC_STATUS_SUCCESS {
                return Err(LibError::status_with_context(
                    status,
                    "failed to set cancel callback",
                ));
            }

            let new_self = Device::<'ctx, 'item, DeviceConnected> {
                ptr: self.ptr,
                item: self.item,
                context: self.context,
                iostream: self.iostream,
                computer: self.computer,
                progress: self.progress,
                data: self.data,
                transport: self.transport,
                cancel: self.cancel,
                _panthom: PhantomData,
            };
            Ok(new_self)
        }
    }
}

impl<'ctx, 'item> Device<'ctx, 'item, DeviceConnected> {
    pub fn set_fingerprint(&self, fingerprint: &str) -> Result<()> {
        let fingerprint_bytes = hex_string_to_bytes(fingerprint)?;
        let status = unsafe {
            ffi::dc_device_set_fingerprint(
                self.ptr,
                fingerprint_bytes.as_ptr(),
                fingerprint_bytes.len() as c_uint,
            )
        };

        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::status_with_context(
                status,
                "failed to set device fingerprint",
            ));
        }

        Ok(())
    }

    pub fn set_datetime(&self, _timestamp: jiff::Timestamp) -> Result<()> {
        #[expect(unused_unsafe)]
        let status = unsafe {
            // ffi::dc_device_timesync(
            //     self.ptr,
            //     fingerprint_bytes.as_ptr(),
            // )
            ffi::DC_STATUS_SUCCESS
        };

        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::status_with_context(
                status,
                "failed to set device fingerprint",
            ));
        }

        Ok(())
    }

    //@TODO refactor to iterator
    pub fn download(&mut self) -> Result<Vec<Dive>> {
        unsafe {
            let status = ffi::dc_device_foreach(self.ptr, Some(dive_callback), void_ptr!(self));
            if status != ffi::DC_STATUS_SUCCESS {
                return Err(LibError::status_with_context(
                    status,
                    "failed to set foreach dive",
                ));
            }

            return Ok(self.data.dives.clone());
        }
    }

    pub fn computer(&self) -> DiveComputer {
        self.computer.clone()
    }
}

#[derive(Debug, Clone)]
pub enum DeviceTransport {
    None,
    Serial {
        name: String,
        path: String,
    },
    Usb {
        vendor_id: u16,
        product_id: u16,
        device_path: Option<String>,
    },
    UsbHid {
        vendor_id: u16,
        product_id: u16,
        device_path: Option<String>,
    },
    Bluetooth {
        address: u64,
        name: String,
        address_string: String,
    },
    Ble {
        address: u64,
        name: String,
        address_string: String,
    },
    Irda {
        address: u32,
        name: String,
    },
}

impl DeviceTransport {
    fn connect(
        &self,
        context_ptr: *mut ffi::dc_context_t,
        iostream: *mut *mut ffi::dc_iostream_t,
        userdata: *mut c_void,
    ) -> Result<()> {
        match self {
            Self::Ble { address, .. } => {
                Self::connect_ble(*address, context_ptr, iostream, userdata)?;
            }
            _ => {
                return Err(LibError::DeviceError("unsupported".into()));
            }
        }

        Ok(())
    }

    fn connect_ble(
        address: u64,
        context_ptr: *mut ffi::dc_context_t,
        iostream: *mut *mut ffi::dc_iostream_t,
        userdata: *mut c_void,
    ) -> Result<()> {
        let addr = mac_address(address);
        let status = ble::ble_packet_open(iostream, context_ptr, addr.as_ptr(), userdata);
        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::status_with_context(
                status,
                "failed to set open ble device",
            ));
        }

        Ok(())
    }

    /// Get a connection string that can be used to connect to this device
    pub fn connection_string(&self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Serial { path, .. } => Some(path.clone()),
            Self::Bluetooth { address_string, .. } | Self::Ble { address_string, .. } => {
                Some(address_string.clone())
            }
            Self::Irda { address, .. } => Some(format!("0x{:08X}", address)),
            Self::Usb { .. } | Self::UsbHid { .. } => None, // USB doesn't use string paths
        }
    }

    /// Get a human-readable name for this device
    pub fn display_name(&self) -> String {
        match self {
            Self::None => String::new(),
            Self::Serial { name, .. } => name.clone(),
            Self::Usb {
                vendor_id,
                product_id,
                ..
            } => get_usb_device_name(*vendor_id, *product_id)
                .unwrap_or_else(|| format!("USB Device {:04X}:{:04X}", vendor_id, product_id)),
            Self::UsbHid {
                vendor_id,
                product_id,
                ..
            } => get_usb_device_name(*vendor_id, *product_id)
                .unwrap_or_else(|| format!("USB HID Device {:04X}:{:04X}", vendor_id, product_id)),
            Self::Bluetooth { name, .. } | Self::Ble { name, .. } => name.clone(),
            Self::Irda { name, .. } => name.clone(),
        }
    }
}

impl fmt::Display for DeviceTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::None => write!(f, "None"),
            Self::Serial { name, path } => {
                write!(f, "Serial: {} ({})", name, path)
            }
            Self::Usb {
                vendor_id,
                product_id,
                ..
            } => {
                write!(f, "USB: {:04X}:{:04X}", vendor_id, product_id)
            }
            Self::UsbHid {
                vendor_id,
                product_id,
                ..
            } => {
                write!(f, "USB HID: {:04X}:{:04X}", vendor_id, product_id)
            }
            Self::Bluetooth {
                name,
                address_string,
                ..
            } => {
                write!(f, "Bluetooth: {} ({})", name, address_string)
            }
            Self::Ble {
                name,
                address_string,
                ..
            } => {
                write!(f, "BLE: {} ({})", name, address_string)
            }
            Self::Irda { name, address } => {
                write!(f, "IrDA: {} (0x{:08X})", name, address)
            }
        }
    }
}

// Helper function to convert hex string to bytes
fn hex_string_to_bytes(hex: &str) -> std::result::Result<Vec<u8>, std::num::ParseIntError> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect()
}

fn mac_address(address: u64) -> CString {
    let bytes = address.to_be_bytes(); // Big-endian byte order
    // Take only the last 6 bytes (MAC address is 48 bits)
    CString::new(format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
    ))
    .unwrap()
}

impl From<&DeviceTransport> for Transport {
    fn from(value: &DeviceTransport) -> Self {
        match value {
            DeviceTransport::None => Self::None,
            DeviceTransport::Serial { .. } => Self::Serial,
            DeviceTransport::Usb { .. } => Self::Usb,
            DeviceTransport::UsbHid { .. } => Self::UsbHid,
            DeviceTransport::Bluetooth { .. } => Self::Bluetooth,
            DeviceTransport::Ble { .. } => Self::Ble,
            DeviceTransport::Irda { .. } => Self::Irda,
        }
    }
}

pub struct DeviceScanner<'a> {
    context: &'a Context,
}

impl<'a> DeviceScanner<'a> {
    pub fn new(context: &'a Context) -> Self {
        DeviceScanner { context }
    }

    pub fn scan_all(&self) -> Result<Vec<DeviceTransport>> {
        let mut all_devices = Vec::new();

        if let Ok(serial_devices) = self.scan_serial_devices() {
            all_devices.extend(serial_devices);
        }

        if let Ok(usb_devices) = self.scan_usb_devices() {
            all_devices.extend(usb_devices);
        }

        if let Ok(usbhid_devices) = self.scan_usbhid_devices() {
            all_devices.extend(usbhid_devices);
        }

        if let Ok(bluetooth_devices) = self.scan_bluetooth_devices() {
            all_devices.extend(bluetooth_devices);
        }

        if let Ok(irda_devices) = self.scan_irda_devices() {
            all_devices.extend(irda_devices);
        }

        Ok(all_devices)
    }

    pub fn scan_serial_devices(&self) -> Result<Vec<DeviceTransport>> {
        let mut devices = Vec::new();
        let mut iterator = ptr::null_mut();

        let status = unsafe {
            ffi::dc_serial_iterator_new(&mut iterator, self.context.ptr, ptr::null_mut())
        };

        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::from(
                Status::try_from(status).unwrap_or(Status::Io),
            ));
        }

        loop {
            let mut device: *mut ffi::dc_serial_device_t = ptr::null_mut();
            let status = unsafe { ffi::dc_iterator_next(iterator, void_ptr!(&mut device)) };

            if status == ffi::DC_STATUS_DONE {
                break;
            }

            if status != ffi::DC_STATUS_SUCCESS {
                break;
            }

            if device.is_null() {
                continue;
            }

            let name_ptr = unsafe { ffi::dc_serial_device_get_name(device) };
            let name = if name_ptr.is_null() {
                "Unknown".to_string()
            } else {
                unsafe { CStr::from_ptr(name_ptr).to_string_lossy().to_string() }
            };

            devices.push(DeviceTransport::Serial {
                path: name.clone(),
                name: extract_device_name(&name),
            });

            unsafe { ffi::dc_serial_device_free(device) };
        }

        unsafe { ffi::dc_iterator_free(iterator) };

        Ok(devices)
    }

    pub fn scan_usb_devices(&self) -> Result<Vec<DeviceTransport>> {
        let mut devices = Vec::new();
        let mut iterator = ptr::null_mut();

        let status =
            unsafe { ffi::dc_usb_iterator_new(&mut iterator, self.context.ptr, ptr::null_mut()) };

        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::from(
                Status::try_from(status).unwrap_or(Status::Io),
            ));
        }

        loop {
            let mut device: *mut ffi::dc_usb_device_t = ptr::null_mut();
            let status = unsafe { ffi::dc_iterator_next(iterator, void_ptr!(&mut device)) };

            if status == ffi::DC_STATUS_DONE {
                break;
            }

            if status != ffi::DC_STATUS_SUCCESS {
                break;
            }

            if device.is_null() {
                continue;
            }

            let vid = unsafe { ffi::dc_usb_device_get_vid(device) };
            let pid = unsafe { ffi::dc_usb_device_get_pid(device) };

            devices.push(DeviceTransport::Usb {
                vendor_id: vid as u16,
                product_id: pid as u16,
                device_path: None,
            });

            unsafe { ffi::dc_usb_device_free(device) };
        }

        unsafe { ffi::dc_iterator_free(iterator) };

        Ok(devices)
    }

    pub fn scan_usbhid_devices(&self) -> Result<Vec<DeviceTransport>> {
        let mut devices = Vec::new();
        let mut iterator = ptr::null_mut();

        let status = unsafe {
            ffi::dc_usbhid_iterator_new(&mut iterator, self.context.ptr, ptr::null_mut())
        };

        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::from(
                Status::try_from(status).unwrap_or(Status::Io),
            ));
        }

        loop {
            let mut device: *mut ffi::dc_usbhid_device_t = ptr::null_mut();
            let status = unsafe { ffi::dc_iterator_next(iterator, void_ptr!(&mut device)) };

            if status == ffi::DC_STATUS_DONE {
                break;
            }

            if status != ffi::DC_STATUS_SUCCESS {
                break;
            }

            if device.is_null() {
                continue;
            }

            let vid = unsafe { ffi::dc_usbhid_device_get_vid(device) };
            let pid = unsafe { ffi::dc_usbhid_device_get_pid(device) };

            devices.push(DeviceTransport::UsbHid {
                vendor_id: vid as u16,
                product_id: pid as u16,
                device_path: None,
            });

            unsafe { ffi::dc_usbhid_device_free(device) };
        }

        unsafe { ffi::dc_iterator_free(iterator) };

        Ok(devices)
    }

    pub fn scan_bluetooth_devices(&self) -> Result<Vec<DeviceTransport>> {
        let mut devices = Vec::new();
        let mut iterator = ptr::null_mut();

        let status = unsafe {
            ffi::dc_bluetooth_iterator_new(&mut iterator, self.context.ptr, ptr::null_mut())
        };

        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::from(
                Status::try_from(status).unwrap_or(Status::Io),
            ));
        }

        loop {
            let mut device: *mut ffi::dc_bluetooth_device_t = ptr::null_mut();
            let status = unsafe { ffi::dc_iterator_next(iterator, void_ptr!(&mut device)) };

            if status == ffi::DC_STATUS_DONE {
                break;
            }

            if status != ffi::DC_STATUS_SUCCESS {
                break;
            }

            if device.is_null() {
                continue;
            }

            let address = unsafe { ffi::dc_bluetooth_device_get_address(device) };
            let name_ptr = unsafe { ffi::dc_bluetooth_device_get_name(device) };

            let name = if name_ptr.is_null() {
                "Unknown Bluetooth Device".to_string()
            } else {
                unsafe { CStr::from_ptr(name_ptr).to_string_lossy().to_string() }
            };

            let address_string = format_bluetooth_address(address);

            devices.push(DeviceTransport::Bluetooth {
                address,
                name,
                address_string,
            });

            unsafe { ffi::dc_bluetooth_device_free(device) };
        }

        unsafe { ffi::dc_iterator_free(iterator) };

        Ok(devices)
    }

    pub fn scan_ble_devices(&self) -> Result<Vec<DeviceTransport>> {
        BleTransport::scan(Duration::from_secs(5))
    }

    pub fn scan_irda_devices(&self) -> Result<Vec<DeviceTransport>> {
        let mut devices = Vec::new();
        let mut iterator = ptr::null_mut();

        let status =
            unsafe { ffi::dc_irda_iterator_new(&mut iterator, self.context.ptr, ptr::null_mut()) };

        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::from(
                Status::try_from(status).unwrap_or(Status::Io),
            ));
        }

        loop {
            let mut device: *mut ffi::dc_irda_device_t = ptr::null_mut();
            let status = unsafe { ffi::dc_iterator_next(iterator, void_ptr!(&mut device)) };

            if status == ffi::DC_STATUS_DONE {
                break;
            }

            if status != ffi::DC_STATUS_SUCCESS {
                break;
            }

            if device.is_null() {
                continue;
            }

            let address = unsafe { ffi::dc_irda_device_get_address(device) };
            let name_ptr = unsafe { ffi::dc_irda_device_get_name(device) };

            let name = if name_ptr.is_null() {
                "Unknown IrDA Device".to_string()
            } else {
                unsafe { CStr::from_ptr(name_ptr).to_string_lossy().to_string() }
            };

            devices.push(DeviceTransport::Irda { address, name });

            unsafe { ffi::dc_irda_device_free(device) };
        }

        unsafe { ffi::dc_iterator_free(iterator) };

        Ok(devices)
    }

    pub fn scan_transport(&self, transport: Transport) -> Result<Vec<DeviceTransport>> {
        match transport {
            Transport::Serial => self.scan_serial_devices(),
            Transport::Usb => self.scan_usb_devices(),
            Transport::UsbHid => self.scan_usbhid_devices(),
            Transport::Bluetooth => self.scan_bluetooth_devices(),
            Transport::Ble => self.scan_ble_devices(),
            Transport::Irda => self.scan_irda_devices(),
            _ => Ok(Vec::new()),
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn event_callback(
    _device: *mut ffi::dc_device_t,
    event: ffi::dc_event_type_t,
    data: *const c_void,
    userdata: *mut c_void,
) {
    let device = unsafe { cast_void!(userdata, Device::<DeviceConnected>) };

    match event {
        ffi::DC_EVENT_WAITING => {
            println!("Event: waiting for user action");
        }
        ffi::DC_EVENT_PROGRESS => {
            let progress = unsafe { &*(data as *const ffi::dc_event_progress_t) };
            device.progress.0 = progress.current;
            device.progress.1 = progress.maximum;

            println!(
                "Event: progress {:.2}% ({}/{})",
                100.0 * (progress.current as f64) / (progress.maximum as f64),
                progress.current,
                progress.maximum
            );
        }
        ffi::DC_EVENT_DEVINFO => {
            let devinfo = unsafe { &*(data as *const ffi::dc_event_devinfo_t) };
            device.computer.firmware = devinfo.firmware;
            device.computer.model = devinfo.model;
            device.computer.serial = devinfo.serial;
        }
        ffi::DC_EVENT_CLOCK => {
            let clock = unsafe { &*(data as *const ffi::dc_event_clock_t) };
            println!(
                "Event: systime={}, devtime={}",
                clock.systime, clock.devtime
            );
        }
        ffi::DC_EVENT_VENDOR => {
            let vendor = unsafe { &*(data as *const ffi::dc_event_vendor_t) };
            let mut hex_string = String::from("Event: vendor=");
            let data_slice =
                unsafe { std::slice::from_raw_parts(vendor.data, vendor.size as usize) };
            for byte in data_slice {
                hex_string.push_str(&format!("{:02X}", byte));
            }
            println!("{hex_string}");
        }
        _ => {
            // Default case - do nothing
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn dive_callback(
    data: *const c_uchar,
    size: c_uint,
    fingerprint: *const c_uchar,
    fsize: c_uint,
    userdata: *mut c_void,
) -> c_int {
    let device = unsafe { cast_void!(userdata, Device::<DeviceConnected>) };

    let data = unsafe { std::slice::from_raw_parts(data, size as usize).to_vec() };
    let fingerprint = unsafe { std::slice::from_raw_parts(fingerprint, fsize as usize).to_vec() };

    println!(
        "Downloaded {:02X}{:02X}{:02X}{:02X}",
        fingerprint[0], fingerprint[1], fingerprint[2], fingerprint[3],
    );
    match Parser::new(device, data) {
        Ok(mut parser) => {
            if let Err(err) = parser.parse(fingerprint) {
                eprintln!("{err}");
                return 0;
            }
        }
        Err(err) => {
            eprintln!("{err}");
            return 0;
        }
    };
    1
}

#[unsafe(no_mangle)]
extern "C" fn cancel_callback(userdata: *mut c_void) -> c_int {
    let device = unsafe { cast_void!(userdata, Device::<DeviceConnected>) };
    if device.cancel { return 1 } else { return 0 }
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
    if let Some(name) = path.split('/').last() {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_creation() {
        let context = Context::default();
        let scanner = DeviceScanner::new(&context);
        // Scanner should be created successfully
        drop(scanner);
    }

    #[test]
    fn test_bluetooth_address_format() {
        let address = 0x001B63041234u64;
        let formatted = format_bluetooth_address(address);
        assert_eq!(formatted, "00:1B:63:04:12:34");
    }

    #[test]
    fn test_device_info_display() {
        let device = DeviceTransport::Bluetooth {
            address: 0x001B63041234u64,
            name: "Test Device".to_string(),
            address_string: "00:1B:63:04:12:34".to_string(),
        };

        let display = format!("{}", device);
        assert!(display.contains("Bluetooth"));
        assert!(display.contains("Test Device"));
    }
}
