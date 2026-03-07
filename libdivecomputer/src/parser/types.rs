use std::{
    collections::HashMap,
    fmt::{self, Display},
    str::FromStr,
    time::Duration,
};

use libdivecomputer_sys as ffi;
use serde::{Deserialize, Serialize};

use crate::{common::EventKind, error::LibError};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Dive {
    pub fingerprint: Fingerprint,
    pub start: jiff::Timestamp,
    pub duration: Duration,
    pub max_depth: f64,
    pub avg_depth: Option<f64>,
    pub gasmixes: Vec<Gasmix>,
    pub atmospheric_pressure: Option<f64>,
    pub temperature_surface: Option<f64>,
    pub temperature_minimum: Option<f64>,
    pub temperature_maximum: Option<f64>,
    pub tanks: Vec<Tank>,
    pub dive_mode: DiveMode,
    pub deco_model: DecoModel,
    pub salinity: Option<Salinity>,
    pub location: Option<Location>,
    pub samples: Vec<DiveSample>,
    pub metadata: HashMap<String, String>,
}

#[derive(Default, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Fingerprint {
    pub(crate) data: Vec<u8>,
}

impl Fingerprint {
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Parse a hex string into a fingerprint.
    ///
    /// Returns an error if the string has odd length or contains non-hex characters.
    pub fn from_hex(hex: &str) -> Result<Self, LibError> {
        if !hex.len().is_multiple_of(2) {
            return Err(LibError::InvalidArguments(
                "hex string must have even length".into(),
            ));
        }
        let data = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect::<Result<Vec<u8>, _>>()?;
        Ok(Self { data })
    }

    /// Convert the fingerprint to a hex string.
    pub fn to_hex(&self) -> String {
        self.data.iter().map(|b| format!("{b:02X}")).collect()
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
        write!(f, "{}", self.to_hex())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint(0x{})", self.to_hex())
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Salinity {
    pub kind: SalinityKind,
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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum SalinityKind {
    #[default]
    Fresh,
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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum DiveMode {
    #[default]
    None,
    Freedive,
    Gauge,
    OC,
    CCR,
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DecoModel {
    #[default]
    None,
    Buhlmann {
        conservatism: i32,
        low: u32,
        high: u32,
    },
    Vpm {
        conservatism: i32,
    },
    Rgbm {
        conservatism: i32,
    },
    Dciem {
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Tank {
    pub gasmix_idx: Option<usize>,
    pub kind: TankKind,
    pub volume: f64,
    pub work_pressure: f64,
    pub begin_pressure: f64,
    pub end_pressure: f64,
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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TankKind {
    #[default]
    None,
    Metric,
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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TankUsage {
    #[default]
    None,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Gasmix {
    pub helium: f64,
    pub oxygen: f64,
    pub nitrogen: f64,
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GasUsage {
    #[default]
    None,
    Oxygen,
    Diluent,
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DiveEvent {
    pub time: Duration,
    pub kind: EventKind,
    pub flags: u32,
    pub value: u32,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DiveSample {
    pub time: Duration,
    pub depth: f64,
    pub gasmix: Option<Gasmix>,
    pub temperature: Option<f64>,
    pub events: Vec<DiveEvent>,
    pub rbt: Option<Duration>,
    pub heartbeat: Option<u16>,
    pub bearing: Option<i16>,
    pub setpoint: Option<f64>,
    pub ppo2: Vec<Ppo2>,
    pub o2_sensor: Vec<O2Sensor>,
    pub pressure: Vec<f64>,
    pub cns: f64,
    pub deco: Option<Deco>,
}

impl DiveSample {
    /// Create a new sample carrying forward persistent fields from the previous sample.
    pub fn carry_forward(prev: &DiveSample) -> Self {
        Self {
            setpoint: prev.setpoint,
            deco: prev.deco,
            cns: prev.cns,
            heartbeat: prev.heartbeat,
            bearing: prev.bearing,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Ppo2 {
    pub sensor: Sensor,
    pub bar: f64,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct O2Sensor {
    pub sensor: Sensor,
    pub ppo2: f64,
    pub millivolt: f64,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Deco {
    pub kind: DecoKind,
    pub time: Duration,
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

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize)]
#[non_exhaustive]
pub enum Sensor {
    #[default]
    None,
    Id(u32),
}

impl Sensor {
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

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[non_exhaustive]
pub enum DecoKind {
    #[default]
    None,
    NDL,
    DecoStop {
        depth: f64,
    },
    DeepStop {
        depth: f64,
    },
    SafetyStop {
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
