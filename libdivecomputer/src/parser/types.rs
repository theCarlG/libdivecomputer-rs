use std::{
    collections::HashMap,
    fmt::{self, Display},
    str::FromStr,
    time::Duration,
};

use libdivecomputer_sys as ffi;
use serde::{Deserialize, Serialize};

use crate::{common::EventKind, error::LibError};

/// A parsed dive. Produced by [`Parser::parse`](crate::parser::Parser::parse)
/// from the raw bytes the C library hands back for a single dive record.
///
/// Most fields are `Option` or empty collections when the dive computer did
/// not record that datum; defaults come from [`Default::default`].
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Dive {
    /// Opaque per-dive identifier; stable across downloads for the same dive.
    pub fingerprint: Fingerprint,
    /// Dive start time (UTC).
    pub start: jiff::Timestamp,
    /// Total dive duration.
    pub duration: Duration,
    /// Maximum depth reached, in metres.
    pub max_depth: f64,
    /// Average depth over the dive, in metres, if recorded.
    pub avg_depth: Option<f64>,
    /// Gas mixes configured for the dive, indexed by `Tank::gasmix_idx` and
    /// `DiveSample::gasmix`.
    pub gasmixes: Vec<Gasmix>,
    /// Surface atmospheric pressure at dive start, in bar.
    pub atmospheric_pressure: Option<f64>,
    /// Surface water temperature at dive start, in °C.
    pub temperature_surface: Option<f64>,
    /// Minimum water temperature during the dive, in °C.
    pub temperature_minimum: Option<f64>,
    /// Maximum water temperature during the dive, in °C.
    pub temperature_maximum: Option<f64>,
    /// Cylinders used during the dive.
    pub tanks: Vec<Tank>,
    /// Dive mode (OC, CCR, …).
    pub dive_mode: DiveMode,
    /// Deco model in effect during the dive.
    pub deco_model: DecoModel,
    /// Water salinity and density, if reported by the device.
    pub salinity: Option<Salinity>,
    /// GPS location of the dive, if tagged by the device.
    pub location: Option<Location>,
    /// Per-sample time series for the dive (depth, temperature, events, …).
    pub samples: Vec<DiveSample>,
    /// Free-form device metadata extracted from string fields on the dive
    /// record (e.g. `STRING_KEY_SERIAL_NUMBER`,
    /// `STRING_KEY_FIRMWARE_VERSION`).
    pub metadata: HashMap<String, String>,
}

/// Opaque per-dive identifier as used by libdivecomputer's incremental
/// download. Two dives with the same fingerprint are the same dive.
#[derive(Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint {
    pub(crate) data: Vec<u8>,
}

impl Fingerprint {
    /// Returns `true` if this fingerprint carries no bytes (the default /
    /// "no dive yet" state).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Raw fingerprint bytes, as the device reported them.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Parse a hex string into a fingerprint.
    ///
    /// Returns an error if the string has odd length or contains non-ASCII-hex
    /// characters. Non-ASCII input is rejected rather than panicking on byte
    /// indexing.
    pub fn from_hex(hex: &str) -> Result<Self, LibError> {
        let bytes = hex.as_bytes();
        if !bytes.len().is_multiple_of(2) {
            return Err(LibError::InvalidArguments(
                "hex string must have even length".into(),
            ));
        }
        let data = bytes
            .chunks_exact(2)
            .map(|pair| {
                let s = std::str::from_utf8(pair).map_err(|_| {
                    LibError::InvalidArguments("hex string contains non-ASCII bytes".into())
                })?;
                u8::from_str_radix(s, 16).map_err(LibError::from)
            })
            .collect::<Result<Vec<u8>, _>>()?;
        Ok(Self { data })
    }

    /// Convert the fingerprint to a hex string.
    #[must_use]
    pub fn to_hex(&self) -> String {
        self.to_string()
    }
}

impl TryFrom<String> for Fingerprint {
    type Error = LibError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_hex(&value)
    }
}

impl TryFrom<&String> for Fingerprint {
    type Error = LibError;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        Self::from_hex(value)
    }
}

impl TryFrom<&str> for Fingerprint {
    type Error = LibError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_hex(value)
    }
}

impl From<&[u8]> for Fingerprint {
    fn from(value: &[u8]) -> Self {
        Self {
            data: value.to_vec(),
        }
    }
}

impl From<Vec<u8>> for Fingerprint {
    fn from(data: Vec<u8>) -> Self {
        Self { data }
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in &self.data {
            write!(f, "{b:02X}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint(0x{self})")
    }
}

/// Water salinity + density at dive start.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Salinity {
    /// Salinity kind (fresh or salt).
    pub kind: SalinityKind,
    /// Measured water density in kg/m³ (typically ~1000 fresh, ~1025 salt).
    pub density: f64,
}

impl From<ffi::dc_salinity_t> for Salinity {
    fn from(value: ffi::dc_salinity_t) -> Self {
        Self {
            kind: if value.type_ == ffi::DC_WATER_SALT {
                SalinityKind::Salt
            } else {
                SalinityKind::Fresh
            },
            density: value.density,
        }
    }
}

impl Display for Salinity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.kind, self.density)
    }
}

/// Water type for [`Salinity`].
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SalinityKind {
    /// Fresh water.
    #[default]
    Fresh,
    /// Salt water.
    Salt,
}

impl Display for SalinityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fresh => write!(f, "fresh"),
            Self::Salt => write!(f, "salt"),
        }
    }
}

/// GPS location of the dive site, as tagged by the device.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Location {
    /// Latitude in degrees (WGS-84).
    pub latitude: f64,
    /// Longitude in degrees (WGS-84).
    pub longitude: f64,
    /// Altitude above sea level in metres (for altitude dives).
    pub altitude: f64,
}

impl From<ffi::dc_location_t> for Location {
    fn from(value: ffi::dc_location_t) -> Self {
        Self {
            latitude: value.latitude,
            longitude: value.longitude,
            altitude: value.altitude,
        }
    }
}

/// Dive mode — the high-level style of diving reported by the computer.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum DiveMode {
    /// Mode not recorded or unknown.
    #[default]
    None,
    /// Breath-hold / freediving.
    Freedive,
    /// Gauge mode (depth + time only, no deco calculations).
    Gauge,
    /// Open-circuit scuba.
    OC,
    /// Closed-circuit rebreather.
    CCR,
    /// Semi-closed-circuit rebreather.
    SCR,
}

impl From<ffi::dc_divemode_t> for DiveMode {
    fn from(value: ffi::dc_divemode_t) -> Self {
        match value {
            ffi::DC_DIVEMODE_FREEDIVE => Self::Freedive,
            ffi::DC_DIVEMODE_GAUGE => Self::Gauge,
            ffi::DC_DIVEMODE_OC => Self::OC,
            ffi::DC_DIVEMODE_CCR => Self::CCR,
            ffi::DC_DIVEMODE_SCR => Self::SCR,
            _ => Self::None,
        }
    }
}

impl FromStr for DiveMode {
    type Err = LibError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "freedive" => Ok(Self::Freedive),
            "gauge" => Ok(Self::Gauge),
            "oc" => Ok(Self::OC),
            "ccr" => Ok(Self::CCR),
            "scr" => Ok(Self::SCR),
            _ => Err(LibError::InvalidArguments(format!(
                "unknown dive mode: {s}"
            ))),
        }
    }
}

impl From<String> for DiveMode {
    fn from(value: String) -> Self {
        Self::from_str(&value).unwrap_or(Self::None)
    }
}

impl fmt::Display for DiveMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::None => "None",
            Self::Freedive => "Freedive",
            Self::Gauge => "Gauge",
            Self::OC => "OC",
            Self::CCR => "CCR",
            Self::SCR => "SCR",
        };
        write!(f, "{s}")
    }
}

/// Decompression model used by the dive computer, plus its parameters.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DecoModel {
    /// Model not recorded or unknown.
    #[default]
    None,
    /// Bühlmann ZH-L with gradient factors.
    Buhlmann {
        /// Conservatism level (vendor-specific scale).
        conservatism: i32,
        /// Gradient factor low (percent, e.g. `30` for GF 30/70).
        low: u32,
        /// Gradient factor high (percent).
        high: u32,
    },
    /// Varying Permeability Model.
    Vpm {
        /// Conservatism level (vendor-specific scale).
        conservatism: i32,
    },
    /// Reduced Gradient Bubble Model.
    Rgbm {
        /// Conservatism level (vendor-specific scale).
        conservatism: i32,
    },
    /// Defence and Civil Institute of Environmental Medicine model.
    Dciem {
        /// Conservatism level (vendor-specific scale).
        conservatism: i32,
    },
}

impl From<ffi::dc_decomodel_t> for DecoModel {
    fn from(value: ffi::dc_decomodel_t) -> Self {
        unsafe {
            match value.type_ {
                ffi::DC_DECOMODEL_BUHLMANN => Self::Buhlmann {
                    conservatism: value.conservatism,
                    low: value.params.gf.low,
                    high: value.params.gf.high,
                },
                ffi::DC_DECOMODEL_VPM => Self::Vpm {
                    conservatism: value.conservatism,
                },
                ffi::DC_DECOMODEL_RGBM => Self::Rgbm {
                    conservatism: value.conservatism,
                },
                ffi::DC_DECOMODEL_DCIEM => Self::Dciem {
                    conservatism: value.conservatism,
                },
                _ => Self::None,
            }
        }
    }
}

/// A single cylinder used during a dive.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Tank {
    /// Index into `Dive::gasmixes` for the gas in this tank; `None` if the
    /// device didn't associate a gas mix with the tank.
    pub gasmix_idx: Option<usize>,
    /// Volume encoding (metric vs. imperial).
    pub kind: TankKind,
    /// Cylinder volume. Units depend on [`TankKind`] — litres for metric,
    /// cubic feet for imperial.
    pub volume: f64,
    /// Working pressure in bar (0 if not reported).
    pub work_pressure: f64,
    /// Pressure at the start of the dive, in bar.
    pub begin_pressure: f64,
    /// Pressure at the end of the dive, in bar.
    pub end_pressure: f64,
    /// How the tank is used in the configuration (e.g. sidemount).
    pub usage: TankUsage,
}

impl From<ffi::dc_tank_t> for Tank {
    fn from(value: ffi::dc_tank_t) -> Self {
        let gasmix_idx = if value.gasmix == ffi::DC_GASMIX_UNKNOWN {
            None
        } else {
            Some(value.gasmix as usize)
        };

        Self {
            gasmix_idx,
            kind: TankKind::from(value.type_),
            volume: value.volume,
            work_pressure: value.workpressure,
            begin_pressure: value.beginpressure,
            end_pressure: value.endpressure,
            usage: TankUsage::from(value.usage),
        }
    }
}

/// Volume encoding for a cylinder. Affects the interpretation of
/// [`Tank::volume`].
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TankKind {
    /// Kind not recorded.
    #[default]
    None,
    /// Volume is in litres (water capacity).
    Metric,
    /// Volume is in cubic feet at working pressure.
    Imperial,
}

impl From<u32> for TankKind {
    fn from(value: u32) -> Self {
        match value {
            ffi::DC_TANKVOLUME_METRIC => Self::Metric,
            ffi::DC_TANKVOLUME_IMPERIAL => Self::Imperial,
            _ => Self::None,
        }
    }
}

/// How the cylinder is mounted/used during the dive.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TankUsage {
    /// Usage not recorded.
    #[default]
    None,
    /// Tank is rigged sidemount.
    Sidemount,
}

impl From<u32> for TankUsage {
    fn from(value: u32) -> Self {
        if value == ffi::DC_TANK_USAGE_SIDEMOUNT {
            Self::Sidemount
        } else {
            Self::None
        }
    }
}

impl From<ffi::dc_usage_t> for GasUsage {
    fn from(value: ffi::dc_usage_t) -> Self {
        match value {
            ffi::DC_USAGE_OXYGEN => Self::Oxygen,
            ffi::DC_USAGE_DILUENT => Self::Diluent,
            ffi::DC_USAGE_OPEN_CIRCUIT => Self::OpenCircuit,
            _ => Self::None,
        }
    }
}

/// Gas mix composition. Fractions are mole fractions in the range `[0.0, 1.0]`
/// and should sum to 1.0 for a valid mix.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Gasmix {
    /// Helium fraction.
    pub helium: f64,
    /// Oxygen fraction.
    pub oxygen: f64,
    /// Nitrogen fraction.
    pub nitrogen: f64,
    /// Role this gas plays (oxygen, diluent, OC bottom gas, …).
    pub usage: GasUsage,
}

impl From<ffi::dc_gasmix_t> for Gasmix {
    fn from(value: ffi::dc_gasmix_t) -> Self {
        Self {
            helium: value.helium,
            oxygen: value.oxygen,
            nitrogen: value.nitrogen,
            usage: GasUsage::from(value.usage),
        }
    }
}

impl Default for Gasmix {
    fn default() -> Self {
        Self {
            helium: 0.0,
            oxygen: 0.21,
            nitrogen: 0.79,
            usage: GasUsage::default(),
        }
    }
}

/// Role a [`Gasmix`] plays in the dive plan.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GasUsage {
    /// Usage not specified.
    #[default]
    None,
    /// Pure-O2 bail-out / deco gas on a rebreather.
    Oxygen,
    /// Diluent supply for a closed-circuit rebreather.
    Diluent,
    /// Open-circuit breathing gas.
    OpenCircuit,
}

impl fmt::Display for GasUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::Oxygen => write!(f, "Oxygen"),
            Self::Diluent => write!(f, "Diluent"),
            Self::OpenCircuit => write!(f, "Open Circuit"),
        }
    }
}

impl FromStr for GasUsage {
    type Err = LibError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "oxygen" => Ok(Self::Oxygen),
            "diluent" => Ok(Self::Diluent),
            "open circuit" | "opencircuit" => Ok(Self::OpenCircuit),
            _ => Err(LibError::InvalidArguments(format!(
                "unknown gas usage: {s}"
            ))),
        }
    }
}

impl From<String> for GasUsage {
    fn from(value: String) -> Self {
        Self::from_str(&value).unwrap_or(Self::None)
    }
}

/// An event raised during a dive (deco violation, gas switch, ascent warning,
/// …). The meaning of `flags` and `value` depends on [`kind`](Self::kind); see
/// [`EventKind`] for the mapping.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DiveEvent {
    /// Offset from dive start.
    pub time: Duration,
    /// Event classification — dictates the meaning of `flags` / `value`.
    pub kind: EventKind,
    /// Event-specific flags (bitfield interpretation depends on `kind`).
    pub flags: u32,
    /// Event-specific value (interpretation depends on `kind`).
    pub value: u32,
    /// Human-readable label, if the C library provided one (e.g. a string
    /// event payload).
    pub name: Option<String>,
}

/// A single sample in the dive's time series.
///
/// Most fields are `Option` / `Vec` because dive computers differ widely in
/// what they record per sample. [`DiveSample::carry_forward`] propagates the
/// fields that are sampled sparsely (deco, CNS, …) from the previous sample.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DiveSample {
    /// Offset from dive start.
    pub time: Duration,
    /// Depth in metres.
    pub depth: f64,
    /// Active gas mix at this sample, if switched.
    pub gasmix: Option<Gasmix>,
    /// Water temperature in °C.
    pub temperature: Option<f64>,
    /// Events raised at this sample.
    pub events: Vec<DiveEvent>,
    /// Remaining bottom time computed by the computer.
    pub rbt: Option<Duration>,
    /// Heart rate in bpm, if the device records one.
    pub heartbeat: Option<u16>,
    /// Compass bearing in degrees, if the device records one.
    pub bearing: Option<i16>,
    /// Current CCR setpoint in bar, if applicable.
    pub setpoint: Option<f64>,
    /// Per-sensor partial-pressure-of-oxygen readings (for CCR).
    pub ppo2: Vec<Ppo2>,
    /// Raw O2 cell readings (ppO2 plus millivolt reading).
    pub o2_sensor: Vec<O2Sensor>,
    /// Tank pressures in bar, indexed in the same order as [`Dive::tanks`].
    pub pressure: Vec<f64>,
    /// Central nervous system toxicity fraction (0.0–1.0+).
    pub cns: f64,
    /// Current deco state (NDL remaining, deco stop, safety stop).
    pub deco: Option<Deco>,
    /// Time-to-surface estimate from the deco model.
    pub tts: Option<Duration>,
}

impl DiveSample {
    /// Create a new sample carrying forward persistent fields from the previous sample.
    #[must_use]
    pub fn carry_forward(prev: &DiveSample) -> Self {
        Self {
            setpoint: prev.setpoint,
            deco: prev.deco,
            tts: prev.tts,
            cns: prev.cns,
            heartbeat: prev.heartbeat,
            bearing: prev.bearing,
            ..Default::default()
        }
    }
}

/// Partial pressure of O2 reading from a single CCR O2 sensor.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Ppo2 {
    /// Sensor identifier (for multi-cell rebreathers).
    pub sensor: Sensor,
    /// Partial pressure of O2, in bar.
    pub bar: f64,
}

/// Raw O2 cell reading — the ppO2 the cell reports plus the underlying
/// millivolt reading, useful for diagnosing failing cells.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct O2Sensor {
    /// Sensor identifier.
    pub sensor: Sensor,
    /// Partial pressure of O2 reported by the cell, in bar.
    pub ppo2: f64,
    /// Raw cell voltage in millivolts.
    pub millivolt: f64,
}

/// Deco state at a sample — either "no-decompression limit" with remaining
/// NDL, or a required stop.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Deco {
    /// Deco-state classification.
    pub kind: DecoKind,
    /// Remaining NDL (for `NDL`) or required stop duration.
    pub time: Duration,
    /// Total time-to-surface estimate.
    pub tts: Duration,
}

impl fmt::Display for Deco {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            DecoKind::NDL => write!(f, "NDL: {} min", self.time.as_secs() / 60),
            DecoKind::DecoStop { depth } => {
                write!(f, "Deco stop: {} min @ {depth}m", self.time.as_secs() / 60)
            }
            DecoKind::DeepStop { depth } => {
                write!(f, "Deep stop: {} min @ {depth}m", self.time.as_secs() / 60)
            }
            DecoKind::SafetyStop { depth } => write!(
                f,
                "Safety stop: {} min @ {depth}m",
                self.time.as_secs() / 60
            ),
            DecoKind::None => write!(f, "None"),
        }
    }
}

impl fmt::Display for DecoKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::NDL => write!(f, "NDL"),
            Self::DecoStop { depth } => write!(f, "Deco stop @ {depth}m"),
            Self::DeepStop { depth } => write!(f, "Deep stop @ {depth}m"),
            Self::SafetyStop { depth } => write!(f, "Safety stop @ {depth}m"),
        }
    }
}

/// Sensor identifier for readings that come from a specific physical sensor
/// (e.g. a particular O2 cell on a rebreather).
#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[non_exhaustive]
pub enum Sensor {
    /// No sensor identifier attached.
    #[default]
    None,
    /// Sensor numbered by the device (1-based on most CCRs).
    Id(u32),
}

impl Sensor {
    /// Numeric sensor id, or `0` when `Sensor::None`.
    #[must_use]
    pub fn id(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::Id(id) => *id,
        }
    }
}

impl fmt::Display for Sensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::Id(id) => write!(f, "Sensor {id}"),
        }
    }
}

impl From<u32> for Sensor {
    fn from(value: u32) -> Self {
        if value == ffi::DC_SENSOR_NONE {
            Self::None
        } else {
            Sensor::Id(value)
        }
    }
}

/// Classification of the current deco state in a [`Deco`] record.
#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum DecoKind {
    /// No deco information.
    #[default]
    None,
    /// Within the no-decompression limit — [`Deco::time`] is NDL remaining.
    NDL,
    /// Required decompression stop at the given depth.
    DecoStop {
        /// Stop depth in metres.
        depth: f64,
    },
    /// Optional deep stop at the given depth.
    DeepStop {
        /// Stop depth in metres.
        depth: f64,
    },
    /// Recommended safety stop at the given depth.
    SafetyStop {
        /// Stop depth in metres.
        depth: f64,
    },
}

impl DecoKind {
    pub(crate) fn new(type_: ffi::dc_deco_type_t, depth: f64) -> Self {
        match type_ {
            ffi::DC_DECO_NDL => Self::NDL,
            ffi::DC_DECO_DECOSTOP => Self::DecoStop { depth },
            ffi::DC_DECO_DEEPSTOP => Self::DeepStop { depth },
            ffi::DC_DECO_SAFETYSTOP => Self::SafetyStop { depth },
            _ => Self::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_from_hex_valid() {
        let fp = Fingerprint::from_hex("DEADBEEF").unwrap();
        assert_eq!(fp.as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn fingerprint_from_hex_lowercase() {
        let fp = Fingerprint::from_hex("deadbeef").unwrap();
        assert_eq!(fp.as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn fingerprint_from_hex_odd_length_error() {
        assert!(Fingerprint::from_hex("ABC").is_err());
    }

    #[test]
    fn fingerprint_from_hex_invalid_chars_error() {
        assert!(Fingerprint::from_hex("GHIJ").is_err());
    }

    #[test]
    fn fingerprint_from_hex_empty() {
        let fp = Fingerprint::from_hex("").unwrap();
        assert!(fp.is_empty());
        assert_eq!(fp.as_bytes(), &[]);
    }

    #[test]
    fn fingerprint_to_hex_round_trip() {
        let original = "0A1B2C3D4E5F";
        let fp = Fingerprint::from_hex(original).unwrap();
        assert_eq!(fp.to_hex(), original);
    }

    #[test]
    fn fingerprint_from_slice() {
        let bytes: &[u8] = &[1, 2, 3];
        let fp = Fingerprint::from(bytes);
        assert_eq!(fp.as_bytes(), &[1, 2, 3]);
    }

    #[test]
    fn fingerprint_from_vec() {
        let fp = Fingerprint::from(vec![0xAA, 0xBB]);
        assert_eq!(fp.as_bytes(), &[0xAA, 0xBB]);
    }

    #[test]
    fn fingerprint_try_from_str() {
        let fp = Fingerprint::try_from("FF00").unwrap();
        assert_eq!(fp.as_bytes(), &[0xFF, 0x00]);
    }

    #[test]
    fn fingerprint_try_from_string() {
        let s = String::from("AABB");
        let fp = Fingerprint::try_from(s).unwrap();
        assert_eq!(fp.as_bytes(), &[0xAA, 0xBB]);
    }

    #[test]
    fn fingerprint_try_from_ref_string() {
        let s = String::from("CCDD");
        let fp = Fingerprint::try_from(&s).unwrap();
        assert_eq!(fp.as_bytes(), &[0xCC, 0xDD]);
    }

    #[test]
    fn fingerprint_display() {
        let fp = Fingerprint::from(vec![0xDE, 0xAD]);
        assert_eq!(format!("{fp}"), "DEAD");
    }

    #[test]
    fn fingerprint_debug() {
        let fp = Fingerprint::from(vec![0xBE, 0xEF]);
        assert_eq!(format!("{fp:?}"), "Fingerprint(0xBEEF)");
    }

    #[test]
    fn fingerprint_is_empty() {
        assert!(Fingerprint::default().is_empty());
        assert!(!Fingerprint::from(vec![1]).is_empty());
    }

    #[test]
    fn dive_mode_from_string_known() {
        assert_eq!(DiveMode::from("freedive".to_string()), DiveMode::Freedive);
        assert_eq!(DiveMode::from("gauge".to_string()), DiveMode::Gauge);
        assert_eq!(DiveMode::from("oc".to_string()), DiveMode::OC);
        assert_eq!(DiveMode::from("ccr".to_string()), DiveMode::CCR);
        assert_eq!(DiveMode::from("scr".to_string()), DiveMode::SCR);
    }

    #[test]
    fn dive_mode_from_string_unknown() {
        assert_eq!(DiveMode::from("unknown".to_string()), DiveMode::None);
    }

    #[test]
    fn gas_usage_from_string_known() {
        assert_eq!(GasUsage::from("oxygen".to_string()), GasUsage::Oxygen);
        assert_eq!(GasUsage::from("diluent".to_string()), GasUsage::Diluent);
        assert_eq!(
            GasUsage::from("open circuit".to_string()),
            GasUsage::OpenCircuit
        );
        assert_eq!(
            GasUsage::from("opencircuit".to_string()),
            GasUsage::OpenCircuit
        );
    }

    #[test]
    fn gas_usage_from_string_unknown() {
        assert_eq!(GasUsage::from("nope".to_string()), GasUsage::None);
    }

    #[test]
    fn gasmix_default_is_air() {
        let air = Gasmix::default();
        assert!((air.oxygen - 0.21).abs() < f64::EPSILON);
        assert!((air.nitrogen - 0.79).abs() < f64::EPSILON);
        assert!((air.helium - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn deco_kind_display() {
        use std::time::Duration;

        let deco = Deco {
            kind: DecoKind::NDL,
            time: Duration::from_secs(300),
            tts: Duration::ZERO,
        };
        assert_eq!(format!("{deco}"), "NDL: 5 min");

        let deco = Deco {
            kind: DecoKind::DecoStop { depth: 6.0 },
            time: Duration::from_secs(180),
            tts: Duration::ZERO,
        };
        assert_eq!(format!("{deco}"), "Deco stop: 3 min @ 6m");

        let deco = Deco {
            kind: DecoKind::SafetyStop { depth: 5.0 },
            time: Duration::from_secs(180),
            tts: Duration::ZERO,
        };
        assert_eq!(format!("{deco}"), "Safety stop: 3 min @ 5m");
    }
}
