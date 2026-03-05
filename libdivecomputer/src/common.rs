use std::ffi::c_void;

use libdivecomputer_sys as ffi;
use serde::{Deserialize, Serialize};
use serde_repr::Deserialize_repr;

/// Cast a mutable reference to a `*mut c_void` for FFI callbacks.
#[inline]
pub(crate) fn as_void_ptr<T>(r: &mut T) -> *mut c_void {
    r as *mut T as *mut c_void
}

/// Recover a mutable reference from a `*mut c_void` FFI callback pointer.
///
/// # Safety
/// The pointer must be non-null and point to a valid, aligned `T`.
#[inline]
pub(crate) unsafe fn from_void_ptr<'a, T>(ptr: *mut c_void) -> &'a mut T {
    unsafe { &mut *(ptr as *mut T) }
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr)]
#[non_exhaustive]
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
    TTS,
}

impl std::fmt::Display for SampleKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
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
        };
        write!(f, "{s}")
    }
}

#[repr(u32)]
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
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
    GasChange = ffi::SAMPLE_EVENT_GASCHANGE,
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
    GasChange2 = ffi::SAMPLE_EVENT_GASCHANGE2,
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
            ffi::SAMPLE_EVENT_GASCHANGE => Self::GasChange,
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
            ffi::SAMPLE_EVENT_GASCHANGE2 => Self::GasChange2,
            ffi::SAMPLE_EVENT_STRING => Self::String,
            _ => Self::None,
        }
    }
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
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
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr)]
#[repr(u32)]
#[non_exhaustive]
pub enum SampleFlag {
    None = 0,
    Begin = 1 << 0,
    End = 1 << 1,
    SeverityMask = 7 << 2,
    SeverityState = 1 << 2,
    SeverityInfo = 2 << 2,
    SeverityWarn = 3 << 2,
    SeverityAlarm = 4 << 2,
    TypeMask = 7 << 5,
    TypeInterest = 1 << 5,
    TypeNavpoint = 2 << 5,
    TypeDanger = 3 << 5,
    TypeAnimal = 4 << 5,
    TypeIssue = 5 << 5,
    TypeInjury = 6 << 5,
}

const SEVERITY_SHIFT: u32 = 2;
const TYPE_SHIFT: u32 = 5;

impl std::fmt::Display for SampleFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::None => "",
            Self::Begin => "Begin",
            Self::End => "End",
            Self::SeverityMask => "SeverityMask",
            Self::SeverityState => "State",
            Self::SeverityInfo => "Info",
            Self::SeverityWarn => "Warn",
            Self::SeverityAlarm => "Alarm",
            Self::TypeMask => "TypeMask",
            Self::TypeInterest => "Interest",
            Self::TypeNavpoint => "Navpoint",
            Self::TypeDanger => "Danger",
            Self::TypeAnimal => "Animal",
            Self::TypeIssue => "Issue",
            Self::TypeInjury => "Injury",
        };
        write!(f, "{s}")
    }
}

impl From<u32> for SampleFlag {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::Begin,
            2 => Self::End,
            v if v == (7 << SEVERITY_SHIFT) => Self::SeverityMask,
            v if v == (1 << SEVERITY_SHIFT) => Self::SeverityState,
            v if v == (2 << SEVERITY_SHIFT) => Self::SeverityInfo,
            v if v == (3 << SEVERITY_SHIFT) => Self::SeverityWarn,
            v if v == (4 << SEVERITY_SHIFT) => Self::SeverityAlarm,
            v if v == (7 << TYPE_SHIFT) => Self::TypeMask,
            v if v == (1 << TYPE_SHIFT) => Self::TypeInterest,
            v if v == (2 << TYPE_SHIFT) => Self::TypeNavpoint,
            v if v == (3 << TYPE_SHIFT) => Self::TypeDanger,
            v if v == (4 << TYPE_SHIFT) => Self::TypeAnimal,
            v if v == (5 << TYPE_SHIFT) => Self::TypeIssue,
            v if v == (6 << TYPE_SHIFT) => Self::TypeInjury,
            _ => Self::None,
        }
    }
}

impl SampleFlag {
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }

    pub fn get_severity(flags: u32) -> u32 {
        (flags & (Self::SeverityMask as u32)) >> SEVERITY_SHIFT
    }

    pub fn set_severity(flags: u32, severity: u32) -> u32 {
        let cleared = flags & !(Self::SeverityMask as u32);
        cleared | ((severity & 0x7) << SEVERITY_SHIFT)
    }

    pub fn get_type(flags: u32) -> u32 {
        (flags & (Self::TypeMask as u32)) >> TYPE_SHIFT
    }

    pub fn set_type(flags: u32, type_val: u32) -> u32 {
        let cleared = flags & !(Self::TypeMask as u32);
        cleared | ((type_val & 0x7) << TYPE_SHIFT)
    }
}
