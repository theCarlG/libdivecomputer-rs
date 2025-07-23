mod types;
use std::{
    ffi::{CStr, c_void},
    ptr,
    time::Duration,
};

pub use types::*;

use libdivecomputer_sys as ffi;

use crate::{
    Context, c_void_as,
    common::EventKind,
    descriptor::DescriptorItem,
    device::{Device, DeviceConnected},
    error::{LibError, Result},
    void_ptr,
};

pub struct Parser {
    pub(crate) ptr: *mut ffi::dc_parser_t,
    pub(crate) data: ParseData,
}

impl Parser {
    pub fn new(device: &Device<DeviceConnected>, data: Vec<u8>) -> Result<Self> {
        let mut ptr = ptr::null_mut();

        let data_ptr = data.as_ptr() as *mut u8;
        let data_size = data.len();

        let status = unsafe { ffi::dc_parser_new(&mut ptr, device.ptr, data_ptr, data_size) };
        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::status_with_context(
                status,
                "failed to create parser",
            ));
        }

        Ok(Self {
            ptr,
            data: ParseData {
                ..Default::default()
            },
        })
    }

    pub fn parse(&mut self, fingerprint: Vec<u8>) -> Result<Dive> {
        self.data.dive = Dive {
            fingerprint: Fingerprint::from(fingerprint),
            ..parse_fields(self.ptr)?
        };

        unsafe {
            let status = ffi::dc_parser_samples_foreach(
                self.ptr,
                Some(sample_callback),
                void_ptr!(&mut self.data),
            );
            if status != ffi::DC_STATUS_SUCCESS {
                return Err(LibError::status_with_context(
                    status,
                    "failed to parse samples",
                ));
            }
        }

        Ok(self.data.dive.clone())
    }

    /// Create a parser without a device connection for parsing saved dive data
    pub fn parse_standalone(
        context: &Context,
        descriptor: &DescriptorItem,
        data: Vec<u8>,
    ) -> Result<Dive> {
        let mut ptr = ptr::null_mut();

        let data_ptr = data.as_ptr() as *mut u8;
        let data_size = data.len();

        let status = unsafe {
            ffi::dc_parser_new2(&mut ptr, context.ptr(), descriptor.ptr, data_ptr, data_size)
        };

        if status != ffi::DC_STATUS_SUCCESS {
            return Err(LibError::status_with_context(
                status,
                "failed to create parser",
            ));
        }

        // Parse the dive data
        let dive = Dive {
            fingerprint: if data.len() > 16 {
                Fingerprint::from(&data[12..16])
            } else {
                Fingerprint::from(data)
            },
            ..parse_fields(ptr)?
        };

        // Parse samples
        let mut parse_data = ParseData {
            dive: dive.clone(),
            sample: DiveSample::default(),
            vendor: String::new(),
        };

        unsafe {
            let status = ffi::dc_parser_samples_foreach(
                ptr,
                Some(sample_callback),
                void_ptr!(&mut parse_data),
            );
            if status != ffi::DC_STATUS_SUCCESS {
                ffi::dc_parser_destroy(ptr);
                return Err(LibError::status_with_context(
                    status,
                    "failed to parse samples",
                ));
            }

            ffi::dc_parser_destroy(ptr);
        }

        Ok(parse_data.dive)
    }
}

impl Drop for Parser {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                ffi::dc_parser_destroy(self.ptr);
            }
        }
    }
}

#[derive(Debug, Default)]
#[repr(C)]
pub struct ParseData {
    dive: Dive,
    sample: DiveSample,
    vendor: String,
}

fn parse_fields(parser: *mut ffi::dc_parser_t) -> Result<Dive> {
    unsafe {
        let mut dive = Dive::default();

        let mut dt = std::mem::MaybeUninit::<ffi::dc_datetime_t>::zeroed().assume_init();
        let mut status = ffi::dc_parser_get_datetime(parser, &mut dt);
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the datetime.",
            ));
        }

        dive.start = if dt.timezone == i32::MIN {
            // Maybe use system timezone instead of UTC?
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}Z",
                dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
            )
            .parse()?
        } else {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}{:+03}:{:02}",
                dt.year,
                dt.month,
                dt.day,
                dt.hour,
                dt.minute,
                dt.second,
                dt.timezone / 3600,
                (dt.timezone.abs() % 3600) / 60
            )
            .parse()?
        };

        let mut divetime: u32 = 0;
        status =
            ffi::dc_parser_get_field(parser, ffi::DC_FIELD_DIVETIME, 0, void_ptr!(&mut divetime));
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the dive duration.",
            ));
        }
        dive.duration = Duration::from_secs(divetime as u64);

        status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_MAXDEPTH,
            0,
            void_ptr!(&mut dive.max_depth),
        );
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the max depth.",
            ));
        }

        let mut avg_depth: f64 = 0.;
        status =
            ffi::dc_parser_get_field(parser, ffi::DC_FIELD_AVGDEPTH, 0, void_ptr!(&mut avg_depth));
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the avg depth.",
            ));
        }

        if status != ffi::DC_STATUS_UNSUPPORTED {
            dive.avg_depth = Some(avg_depth);
        }

        status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_TEMPERATURE_MAXIMUM,
            0,
            void_ptr!(&mut dive.temperature_maximum),
        );
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the max temperature.",
            ));
        }
        status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_TEMPERATURE_MINIMUM,
            0,
            void_ptr!(&mut dive.temperature_minimum),
        );
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the min temperature.",
            ));
        }
        status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_TEMPERATURE_SURFACE,
            0,
            void_ptr!(&mut dive.temperature_surface),
        );
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the surface temperature.",
            ));
        }

        let mut num_gases: u32 = 0;
        status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_GASMIX_COUNT,
            0,
            void_ptr!(&mut num_gases),
        );
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the gas mix count.",
            ));
        }

        for i in 0..num_gases {
            let mut gasmix = std::mem::MaybeUninit::<ffi::dc_gasmix_t>::zeroed().assume_init();
            status =
                ffi::dc_parser_get_field(parser, ffi::DC_FIELD_GASMIX, i, void_ptr!(&mut gasmix));
            if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
                return Err(LibError::status_with_context(
                    status,
                    "failed to parse the gas mix.",
                ));
            }

            dive.gasmixes.push(Gasmix::from(gasmix));
        }

        let mut num_tanks: u32 = 0;
        status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_TANK_COUNT,
            0,
            void_ptr!(&mut num_tanks),
        );
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the gas tank count.",
            ));
        }

        for i in 0..num_tanks {
            let mut tank = std::mem::MaybeUninit::<ffi::dc_tank_t>::zeroed().assume_init();
            status = ffi::dc_parser_get_field(parser, ffi::DC_FIELD_TANK, i, void_ptr!(&mut tank));
            if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
                return Err(LibError::status_with_context(
                    status,
                    "failed to parse the gas tank.",
                ));
            }
            dive.tanks.push(Tank::from(tank));
        }

        // Parse the dive mode.
        let mut divemode = ffi::DC_DIVEMODE_OC;
        status =
            ffi::dc_parser_get_field(parser, ffi::DC_FIELD_DIVEMODE, 0, void_ptr!(&mut divemode));
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the dive mode.",
            ));
        }
        dive.dive_mode = DiveMode::from(divemode);

        let mut decomodel = std::mem::MaybeUninit::<ffi::dc_decomodel_t>::zeroed().assume_init();
        status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_DECOMODEL,
            0,
            void_ptr!(&mut decomodel),
        );
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the deco model.",
            ));
        }
        dive.deco_model = DecoModel::from(decomodel);

        let mut salinity = std::mem::MaybeUninit::<ffi::dc_salinity_t>::zeroed().assume_init();
        status =
            ffi::dc_parser_get_field(parser, ffi::DC_FIELD_SALINITY, 0, void_ptr!(&mut salinity));
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the water salinity.",
            ));
        }

        if status != ffi::DC_STATUS_UNSUPPORTED {
            dive.salinity = Some(Salinity::from(salinity));
        }

        let mut atmospheric: f64 = 0.;
        status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_ATMOSPHERIC,
            0,
            void_ptr!(&mut atmospheric),
        );
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the atmospheric pressure.",
            ));
        }

        if status != ffi::DC_STATUS_UNSUPPORTED {
            dive.atmospheric_pressure = Some(atmospheric);
        }

        for idx in 0..100 {
            let mut str = std::mem::MaybeUninit::<ffi::dc_field_string_t>::zeroed().assume_init();
            status =
                ffi::dc_parser_get_field(parser, ffi::DC_FIELD_STRING, idx, void_ptr!(&mut str));
            if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
                return Err(LibError::status_with_context(
                    status,
                    "failed to parse strings.",
                ));
            }
            if status == ffi::DC_STATUS_UNSUPPORTED || str.desc.is_null() || str.value.is_null() {
                break;
            }

            let key = CStr::from_ptr(str.desc).to_string_lossy().into_owned();
            let value = CStr::from_ptr(str.value).to_string_lossy().into_owned();

            dive.metadata.insert(key, value);
        }

        let mut location = std::mem::MaybeUninit::<ffi::dc_location_t>::zeroed().assume_init();
        status =
            ffi::dc_parser_get_field(parser, ffi::DC_FIELD_LOCATION, 0, void_ptr!(&mut location));
        if status != ffi::DC_STATUS_SUCCESS && status != ffi::DC_STATUS_UNSUPPORTED {
            return Err(LibError::status_with_context(
                status,
                "failed to parse the GPS location.",
            ));
        }

        if status != ffi::DC_STATUS_UNSUPPORTED {
            dive.location = Some(Location::from(location));
        }

        Ok(dive)
    }
}

#[unsafe(no_mangle)]
extern "C" fn sample_callback(
    kind: ffi::dc_sample_type_t,
    pvalue: *const ffi::dc_sample_value_t,
    userdata: *mut c_void,
) {
    unsafe {
        let parse_data = c_void_as!(userdata, ParseData);

        let value = *pvalue;
        match kind {
            ffi::DC_SAMPLE_TIME => {
                let sample = parse_data.sample.clone();
                parse_data.sample = DiveSample::from(&sample);
                parse_data.sample.time = Duration::from_millis(value.time as u64);

                if sample.time.as_millis() > 0 {
                    parse_data.dive.samples.push(sample);
                }
            }

            ffi::DC_SAMPLE_O2SENSOR => {
                let sensor = O2Sensor {
                    sensor: Sensor::from(value.o2sensor.sensor),
                    ppo2: value.o2sensor.ppo2,
                    millivolt: value.o2sensor.millivolt,
                };
                parse_data.sample.o2_sensor.push(sensor);
            }

            ffi::DC_SAMPLE_DEPTH => {
                parse_data.sample.depth = value.depth;
            }

            ffi::DC_SAMPLE_PRESSURE => {
                let idx = value.pressure.tank as usize;
                let value = value.pressure.value;
                if let Some(pressure) = parse_data.sample.pressure.get_mut(idx) {
                    *pressure = value;
                } else {
                    parse_data.sample.pressure.insert(idx, value);
                };
            }

            ffi::DC_SAMPLE_GASMIX => {
                let idx = value.gasmix as usize;
                parse_data.sample.gasmix = parse_data.dive.gasmixes.get(idx).cloned();
            }

            ffi::DC_SAMPLE_EVENT => {
                let kind = EventKind::from(value.event.type_);
                let time =
                    Duration::from_secs(value.event.time as u64 + parse_data.sample.time.as_secs());
                let flags = value.event.flags;
                let value = value.event.value;
                parse_data.sample.event = Some(DiveEvent {
                    kind,
                    time,
                    flags,
                    value,
                });
            }

            ffi::DC_SAMPLE_TEMPERATURE => {
                parse_data.sample.temperature = value.temperature;
            }

            ffi::DC_SAMPLE_RBT => {
                let model_lower = parse_data.vendor.to_lowercase();
                let seconds = if model_lower.starts_with("suunto") {
                    value.rbt
                } else {
                    value.rbt * 60
                };

                parse_data.sample.rbt = Some(Duration::from_secs(seconds as u64));
            }

            ffi::DC_SAMPLE_HEARTBEAT => {
                parse_data.sample.heartbeat = Some(value.heartbeat as u16);
            }

            ffi::DC_SAMPLE_BEARING => {
                parse_data.sample.bearing = Some(value.bearing as i16);
            }

            ffi::DC_SAMPLE_SETPOINT => {
                parse_data.sample.setpoint = Some(value.setpoint);
            }

            ffi::DC_SAMPLE_PPO2 => {
                parse_data.sample.ppo2.push(Ppo2 {
                    sensor: Sensor::from(value.ppo2.sensor),
                    bar: value.ppo2.value,
                });
            }

            ffi::DC_SAMPLE_CNS => parse_data.sample.cns = value.cns,

            ffi::DC_SAMPLE_DECO => {
                parse_data.sample.deco = Some(Deco {
                    kind: DecoKind::new(value.deco.type_, value.deco.depth),
                    time: Duration::from_secs(value.deco.time as u64),
                    tts: Duration::from_secs(value.deco.tts as u64),
                })
            }

            ffi::DC_SAMPLE_VENDOR => {
                // printf("   <vendor time='%u:%02u' type=\"%u\" size=\"%u\">", FRACTION_TUPLE(sample.time.seconds, 60),
                //        value.vendor.type, value.vendor.size);
                // for (int i = 0; i < value.vendor.size; ++i)
                // 	printf("%02X", ((unsigned char *)value.vendor.data)[i]);
                // printf("</vendor>\n");
            }
            _ => {}
        };
    }
}
