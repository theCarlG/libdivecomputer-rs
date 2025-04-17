#[repr(i32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Status {
    Success = 0,
    Done = 1,
    Unsupported = -1,
    InvalidArgs = -2,
    NoMemory = -3,
    NoDevice = -4,
    NoAccess = -5,
    Io = -6,
    Timeout = -7,
    Protocol = -8,
    DataFormat = -9,
    Cancelled = -10,
}

impl TryFrom<i32> for Status {
    type Error = String;
    fn try_from(value: i32) -> Result<Status, Self::Error> {
        let result = match value {
            0 => Self::Success,
            1 => Self::Done,
            -1 => Self::Unsupported,
            -2 => Self::InvalidArgs,
            -3 => Self::NoMemory,
            -4 => Self::NoDevice,
            -5 => Self::NoAccess,
            -6 => Self::Io,
            -7 => Self::Timeout,
            -8 => Self::Protocol,
            -9 => Self::DataFormat,
            -10 => Self::Cancelled,
            _ => return Err(format!("Invalid status: {value}")),
        };

        Ok(result)
    }
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
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
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Family {
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

            _ => Family::None, // Default for unknown values
        }
    }
}
