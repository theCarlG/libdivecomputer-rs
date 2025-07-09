use libdivecomputer_sys as ffi;
use serde::Serialize;
use serde_repr::Deserialize_repr;

#[macro_export]
macro_rules! void_ptr {
    ($s:expr) => {
        $s as *mut _ as *mut c_void
    };
}

#[macro_export]
macro_rules! c_void_as {
    ($s:expr, $t:ty) => {
        &mut *($s as *mut $t)
    };
}

#[repr(i32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr)]
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

impl TryFrom<u32> for Status {
    type Error = String;

    fn try_from(value: u32) -> Result<Status, Self::Error> {
        Self::try_from(value as i32)
    }
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
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr)]
pub enum SampleKind {
    Time,
    Depth,
    Pressure,
    Temperature,
    Event,
    Rbt,
    Heartbeat,
    Bearing,
    Vendor,
    Setpoint,
    Ppo2,
    Cns,
    Deco,
    Gasmix,
    O2sensor,

    // From Subsurface
    TTS,
}

impl std::fmt::Display for SampleKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Time => "Time",
                Self::Depth => "Depth",
                Self::Pressure => "Pressure",
                Self::Temperature => "Temperature",
                Self::Event => "Event",
                Self::Rbt => "RBT",
                Self::Heartbeat => "Heartbeat",
                Self::Bearing => "Bearing",
                Self::Vendor => "Vendor",
                Self::Setpoint => "Setpoint",
                Self::Ppo2 => "PPO2",
                Self::Cns => "CNS",
                Self::Deco => "Deco",
                Self::Gasmix => "Gasmix",
                Self::O2sensor => "O2 Sensor",
                Self::TTS => "TTS",
            }
        )
    }
}

#[repr(u32)]
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr)]
pub enum EventKind {
    #[default]
    None = ffi::SAMPLE_EVENT_NONE,
    DecoStop = ffi::SAMPLE_EVENT_DECOSTOP,
    Rbt = ffi::SAMPLE_EVENT_RBT,
    Ascent = ffi::SAMPLE_EVENT_ASCENT,
    Ceiling = ffi::SAMPLE_EVENT_CEILING,
    Workload = ffi::SAMPLE_EVENT_WORKLOAD,
    Transmitter = ffi::SAMPLE_EVENT_TRANSMITTER,
    Violation = ffi::SAMPLE_EVENT_VIOLATION,
    Bookmark = ffi::SAMPLE_EVENT_BOOKMARK,
    Surface = ffi::SAMPLE_EVENT_SURFACE,
    SafetyStop = ffi::SAMPLE_EVENT_SAFETYSTOP,
    GasChange = ffi::SAMPLE_EVENT_GASCHANGE, // Deprecated: replaced by SampleKind::Gasmix
    SafetyStopVoluntary = ffi::SAMPLE_EVENT_SAFETYSTOP_VOLUNTARY,
    SafetyStopMandatory = ffi::SAMPLE_EVENT_SAFETYSTOP_MANDATORY,
    DeepStop = ffi::SAMPLE_EVENT_DEEPSTOP,
    CeilingSafetyStop = ffi::SAMPLE_EVENT_CEILING_SAFETYSTOP,
    Floor = ffi::SAMPLE_EVENT_FLOOR,
    DiveTime = ffi::SAMPLE_EVENT_DIVETIME,
    MaxDepth = ffi::SAMPLE_EVENT_MAXDEPTH,
    Olf = ffi::SAMPLE_EVENT_OLF,
    Po2 = ffi::SAMPLE_EVENT_PO2,
    AirTime = ffi::SAMPLE_EVENT_AIRTIME,
    Rgbm = ffi::SAMPLE_EVENT_RGBM,
    Heading = ffi::SAMPLE_EVENT_HEADING,
    TissueLevel = ffi::SAMPLE_EVENT_TISSUELEVEL,
    GasChange2 = ffi::SAMPLE_EVENT_GASCHANGE2, // Deprecated: replaced by SampleKind::Gasmix

    // From Subsurface
    String = ffi::SAMPLE_EVENT_STRING,
}

impl From<u32> for EventKind {
    fn from(value: u32) -> Self {
        match value {
            ffi::SAMPLE_EVENT_DECOSTOP => Self::DecoStop,
            ffi::SAMPLE_EVENT_RBT => Self::Rbt,
            ffi::SAMPLE_EVENT_ASCENT => Self::Ascent,
            ffi::SAMPLE_EVENT_CEILING => Self::Ceiling,
            ffi::SAMPLE_EVENT_WORKLOAD => Self::Workload,
            ffi::SAMPLE_EVENT_TRANSMITTER => Self::Transmitter,
            ffi::SAMPLE_EVENT_VIOLATION => Self::Violation,
            ffi::SAMPLE_EVENT_BOOKMARK => Self::Bookmark,
            ffi::SAMPLE_EVENT_SURFACE => Self::Surface,
            ffi::SAMPLE_EVENT_SAFETYSTOP => Self::SafetyStop,
            ffi::SAMPLE_EVENT_GASCHANGE => Self::GasChange, // Deprecated: replaced by SampleKind::Gasmix
            ffi::SAMPLE_EVENT_SAFETYSTOP_VOLUNTARY => Self::SafetyStopVoluntary,
            ffi::SAMPLE_EVENT_SAFETYSTOP_MANDATORY => Self::SafetyStopMandatory,
            ffi::SAMPLE_EVENT_DEEPSTOP => Self::DeepStop,
            ffi::SAMPLE_EVENT_FLOOR => Self::Floor,
            ffi::SAMPLE_EVENT_DIVETIME => Self::DiveTime,
            ffi::SAMPLE_EVENT_MAXDEPTH => Self::MaxDepth,
            ffi::SAMPLE_EVENT_OLF => Self::Olf,
            ffi::SAMPLE_EVENT_PO2 => Self::Po2,
            ffi::SAMPLE_EVENT_AIRTIME => Self::AirTime,
            ffi::SAMPLE_EVENT_RGBM => Self::Rgbm,
            ffi::SAMPLE_EVENT_HEADING => Self::Heading,
            ffi::SAMPLE_EVENT_TISSUELEVEL => Self::TissueLevel,
            ffi::SAMPLE_EVENT_GASCHANGE2 => Self::GasChange2, // Deprecated: replaced by SampleKind::Gasmix
            ffi::SAMPLE_EVENT_STRING => Self::String,

            _ => Self::None,
        }
    }
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::None => "",
                Self::DecoStop => "Deco Stop",
                Self::Rbt => "RBT",
                Self::Ascent => "Ascent",
                Self::Ceiling => "Ceiling",
                Self::Workload => "Workload",
                Self::Transmitter => "Transmitter",
                Self::Violation => "Violation",
                Self::Bookmark => "Bookmark",
                Self::Surface => "Surface",
                Self::SafetyStop => "Safety Stop",
                Self::GasChange => "Gas Change",
                Self::SafetyStopVoluntary => "Safety Stop Voluntary",
                Self::SafetyStopMandatory => "Safety Stop Mandatory",
                Self::DeepStop => "Deep Stop",
                Self::CeilingSafetyStop => "Ceiling Safety Stop",
                Self::Floor => "Floor",
                Self::DiveTime => "Dive Time",
                Self::MaxDepth => "Max Depth",
                Self::Olf => "OLF",
                Self::Po2 => "PO2",
                Self::AirTime => "Air Time",
                Self::Rgbm => "RGBM",
                Self::Heading => "Heading",
                Self::TissueLevel => "Tissue Level",
                Self::GasChange2 => "Gas change2",
                Self::String => "String",
            }
        )
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr)]
#[repr(u32)]
pub enum SampleFlag {
    None = 0,
    Begin = 1 << 0,
    End = 1 << 1,

    // Severity flags with mask
    SeverityMask = 7 << 2,
    SeverityState = 1 << 2,
    SeverityInfo = 2 << 2,
    SeverityWarn = 3 << 2,
    SeverityAlarm = 4 << 2,

    // Type flags with mask
    TypeMask = 7 << 5,
    TypeInterest = 1 << 5,
    TypeNavpoint = 2 << 5,
    TypeDanger = 3 << 5,
    TypeAnimal = 4 << 5,
    TypeIssue = 5 << 5,
    TypeInjury = 6 << 5,
}

// Constants for shifts (these can't be inside the enum)
pub const SEVERITY_SHIFT: u32 = 2;
pub const TYPE_SHIFT: u32 = 5;

impl std::fmt::Display for SampleFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::None => "",
                Self::Begin => "Begin",
                Self::End => "End",

                // Severity flags with mask from Subsurface
                Self::SeverityMask => "SeverityMask",
                Self::SeverityState => "State",
                Self::SeverityInfo => "Info",
                Self::SeverityWarn => "Warn",
                Self::SeverityAlarm => "Alarm",

                // Type flags with mask from Subsurface
                Self::TypeMask => "TypeMask",
                Self::TypeInterest => "Interest",
                Self::TypeNavpoint => "Navpoint",
                Self::TypeDanger => "Danger",
                Self::TypeAnimal => "Animal",
                Self::TypeIssue => "Issue",
                Self::TypeInjury => "Injury",
            }
        )
    }
}

impl From<u32> for SampleFlag {
    fn from(value: u32) -> Self {
        if value == 1 {
            Self::Begin
        } else if value == 2 {
            Self::End

        // Severity flags with mask
        } else if value == (7 << SEVERITY_SHIFT) {
            Self::SeverityMask
        } else if value == (1 << SEVERITY_SHIFT) {
            Self::SeverityState
        } else if value == (2 << SEVERITY_SHIFT) {
            Self::SeverityInfo
        } else if value == (3 << SEVERITY_SHIFT) {
            Self::SeverityWarn
        } else if value == (4 << SEVERITY_SHIFT) {
            Self::SeverityAlarm

        // Type flags with mask
        } else if value == (7 << TYPE_SHIFT) {
            Self::TypeMask
        } else if value == (1 << TYPE_SHIFT) {
            Self::TypeInterest
        } else if value == (2 << TYPE_SHIFT) {
            Self::TypeNavpoint
        } else if value == (3 << TYPE_SHIFT) {
            Self::TypeDanger
        } else if value == (4 << TYPE_SHIFT) {
            Self::TypeAnimal
        } else if value == (5 << TYPE_SHIFT) {
            Self::TypeIssue
        } else if value == (6 << TYPE_SHIFT) {
            Self::TypeInjury
        } else {
            Self::None
        }
    }
}

// Helper functions for working with the enum and flags
impl SampleFlag {
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }

    // Get the severity value (shifted right to get the actual value)
    pub fn get_severity(flags: u32) -> u32 {
        (flags & (Self::SeverityMask as u32)) >> SEVERITY_SHIFT
    }

    // Set the severity value (applies the shift)
    pub fn set_severity(flags: u32, severity: u32) -> u32 {
        // Clear the severity bits
        let cleared = flags & !(Self::SeverityMask as u32);
        // Apply the new severity
        cleared | ((severity & 0x7) << SEVERITY_SHIFT)
    }

    // Get the type value (shifted right to get the actual value)
    pub fn get_type(flags: u32) -> u32 {
        (flags & (Self::TypeMask as u32)) >> TYPE_SHIFT
    }

    // Set the type value (applies the shift)
    pub fn set_type(flags: u32, type_val: u32) -> u32 {
        // Clear the type bits
        let cleared = flags & !(Self::TypeMask as u32);
        // Apply the new type
        cleared | ((type_val & 0x7) << TYPE_SHIFT)
    }
}
