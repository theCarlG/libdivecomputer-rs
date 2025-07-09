use std::{collections::HashMap, time::Duration};

use libdivecomputer_sys as ffi;
use serde::{Deserialize, Serialize};

use crate::common::EventKind;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct Dive {
    pub fingerprint: Vec<u8>,
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum SalinityKind {
    #[default]
    Fresh,
    Salt,
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

#[derive(Debug)]
pub struct Vendor {}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Tank {
    pub gasmix_idx: Option<usize>, /* Gas mix index, or DC_GASMIX_UNKNOWN */
    pub kind: TankKind,
    pub volume: f64,         /* Volume (liter) */
    pub work_pressure: f64,  /* Work pressure (bar) */
    pub begin_pressure: f64, /* Begin pressure (bar) */
    pub end_pressure: f64,   /* End pressure (bar) */
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

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
pub enum Sensor {
    #[default]
    None,
    Id(u32),
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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
