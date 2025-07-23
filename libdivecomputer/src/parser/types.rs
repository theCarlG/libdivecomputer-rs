use std::{
    collections::HashMap,
    fmt::{self, Display},
    time::Duration,
};

use libdivecomputer_sys as ffi;
use serde::{Deserialize, Serialize};

use crate::{
    common::EventKind,
    device::{bytes_to_hex, hex_string_to_bytes},
};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct Dive {
    pub fingerprint: Fingerprint,
    pub start: jiff::Timestamp,
    pub duration: Duration,
    pub max_depth: f64,
    pub avg_depth: Option<f64>,
    pub gasmixes: Vec<Gasmix>,
    pub atmospheric_pressure: Option<f64>,
    pub temperature_surface: f32,
    pub temperature_minimum: f32,
    pub temperature_maximum: f32,
    pub tanks: Vec<Tank>,
    pub dive_mode: DiveMode,
    pub deco_model: DecoModel,
    pub salinity: Option<Salinity>,
    pub location: Option<Location>,
    pub samples: Vec<DiveSample>,
    pub metadata: HashMap<String, String>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Fingerprint {
    pub(crate) data: Vec<u8>,
}

impl Fingerprint {
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl TryFrom<String> for Fingerprint {
    type Error = std::num::ParseIntError;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            data: hex_string_to_bytes(&value)?,
        })
    }
}

impl TryFrom<&String> for Fingerprint {
    type Error = std::num::ParseIntError;

    fn try_from(value: &String) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            data: hex_string_to_bytes(value)?,
        })
    }
}

impl TryFrom<&str> for Fingerprint {
    type Error = std::num::ParseIntError;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            data: hex_string_to_bytes(value)?,
        })
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
        write!(f, "{}", bytes_to_hex(&self.data))
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint(0x{})", bytes_to_hex(&self.data))
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}

impl From<ffi::dc_location_t> for Location {
    fn from(value: ffi::dc_location_t) -> Self {
        let ffi::dc_location_t {
            latitude,
            longitude,
            altitude,
        } = value;

        Self {
            latitude,
            longitude,
            altitude,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq)]
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

impl From<String> for DiveMode {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "freedive" => Self::Freedive,
            "gauge" => Self::Gauge,
            "oc" => Self::OC,
            "ccr" => Self::CCR,
            "scr" => Self::SCR,
            _ => Self::None,
        }
    }
}

impl fmt::Display for DiveMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::None => "None",
                Self::Freedive => "Freedive",
                Self::Gauge => "Gauge",
                Self::OC => "OC",
                Self::CCR => "CCR",
                Self::SCR => "SCR",
            }
        )
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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
    pub gasmix_idx: Option<usize>, // Gas mix index
    pub kind: TankKind,
    pub volume: f64,         // Volume (liter)
    pub work_pressure: f64,  // Work pressure (bar)
    pub begin_pressure: f64, // Begin pressure (bar)
    pub end_pressure: f64,   // End pressure (bar)
    pub usage: TankUsage,
}

impl From<ffi::dc_tank_t> for Tank {
    fn from(value: ffi::dc_tank_t) -> Self {
        let ffi::dc_tank_t {
            gasmix,
            type_,
            volume,
            workpressure: work_pressure,
            beginpressure: begin_pressure,
            endpressure: end_pressure,
            usage,
        } = value;
        let gasmix_idx = if gasmix == ffi::DC_GASMIX_UNKNOWN {
            None
        } else {
            Some(gasmix as usize)
        };

        Self {
            gasmix_idx,
            kind: TankKind::from(type_),
            volume,
            work_pressure,
            begin_pressure,
            end_pressure,
            usage: TankUsage::from(usage),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gasmix {
    pub helium: f64,
    pub oxygen: f64,
    pub nitrogen: f64,
    pub usage: GasUsage,
}

impl From<ffi::dc_gasmix_t> for Gasmix {
    fn from(value: ffi::dc_gasmix_t) -> Self {
        let ffi::dc_gasmix_t {
            helium,
            oxygen,
            nitrogen,
            usage,
        } = value;

        Self {
            helium,
            oxygen,
            nitrogen,
            usage: GasUsage::from(usage),
        }
    }
}

impl Default for Gasmix {
    fn default() -> Self {
        Self {
            helium: 0.,
            oxygen: 0.21,
            nitrogen: 0.79,
            usage: GasUsage::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

impl From<String> for GasUsage {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "oxygen" => Self::Oxygen,
            "diluent" => Self::Diluent,
            "open circuit" | "opencircuit" => Self::OpenCircuit,
            _ => Self::None,
        }
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
    pub temperature: f64,
    pub event: Option<DiveEvent>,
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

impl From<&DiveSample> for DiveSample {
    fn from(value: &DiveSample) -> Self {
        let DiveSample {
            setpoint,
            deco,
            cns,
            heartbeat,
            bearing,
            ..
        } = value;

        Self {
            setpoint: *setpoint,
            deco: deco.clone(),
            cns: *cns,
            heartbeat: *heartbeat,
            bearing: *bearing,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Ppo2 {
    pub sensor: Sensor,
    pub bar: f64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct O2Sensor {
    pub sensor: Sensor,
    pub ppo2: f64,
    pub millivolt: f64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub enum DecoKind {
    #[default]
    None,
    NDL,
    DecoStop {
        depth: f64, // meters
    },
    DeepStop {
        depth: f64, // meters
    },
    SafetyStop {
        depth: f64, // meters
    },
}

impl DecoKind {
    pub fn new(type_: ffi::dc_deco_type_t, depth: f64) -> Self {
        match type_ {
            ffi::DC_DECO_NDL => Self::NDL,
            ffi::DC_DECO_DECOSTOP => Self::DecoStop { depth },
            ffi::DC_DECO_DEEPSTOP => Self::DeepStop { depth },
            ffi::DC_DECO_SAFETYSTOP => Self::SafetyStop { depth },
            _ => Self::None,
        }
    }
}
