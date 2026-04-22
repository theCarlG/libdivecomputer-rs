use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

use bitflags::bitflags;
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

/// Run `f` inside a panic guard, returning `R::default()` if it unwinds.
///
/// Used at every `extern "C"` boundary: unwinding into C is undefined
/// behaviour, so any user-supplied closure we call must have its panics
/// swallowed. `AssertUnwindSafe` is appropriate here because the callback
/// closure is the only thing being caught — there is no shared state that
/// could be observed in a broken state by a later Rust caller.
#[inline]
pub(crate) fn ffi_guard<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
    R: Default,
{
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_default()
}

/// Which field of a dive sample a given reading populates. A single
/// [`DiveSample`](crate::parser::DiveSample) may carry multiple readings of
/// different kinds.
#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr)]
#[non_exhaustive]
pub enum SampleKind {
    /// Elapsed time since the start of the dive.
    Time,
    /// Instantaneous depth.
    Depth,
    /// Tank pressure reading from a specific sensor.
    Pressure,
    /// Water temperature.
    Temperature,
    /// An event marker (see [`EventKind`]).
    Event,
    /// Remaining bottom time, in minutes.
    Rbt,
    /// Heart-rate reading.
    Heartbeat,
    /// Compass bearing.
    Bearing,
    /// Vendor-specific opaque blob.
    Vendor,
    /// Rebreather setpoint (target PPO2).
    Setpoint,
    /// Actual measured partial pressure of O2.
    Ppo2,
    /// Central nervous system oxygen toxicity load, in percent.
    Cns,
    /// Decompression state: NDL/TTS/ceiling/stop info.
    Deco,
    /// Active gas mix index.
    Gasmix,
    /// Individual O2 sensor cell reading (multi-cell rebreathers).
    O2sensor,
    /// Time to surface, in minutes.
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

/// Event markers emitted by the dive computer during a dive — warnings, alarms,
/// user bookmarks, gas changes, and so on. Paired with a
/// [`SampleFlag`] severity/type on the sample.
#[repr(u32)]
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum EventKind {
    /// No event / placeholder for unknown codes.
    #[default]
    None = ffi::SAMPLE_EVENT_NONE,
    /// Decompression stop required.
    DecoStop = ffi::SAMPLE_EVENT_DECOSTOP,
    /// Remaining bottom time warning.
    Rbt = ffi::SAMPLE_EVENT_RBT,
    /// Ascent rate alarm.
    Ascent = ffi::SAMPLE_EVENT_ASCENT,
    /// Ceiling violation (ascent past deco ceiling).
    Ceiling = ffi::SAMPLE_EVENT_CEILING,
    /// Workload / exertion warning.
    Workload = ffi::SAMPLE_EVENT_WORKLOAD,
    /// Tank-pressure transmitter state change.
    Transmitter = ffi::SAMPLE_EVENT_TRANSMITTER,
    /// Safety violation (generic).
    Violation = ffi::SAMPLE_EVENT_VIOLATION,
    /// User-placed bookmark.
    Bookmark = ffi::SAMPLE_EVENT_BOOKMARK,
    /// Surface interval marker.
    Surface = ffi::SAMPLE_EVENT_SURFACE,
    /// Safety stop required or in progress.
    SafetyStop = ffi::SAMPLE_EVENT_SAFETYSTOP,
    /// Gas change (legacy variant — see [`Self::GasChange2`]).
    GasChange = ffi::SAMPLE_EVENT_GASCHANGE,
    /// Voluntary safety stop.
    SafetyStopVoluntary = ffi::SAMPLE_EVENT_SAFETYSTOP_VOLUNTARY,
    /// Mandatory safety stop.
    SafetyStopMandatory = ffi::SAMPLE_EVENT_SAFETYSTOP_MANDATORY,
    /// Deep stop reached.
    DeepStop = ffi::SAMPLE_EVENT_DEEPSTOP,
    /// Safety stop ceiling.
    CeilingSafetyStop = ffi::SAMPLE_EVENT_CEILING_SAFETYSTOP,
    /// Floor (deepest allowable) depth.
    Floor = ffi::SAMPLE_EVENT_FLOOR,
    /// Dive time alarm.
    DiveTime = ffi::SAMPLE_EVENT_DIVETIME,
    /// Max depth alarm.
    MaxDepth = ffi::SAMPLE_EVENT_MAXDEPTH,
    /// OLF (oxygen limit fraction) warning.
    Olf = ffi::SAMPLE_EVENT_OLF,
    /// PO2 (oxygen partial pressure) alarm.
    Po2 = ffi::SAMPLE_EVENT_PO2,
    /// Remaining air time warning.
    AirTime = ffi::SAMPLE_EVENT_AIRTIME,
    /// RGBM decompression model state.
    Rgbm = ffi::SAMPLE_EVENT_RGBM,
    /// Compass heading update.
    Heading = ffi::SAMPLE_EVENT_HEADING,
    /// Tissue saturation level indicator.
    TissueLevel = ffi::SAMPLE_EVENT_TISSUELEVEL,
    /// Gas change (extended variant with explicit o2/he percentages).
    GasChange2 = ffi::SAMPLE_EVENT_GASCHANGE2,
    /// Free-form string annotation attached by the computer.
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

const SEVERITY_SHIFT: u32 = 2;
const TYPE_SHIFT: u32 = 5;

bitflags! {
    /// Sample event flags. These are bitmask values that can be combined.
    ///
    /// Encodes three fields: a begin/end marker, a severity level
    /// (3 bits, accessors: [`SampleFlag::severity`] / [`SampleFlag::with_severity`]),
    /// and an event type (3 bits, accessors: [`SampleFlag::event_type`] /
    /// [`SampleFlag::with_event_type`]).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SampleFlag: u32 {
        /// Event is starting.
        const BEGIN = 1 << 0;
        /// Event is ending.
        const END = 1 << 1;
        /// Bitmask for the severity field.
        const SEVERITY_MASK = 7 << SEVERITY_SHIFT;
        /// Informational state, no user action required.
        const SEVERITY_STATE = 1 << SEVERITY_SHIFT;
        /// Informational.
        const SEVERITY_INFO = 2 << SEVERITY_SHIFT;
        /// Warning.
        const SEVERITY_WARN = 3 << SEVERITY_SHIFT;
        /// Alarm — diver should act.
        const SEVERITY_ALARM = 4 << SEVERITY_SHIFT;
        /// Bitmask for the event-type field.
        const TYPE_MASK = 7 << TYPE_SHIFT;
        /// Point of interest.
        const TYPE_INTEREST = 1 << TYPE_SHIFT;
        /// Navigation point.
        const TYPE_NAVPOINT = 2 << TYPE_SHIFT;
        /// Hazard.
        const TYPE_DANGER = 3 << TYPE_SHIFT;
        /// Animal sighting.
        const TYPE_ANIMAL = 4 << TYPE_SHIFT;
        /// Equipment or procedural issue.
        const TYPE_ISSUE = 5 << TYPE_SHIFT;
        /// Injury.
        const TYPE_INJURY = 6 << TYPE_SHIFT;
    }
}

impl SampleFlag {
    /// Extract the 3-bit severity value.
    #[must_use]
    pub fn severity(self) -> u32 {
        (self & Self::SEVERITY_MASK).bits() >> SEVERITY_SHIFT
    }

    /// Return flags with the severity field replaced. Only the low 3 bits of
    /// `severity` are used; higher bits are silently masked.
    #[must_use]
    pub fn with_severity(self, severity: u32) -> Self {
        let cleared = self & !Self::SEVERITY_MASK;
        cleared | Self::from_bits_truncate((severity & 0x7) << SEVERITY_SHIFT)
    }

    /// Extract the 3-bit event-type value.
    #[must_use]
    pub fn event_type(self) -> u32 {
        (self & Self::TYPE_MASK).bits() >> TYPE_SHIFT
    }

    /// Return flags with the event-type field replaced. Only the low 3 bits of
    /// `type_val` are used.
    #[must_use]
    pub fn with_event_type(self, type_val: u32) -> Self {
        let cleared = self & !Self::TYPE_MASK;
        cleared | Self::from_bits_truncate((type_val & 0x7) << TYPE_SHIFT)
    }
}

impl std::fmt::Display for SampleFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        bitflags::parser::to_writer(self, f)
    }
}

impl From<u32> for SampleFlag {
    fn from(value: u32) -> Self {
        Self::from_bits_truncate(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_flag_combine() {
        let flags = SampleFlag::BEGIN | SampleFlag::SEVERITY_WARN;
        assert!(flags.contains(SampleFlag::BEGIN));
        assert!(flags.contains(SampleFlag::SEVERITY_WARN));
        assert!(!flags.contains(SampleFlag::END));
    }

    #[test]
    fn sample_flag_severity() {
        let flags = SampleFlag::SEVERITY_WARN;
        assert_eq!(flags.severity(), 3);

        let flags = SampleFlag::SEVERITY_ALARM;
        assert_eq!(flags.severity(), 4);

        assert_eq!(SampleFlag::empty().severity(), 0);
    }

    #[test]
    fn sample_flag_with_severity() {
        let flags = SampleFlag::BEGIN | SampleFlag::SEVERITY_INFO;
        let updated = flags.with_severity(4); // ALARM (0b100 << 2), no overlap with INFO (0b010 << 2)
        assert!(updated.contains(SampleFlag::BEGIN));
        assert_eq!(updated.severity(), 4);
        // Severity field should be exactly ALARM now, not INFO
        assert_eq!(
            updated & SampleFlag::SEVERITY_MASK,
            SampleFlag::SEVERITY_ALARM
        );
    }

    #[test]
    fn sample_flag_event_type() {
        let flags = SampleFlag::TYPE_DANGER;
        assert_eq!(flags.event_type(), 3);

        assert_eq!(SampleFlag::empty().event_type(), 0);
    }

    #[test]
    fn sample_flag_with_event_type() {
        let flags = SampleFlag::END | SampleFlag::TYPE_INTEREST;
        let updated = flags.with_event_type(4); // ANIMAL
        assert!(updated.contains(SampleFlag::END));
        assert_eq!(updated.event_type(), 4);
        assert!(!updated.contains(SampleFlag::TYPE_INTEREST));
    }

    #[test]
    fn sample_flag_from_u32_truncates() {
        let flags = SampleFlag::from(0xFFFF_FFFF);
        // Should only retain known bits
        assert!(flags.contains(SampleFlag::BEGIN));
        assert!(flags.contains(SampleFlag::END));
    }

    #[test]
    fn sample_flag_empty_is_zero() {
        assert_eq!(SampleFlag::empty().bits(), 0);
    }

    #[test]
    fn event_kind_from_known_values() {
        assert_eq!(
            EventKind::from(ffi::SAMPLE_EVENT_DECOSTOP),
            EventKind::DecoStop
        );
        assert_eq!(EventKind::from(ffi::SAMPLE_EVENT_ASCENT), EventKind::Ascent);
        assert_eq!(
            EventKind::from(ffi::SAMPLE_EVENT_BOOKMARK),
            EventKind::Bookmark
        );
        assert_eq!(EventKind::from(ffi::SAMPLE_EVENT_STRING), EventKind::String);
    }

    #[test]
    fn event_kind_from_unknown_returns_none() {
        assert_eq!(EventKind::from(9999), EventKind::None);
    }

    #[test]
    fn sample_kind_display() {
        assert_eq!(SampleKind::Time.to_string(), "Time");
        assert_eq!(SampleKind::Depth.to_string(), "Depth");
        assert_eq!(SampleKind::Ppo2.to_string(), "PPO2");
        assert_eq!(SampleKind::O2sensor.to_string(), "O2 Sensor");
        assert_eq!(SampleKind::TTS.to_string(), "TTS");
    }

    #[test]
    fn event_kind_display() {
        assert_eq!(EventKind::DecoStop.to_string(), "Deco Stop");
        assert_eq!(EventKind::None.to_string(), "");
        assert_eq!(EventKind::SafetyStop.to_string(), "Safety Stop");
    }
}
