pub mod ble;

use std::{
    ffi::{CString, c_int, c_uchar, c_uint, c_void},
    fmt::{self, Display},
    marker::PhantomData,
    ptr,
    sync::{Arc, atomic::Ordering},
};

use libdivecomputer_sys as ffi;
use serde::{Deserialize, Serialize};
use serde_repr::Deserialize_repr;
use std::sync::mpsc;

use crate::{
    Context, DiveComputerState, DownloadProgress, c_void_as,
    descriptor::DescriptorItem,
    error::{LibError, Result},
    parser::{Dive, Parser},
    void_ptr,
};

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, Hash, Ord, PartialOrd)]
pub enum Transport {
    None = 0,
    Serial = 1 << 0,
    Usb = 1 << 1,
    UsbHid = 1 << 2,
    Irda = 1 << 3,
    Bluetooth = 1 << 4,
    Ble = 1 << 5,
}

impl Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let output = match self {
            Transport::None => "None",
            Transport::Serial => "Serial",
            Transport::Usb => "USB",
            Transport::UsbHid => "USB HID",
            Transport::Irda => "IrDA",
            Transport::Bluetooth => "Bluetooth",
            Transport::Ble => "BLE",
        };

        write!(f, "{output}")
    }
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

impl From<&String> for Transport {
    fn from(value: &String) -> Self {
        match value.as_str() {
            "Serial" => Self::Serial,
            "USB" => Self::Usb,
            "USB HID" => Self::UsbHid,
            "IrDA" => Self::Irda,
            "Bluetooth" => Self::Bluetooth,
            "BLE" => Self::Ble,
            _ => Self::None,
        }
    }
}

impl From<&str> for Transport {
    fn from(s: &str) -> Self {
        Self::from(&s.to_string())
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
#[derive(
    Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr, Default, Hash, Ord, PartialOrd,
)]
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

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Family::None => "None",
            Family::SuuntoSolution => "Suunto Solution",
            Family::SuuntoEon => "Suunto Eon",
            Family::SuuntoVyper => "Suunto Vyper",
            Family::SuuntoVyper2 => "Suunto Vyper 2",
            Family::SuuntoD9 => "Suunto D9",
            Family::SuuntoEonSteel => "Suunto Eon Steel",
            Family::ReefnetSensus => "Reefnet Sensus",
            Family::ReefnetSensusPro => "Reefnet Sensus Pro",
            Family::ReefnetSensusUltra => "Reefnet Sensus Ultra",
            Family::UwatecAladin => "Uwatec Aladin",
            Family::UwatecMemoMouse => "Uwatec Memo Mouse",
            Family::UwatecSmart => "Uwatec Smart",
            Family::UwatecMeridian => "Uwatec Meridian",
            Family::UwatecG2 => "Uwatec G2",
            Family::OceanicVtPro => "Oceanic Vt Pro",
            Family::OceanicVeo250 => "Oceanic Veo 250",
            Family::OceanicAtom2 => "Oceanic Atom 2",
            Family::MaresNemo => "Mares Nemo",
            Family::MaresPuck => "Mares Puck",
            Family::MaresDarwin => "Mares Darwin",
            Family::MaresIconHD => "Mares Icon HD",
            Family::HwOstc => "Hw Ostc",
            Family::HwFrog => "Hw Frog",
            Family::HwOstc3 => "Hw Ostc 3",
            Family::CressiEdy => "Cressi Edy",
            Family::CressiLeonardo => "Cressi Leonardo",
            Family::CressiGoa => "Cressi Goa",
            Family::ZeagleN2ition3 => "Zeagle N2ition 3",
            Family::AtomicsCobalt => "Atomics Cobalt",
            Family::ShearwaterPredator => "Shearwater Predator",
            Family::ShearwaterPetrel => "Shearwater Petrel",
            Family::DiveRiteNitekQ => "Dive Rite Nitek Q",
            Family::CitizenAqualand => "Citizen Aqualand",
            Family::DiveSystemIDive => "Dive System I Dive",
            Family::CochranCommander => "Cochran Commander",
            Family::TecdivingDivecomputerEu => "Tecdiving Divecomputer Eu",
            Family::McLeanExtreme => "Mc Lean Extreme",
            Family::LiquivisionLynx => "Liquivision Lynx",
            Family::SporasubSp2 => "Sporasub Sp 2",
            Family::DeepSixExcursion => "Deep Six Excursion",
            Family::SeacScreen => "Seac Screen",
            Family::DeepbluCosmiq => "Deepblu Cosmiq",
            Family::OceansS1 => "Oceans S1",
            Family::DivesoftFreedom => "Divesoft Freedom",
            Family::HalcyonSymbios => "Halcyon Symbios",
        };
        write!(f, "{}", s)
    }
}

impl From<&String> for Family {
    fn from(s: &String) -> Self {
        match s.as_str() {
            "None" => Family::None,
            "Suunto Solution" => Family::SuuntoSolution,
            "Suunto Eon" => Family::SuuntoEon,
            "Suunto Vyper" => Family::SuuntoVyper,
            "Suunto Vyper 2" => Family::SuuntoVyper2,
            "Suunto D9" => Family::SuuntoD9,
            "Suunto Eon Steel" => Family::SuuntoEonSteel,
            "Reefnet Sensus" => Family::ReefnetSensus,
            "Reefnet Sensus Pro" => Family::ReefnetSensusPro,
            "Reefnet Sensus Ultra" => Family::ReefnetSensusUltra,
            "Uwatec Aladin" => Family::UwatecAladin,
            "Uwatec Memo Mouse" => Family::UwatecMemoMouse,
            "Uwatec Smart" => Family::UwatecSmart,
            "Uwatec Meridian" => Family::UwatecMeridian,
            "Uwatec G2" => Family::UwatecG2,
            "Oceanic Vt Pro" => Family::OceanicVtPro,
            "Oceanic Veo 250" => Family::OceanicVeo250,
            "Oceanic Atom 2" => Family::OceanicAtom2,
            "Mares Nemo" => Family::MaresNemo,
            "Mares Puck" => Family::MaresPuck,
            "Mares Darwin" => Family::MaresDarwin,
            "Mares Icon HD" => Family::MaresIconHD,
            "Hw Ostc" => Family::HwOstc,
            "Hw Frog" => Family::HwFrog,
            "Hw Ostc 3" => Family::HwOstc3,
            "Cressi Edy" => Family::CressiEdy,
            "Cressi Leonardo" => Family::CressiLeonardo,
            "Cressi Goa" => Family::CressiGoa,
            "Zeagle N2ition 3" => Family::ZeagleN2ition3,
            "Atomics Cobalt" => Family::AtomicsCobalt,
            "Shearwater Predator" => Family::ShearwaterPredator,
            "Shearwater Petrel" => Family::ShearwaterPetrel,
            "Dive Rite Nitek Q" => Family::DiveRiteNitekQ,
            "Citizen Aqualand" => Family::CitizenAqualand,
            "Dive System I Dive" => Family::DiveSystemIDive,
            "Cochran Commander" => Family::CochranCommander,
            "Tecdiving Divecomputer Eu" => Family::TecdivingDivecomputerEu,
            "Mc Lean Extreme" => Family::McLeanExtreme,
            "Liquivision Lynx" => Family::LiquivisionLynx,
            "Sporasub Sp 2" => Family::SporasubSp2,
            "Deep Six Excursion" => Family::DeepSixExcursion,
            "Seac Screen" => Family::SeacScreen,
            "Deepblu Cosmiq" => Family::DeepbluCosmiq,
            "Oceans S1" => Family::OceansS1,
            "Divesoft Freedom" => Family::DivesoftFreedom,
            "Halcyon Symbios" => Family::HalcyonSymbios,
            _ => Family::None, // Default for unknown strings
        }
    }
}

impl From<&str> for Family {
    fn from(s: &str) -> Self {
        Self::from(&s.to_string())
    }
}

impl From<u32> for Family {
    fn from(value: u32) -> Self {
        match value {
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
    pub(crate) transport: ConnectionInfo,
}

impl Default for DeviceData {
    fn default() -> Self {
        Self {
            dives: Vec::new(),
            transport: ConnectionInfo::None,
        }
    }
}

pub trait DeviceState {}

pub struct DeviceConnected;
impl DeviceState for DeviceConnected {}

pub struct DeviceDisconnected;
impl DeviceState for DeviceDisconnected {}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub transport: Transport,
    pub connection_info: ConnectionInfo,
}

#[derive(Debug, Clone)]
pub struct Device<T: DeviceState> {
    pub(crate) ptr: *mut ffi::dc_device_t,
    pub(crate) item: DescriptorItem,
    pub(crate) context: Arc<Context>,

    iostream: *mut ffi::dc_iostream_t,

    pub(crate) data: DeviceData,
    connection_info: ConnectionInfo,

    pub(crate) tx: mpsc::Sender<Dive>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    state: Arc<std::sync::RwLock<DiveComputerState>>,

    model: u32,
    firmware: u32,
    serial: u32,

    _panthom: PhantomData<T>,
}

unsafe impl<T: DeviceState> Send for Device<T> {}
unsafe impl<T: DeviceState> Sync for Device<T> {}

impl Device<DeviceDisconnected> {
    pub fn new(
        context: &Arc<Context>,
        connection_info: &ConnectionInfo,
        item: DescriptorItem,
        tx: mpsc::Sender<Dive>,
        cancel: Arc<std::sync::atomic::AtomicBool>,
        state: Arc<std::sync::RwLock<DiveComputerState>>,
    ) -> Result<Self> {
        Ok(Self {
            ptr: ptr::null_mut(),
            context: context.clone(),
            connection_info: connection_info.clone(),
            item,
            iostream: ptr::null_mut(),
            data: DeviceData::default(),
            model: 0,
            firmware: 0,
            serial: 0,
            tx,
            cancel,
            state,
            _panthom: PhantomData,
        })
    }

    pub async fn connect(mut self) -> Result<Device<DeviceConnected>> {
        self.data.transport = self.connection_info.clone();

        self.connection_info
            .connect(self.context.ptr, &mut self.iostream)
            .await?;

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

            let new_self = Device::<DeviceConnected> {
                ptr: self.ptr,
                item: self.item,
                context: self.context,
                iostream: self.iostream,
                model: self.model,
                firmware: self.firmware,
                serial: self.serial,
                tx: self.tx,
                state: self.state,
                data: self.data,
                connection_info: self.connection_info,
                cancel: self.cancel,
                _panthom: PhantomData,
            };
            Ok(new_self)
        }
    }
}

impl Device<DeviceConnected> {
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
    pub fn start_download(&mut self) -> Result<()> {
        unsafe {
            let events = ffi::DC_EVENT_WAITING
                | ffi::DC_EVENT_PROGRESS
                | ffi::DC_EVENT_DEVINFO
                | ffi::DC_EVENT_CLOCK
                | ffi::DC_EVENT_VENDOR;

            let status =
                ffi::dc_device_set_events(self.ptr, events, Some(event_callback), void_ptr!(self));
            if status != ffi::DC_STATUS_SUCCESS {
                return Err(LibError::status_with_context(
                    status,
                    "failed to set event handler",
                ));
            }

            let status =
                ffi::dc_device_set_cancel(self.ptr, Some(cancel_callback), void_ptr!(self));
            if status != ffi::DC_STATUS_SUCCESS {
                return Err(LibError::status_with_context(
                    status,
                    "failed to set cancel callback",
                ));
            }
            let status = ffi::dc_device_foreach(self.ptr, Some(dive_callback), void_ptr!(self));
            if status != ffi::DC_STATUS_SUCCESS {
                return Err(LibError::status_with_context(
                    status,
                    "failed to set foreach dive",
                ));
            }
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
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConnectionInfo {
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
        local_name: Option<String>,
        service_name: String,
        address_string: String,
    },
    Irda {
        address: u32,
        name: String,
    },
}

impl ConnectionInfo {
    async fn connect(
        &self,
        context_ptr: *mut ffi::dc_context_t,
        iostream: *mut *mut ffi::dc_iostream_t,
    ) -> Result<()> {
        match self {
            Self::Ble { address, .. } => {
                Self::connect_ble(*address, context_ptr, iostream).await?;
            }
            _ => {
                return Err(LibError::DeviceError("unsupported".into()));
            }
        }

        Ok(())
    }

    async fn connect_ble(
        address: u64,
        context_ptr: *mut ffi::dc_context_t,
        iostream: *mut *mut ffi::dc_iostream_t,
    ) -> Result<()> {
        let addr = mac_address(address);
        let status = ble::ble_packet_open(iostream, context_ptr, addr.as_ptr()).await;
        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::status_with_context(
                status,
                format!("failed to set open ble device: {}", addr.to_string_lossy()),
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
            Self::Irda { address, .. } => Some(format!("0x{address:08X}")),
            Self::Usb { .. } | Self::UsbHid { .. } => None, // USB doesn't use string paths
        }
    }

    /// Get a human-readable name for this device
    pub fn display_name(&self) -> String {
        match self {
            Self::None => "None".to_string(),
            Self::Serial { name, .. } => name.clone(),
            Self::Usb {
                vendor_id,
                product_id,
                ..
            } => get_usb_device_name(*vendor_id, *product_id)
                .unwrap_or_else(|| format!("USB Device {vendor_id:04X}:{product_id:04X}")),
            Self::UsbHid {
                vendor_id,
                product_id,
                ..
            } => get_usb_device_name(*vendor_id, *product_id)
                .unwrap_or_else(|| format!("USB HID Device {vendor_id:04X}:{product_id:04X}")),
            Self::Bluetooth { name, .. } => name.clone(),
            Self::Ble {
                local_name,
                service_name,
                ..
            } => local_name
                .clone()
                .map(|name| format!("{name} - {service_name}"))
                .unwrap_or(service_name.to_string()),
            Self::Irda { name, .. } => name.clone(),
        }
    }
}

impl fmt::Display for ConnectionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// Helper function to convert hex string to bytes
pub fn hex_string_to_bytes(hex: &str) -> std::result::Result<Vec<u8>, std::num::ParseIntError> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
        .collect()
}

pub fn bytes_to_hex(data: &Vec<u8>) -> String {
    let mut hex_string = String::new();
    for byte in data {
        hex_string.push_str(&format!("{byte:02X}"));
    }
    hex_string
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

impl From<&ConnectionInfo> for Transport {
    fn from(value: &ConnectionInfo) -> Self {
        match value {
            ConnectionInfo::None => Self::None,
            ConnectionInfo::Serial { .. } => Self::Serial,
            ConnectionInfo::Usb { .. } => Self::Usb,
            ConnectionInfo::UsbHid { .. } => Self::UsbHid,
            ConnectionInfo::Bluetooth { .. } => Self::Bluetooth,
            ConnectionInfo::Ble { .. } => Self::Ble,
            ConnectionInfo::Irda { .. } => Self::Irda,
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
    let device = unsafe { c_void_as!(userdata, Device::<DeviceConnected>) };

    match event {
        ffi::DC_EVENT_WAITING => {
            *device.state.write().unwrap() = DiveComputerState::WaitingForUser;
            // println!("Event: waiting for user action");
        }
        ffi::DC_EVENT_PROGRESS => {
            let progress = unsafe { &*(data as *const ffi::dc_event_progress_t) };
            *device.state.write().unwrap() =
                if progress.current < progress.maximum && !device.cancel.load(Ordering::Relaxed) {
                    DiveComputerState::Downloading {
                        progress: DownloadProgress {
                            current: progress.current,
                            total: progress.maximum,
                        },
                        current_task: None,
                        device: device.item.product(),
                    }
                } else {
                    DiveComputerState::Idle
                };

            // println!(
            //     "Event: progress {:.2}% ({}/{})",
            //     100.0 * (progress.current as f64) / (progress.maximum as f64),
            //     progress.current,
            //     progress.maximum
            // );
        }
        ffi::DC_EVENT_DEVINFO => {
            let devinfo = unsafe { &*(data as *const ffi::dc_event_devinfo_t) };
            device.firmware = devinfo.firmware;
            device.model = devinfo.model;
            device.serial = devinfo.serial;

            // println!(
            //     "Event Clock: Firmware: {}, Serial: {}, Model: {}",
            //     device.firmware, device.serial, device.model
            // );
        }
        ffi::DC_EVENT_CLOCK => {
            // let clock = unsafe { &*(data as *const ffi::dc_event_clock_t) };
            // println!(
            //     "Event: systime={}, devtime={}",
            //     clock.systime, clock.devtime
            // );
        }
        ffi::DC_EVENT_VENDOR => {
            let vendor = unsafe { &*(data as *const ffi::dc_event_vendor_t) };
            let mut hex_string = String::from("Event: vendor=");
            let data_slice =
                unsafe { std::slice::from_raw_parts(vendor.data, vendor.size as usize) };
            for byte in data_slice {
                hex_string.push_str(&format!("{byte:02X}"));
            }
            println!("Vendor: {hex_string}");
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
    let device = unsafe { c_void_as!(userdata, Device::<DeviceConnected>) };

    let data = unsafe { std::slice::from_raw_parts(data, size as usize).to_vec() };
    let fingerprint = unsafe { std::slice::from_raw_parts(fingerprint, fsize as usize).to_vec() };

    let mut parser = match Parser::new(device, data) {
        Ok(parser) => parser,
        Err(err) => {
            eprintln!("{err:?}");
            return 0;
        }
    };

    match parser.parse(fingerprint) {
        Ok(dive) => {
            if let Err(err) = device.tx.send(dive) {
                eprintln!("{err:?}");
                return 0;
            }
        }
        Err(err) => {
            eprintln!("{err:?}");
            return 0;
        }
    }

    1
}

#[unsafe(no_mangle)]
extern "C" fn cancel_callback(userdata: *mut c_void) -> c_int {
    let device = unsafe { c_void_as!(userdata, Device::<DeviceConnected>) };
    if device.cancel.load(Ordering::Relaxed) {
        1
    } else {
        0
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
    fn test_device_info_display() {
        let device = ConnectionInfo::Bluetooth {
            address: 0x001B63041234u64,
            name: "Test Device".to_string(),
            address_string: "00:1B:63:04:12:34".to_string(),
        };

        let display = format!("{device}");
        assert!(display.contains("Bluetooth"));
        assert!(display.contains("Test Device"));
    }
}
