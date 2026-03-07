pub mod types;

use std::{
    ffi::{CStr, c_void},
    ptr,
    time::Duration,
};

pub use types::*;

use libdivecomputer_sys as ffi;

use crate::{
    common::{EventKind, as_void_ptr, from_void_ptr},
    context::Context,
    descriptor::Descriptor,
    device::Device,
    error::Result,
    status::Status,
};

/// Well-known string key for firmware version in `SAMPLE_EVENT_STRING` events.
pub const STRING_KEY_FIRMWARE_VERSION: &str = "FW Version";

/// Well-known string key for serial number in `SAMPLE_EVENT_STRING` events.
pub const STRING_KEY_SERIAL_NUMBER: &str = "Serial";

/// Dive data parser. Wraps `dc_parser_t`.
pub struct Parser {
    ptr: *mut ffi::dc_parser_t,
}

impl Parser {
    /// Create a parser from a connected device.
    pub fn from_device(device: &Device, data: &[u8]) -> Result<Self> {
        unsafe { Self::from_raw_device_ptr(device.raw_ptr(), data) }
    }

    /// Create a parser from a raw device pointer.
    ///
    /// # Safety
    /// The caller must ensure the pointer is a valid `dc_device_t`.
    pub(crate) unsafe fn from_raw_device_ptr(
        device_ptr: *mut ffi::dc_device_t,
        data: &[u8],
    ) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let status = unsafe { ffi::dc_parser_new(&mut ptr, device_ptr, data.as_ptr(), data.len()) };
        Status::check(status, "failed to create parser from device")?;
        Ok(Self { ptr })
    }

    /// Create a parser from a descriptor (for parsing saved dive data).
    pub fn from_descriptor(ctx: &Context, desc: &Descriptor, data: &[u8]) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let status = unsafe {
            ffi::dc_parser_new2(&mut ptr, ctx.ptr(), desc.ptr, data.as_ptr(), data.len())
        };
        Status::check(status, "failed to create parser from descriptor")?;
        Ok(Self { ptr })
    }

    /// Set the device clock reference for datetime calculation.
    pub fn set_clock(&self, devtime: u32, systime: i64) -> Result<()> {
        let status = unsafe { ffi::dc_parser_set_clock(self.ptr, devtime, systime) };
        Status::check(status, "failed to set parser clock")
    }

    /// Set the surface atmospheric pressure (in bar).
    pub fn set_atmospheric(&self, pressure: f64) -> Result<()> {
        let status = unsafe { ffi::dc_parser_set_atmospheric(self.ptr, pressure) };
        Status::check(status, "failed to set atmospheric pressure")
    }

    /// Set the water density (in kg/m3).
    pub fn set_density(&self, density: f64) -> Result<()> {
        let status = unsafe { ffi::dc_parser_set_density(self.ptr, density) };
        Status::check(status, "failed to set water density")
    }

    /// Get the parser family (device type).
    pub fn family(&self) -> crate::family::Family {
        let raw = unsafe { ffi::dc_parser_get_type(self.ptr) };
        crate::family::Family::from(raw)
    }

    /// Parse all fields and samples into a `Dive`.
    pub fn parse(&self, fingerprint: &Fingerprint) -> Result<Dive> {
        let mut dive = Dive {
            fingerprint: fingerprint.clone(),
            ..parse_fields(self.ptr)?
        };

        let mut parse_data = ParseData {
            dive: &mut dive,
            sample: DiveSample::default(),
        };

        unsafe {
            let status = ffi::dc_parser_samples_foreach(
                self.ptr,
                Some(sample_callback),
                as_void_ptr(&mut parse_data),
            );
            Status::check(status, "failed to parse samples")?;
        }

        // Push the last sample if it has data.
        let last_sample = std::mem::take(&mut parse_data.sample);
        if last_sample.time.as_millis() > 0 {
            parse_data.dive.samples.push(last_sample);
        }

        Ok(dive)
    }
}

impl std::fmt::Debug for Parser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Parser")
            .field("open", &!self.ptr.is_null())
            .finish()
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

/// Internal parse state for sample callback.
struct ParseData<'a> {
    dive: &'a mut Dive,
    sample: DiveSample,
}

fn parse_fields(parser: *mut ffi::dc_parser_t) -> Result<Dive> {
    unsafe {
        let mut dive = Dive::default();

        // Datetime
        let mut dt = std::mem::MaybeUninit::<ffi::dc_datetime_t>::zeroed().assume_init();
        let status = ffi::dc_parser_get_datetime(parser, &mut dt);
        if Status::check_unsupported(status, "failed to parse datetime")? {
            dive.start = crate::datetime::ffi_to_timestamp(&dt)?;
        }

        // Divetime
        let mut divetime: u32 = 0;
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_DIVETIME,
            0,
            &mut divetime as *mut _ as *mut c_void,
        );
        Status::check_unsupported(status, "failed to parse divetime")?;
        dive.duration = Duration::from_secs(divetime as u64);

        // Max depth
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_MAXDEPTH,
            0,
            &mut dive.max_depth as *mut _ as *mut c_void,
        );
        Status::check_unsupported(status, "failed to parse max depth")?;

        // Avg depth
        let mut avg_depth: f64 = 0.0;
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_AVGDEPTH,
            0,
            &mut avg_depth as *mut _ as *mut c_void,
        );
        if Status::check_unsupported(status, "failed to parse avg depth")? {
            dive.avg_depth = Some(avg_depth);
        }

        // Temperatures
        let mut temp_max: f64 = 0.0;
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_TEMPERATURE_MAXIMUM,
            0,
            &mut temp_max as *mut _ as *mut c_void,
        );
        if Status::check_unsupported(status, "failed to parse max temperature")? {
            dive.temperature_maximum = Some(temp_max);
        }

        let mut temp_min: f64 = 0.0;
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_TEMPERATURE_MINIMUM,
            0,
            &mut temp_min as *mut _ as *mut c_void,
        );
        if Status::check_unsupported(status, "failed to parse min temperature")? {
            dive.temperature_minimum = Some(temp_min);
        }

        let mut temp_surface: f64 = 0.0;
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_TEMPERATURE_SURFACE,
            0,
            &mut temp_surface as *mut _ as *mut c_void,
        );
        if Status::check_unsupported(status, "failed to parse surface temperature")? {
            dive.temperature_surface = Some(temp_surface);
        }

        // Gas mixes
        let mut num_gases: u32 = 0;
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_GASMIX_COUNT,
            0,
            &mut num_gases as *mut _ as *mut c_void,
        );
        Status::check_unsupported(status, "failed to parse gasmix count")?;

        for i in 0..num_gases {
            let mut gasmix = std::mem::MaybeUninit::<ffi::dc_gasmix_t>::zeroed().assume_init();
            let status = ffi::dc_parser_get_field(
                parser,
                ffi::DC_FIELD_GASMIX,
                i,
                &mut gasmix as *mut _ as *mut c_void,
            );
            Status::check_unsupported(status, "failed to parse gasmix")?;
            dive.gasmixes.push(Gasmix::from(gasmix));
        }

        // Tanks
        let mut num_tanks: u32 = 0;
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_TANK_COUNT,
            0,
            &mut num_tanks as *mut _ as *mut c_void,
        );
        Status::check_unsupported(status, "failed to parse tank count")?;

        for i in 0..num_tanks {
            let mut tank = std::mem::MaybeUninit::<ffi::dc_tank_t>::zeroed().assume_init();
            let status = ffi::dc_parser_get_field(
                parser,
                ffi::DC_FIELD_TANK,
                i,
                &mut tank as *mut _ as *mut c_void,
            );
            Status::check_unsupported(status, "failed to parse tank")?;
            dive.tanks.push(Tank::from(tank));
        }

        // Dive mode
        let mut divemode = ffi::DC_DIVEMODE_OC;
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_DIVEMODE,
            0,
            &mut divemode as *mut _ as *mut c_void,
        );
        Status::check_unsupported(status, "failed to parse dive mode")?;
        dive.dive_mode = DiveMode::from(divemode);

        // Deco model
        let mut decomodel = std::mem::MaybeUninit::<ffi::dc_decomodel_t>::zeroed().assume_init();
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_DECOMODEL,
            0,
            &mut decomodel as *mut _ as *mut c_void,
        );
        Status::check_unsupported(status, "failed to parse deco model")?;
        dive.deco_model = DecoModel::from(decomodel);

        // Salinity
        let mut salinity = std::mem::MaybeUninit::<ffi::dc_salinity_t>::zeroed().assume_init();
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_SALINITY,
            0,
            &mut salinity as *mut _ as *mut c_void,
        );
        if Status::check_unsupported(status, "failed to parse salinity")? {
            dive.salinity = Some(Salinity::from(salinity));
        }

        // Atmospheric pressure
        let mut atmospheric: f64 = 0.0;
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_ATMOSPHERIC,
            0,
            &mut atmospheric as *mut _ as *mut c_void,
        );
        if Status::check_unsupported(status, "failed to parse atmospheric pressure")? {
            dive.atmospheric_pressure = Some(atmospheric);
        }

        // String fields (metadata)
        for idx in 0u32.. {
            let mut field = std::mem::MaybeUninit::<ffi::dc_field_string_t>::zeroed().assume_init();
            let status = ffi::dc_parser_get_field(
                parser,
                ffi::DC_FIELD_STRING,
                idx,
                &mut field as *mut _ as *mut c_void,
            );
            if !Status::check_unsupported(status, "failed to parse string field")?
                || field.desc.is_null()
                || field.value.is_null()
            {
                break;
            }

            let key = CStr::from_ptr(field.desc).to_string_lossy().into_owned();
            let value = CStr::from_ptr(field.value).to_string_lossy().into_owned();
            dive.metadata.insert(key, value);
        }

        // Location
        let mut location = std::mem::MaybeUninit::<ffi::dc_location_t>::zeroed().assume_init();
        let status = ffi::dc_parser_get_field(
            parser,
            ffi::DC_FIELD_LOCATION,
            0,
            &mut location as *mut _ as *mut c_void,
        );
        if Status::check_unsupported(status, "failed to parse location")? {
            dive.location = Some(Location::from(location));
        }

        Ok(dive)
    }
}

extern "C" fn sample_callback(
    kind: ffi::dc_sample_type_t,
    pvalue: *const ffi::dc_sample_value_t,
    userdata: *mut c_void,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let parse_data = from_void_ptr::<ParseData>(userdata);
        let value = *pvalue;

        match kind {
            ffi::DC_SAMPLE_TIME => {
                let prev = std::mem::take(&mut parse_data.sample);
                parse_data.sample = DiveSample::carry_forward(&prev);
                parse_data.sample.time = Duration::from_millis(value.time as u64);

                if prev.time.as_millis() > 0 {
                    parse_data.dive.samples.push(prev);
                }
            }

            ffi::DC_SAMPLE_DEPTH => {
                parse_data.sample.depth = value.depth;
            }

            ffi::DC_SAMPLE_PRESSURE => {
                let idx = value.pressure.tank as usize;
                let val = value.pressure.value;
                if let Some(p) = parse_data.sample.pressure.get_mut(idx) {
                    *p = val;
                } else {
                    // Extend to fit
                    while parse_data.sample.pressure.len() <= idx {
                        parse_data.sample.pressure.push(0.0);
                    }
                    parse_data.sample.pressure[idx] = val;
                }
            }

            ffi::DC_SAMPLE_TEMPERATURE => {
                parse_data.sample.temperature = Some(value.temperature);
            }

            ffi::DC_SAMPLE_EVENT => {
                let kind = EventKind::from(value.event.type_);
                let time =
                    Duration::from_secs(value.event.time as u64 + parse_data.sample.time.as_secs());
                let name = if value.event.name.is_null() {
                    None
                } else {
                    Some(
                        CStr::from_ptr(value.event.name)
                            .to_string_lossy()
                            .into_owned(),
                    )
                };
                parse_data.sample.events.push(DiveEvent {
                    kind,
                    time,
                    flags: value.event.flags,
                    value: value.event.value,
                    name,
                });
            }

            ffi::DC_SAMPLE_GASMIX => {
                let idx = value.gasmix as usize;
                parse_data.sample.gasmix = parse_data.dive.gasmixes.get(idx).cloned();
            }

            ffi::DC_SAMPLE_O2SENSOR => {
                parse_data.sample.o2_sensor.push(O2Sensor {
                    sensor: Sensor::from(value.o2sensor.sensor),
                    ppo2: value.o2sensor.ppo2,
                    millivolt: value.o2sensor.millivolt,
                });
            }

            ffi::DC_SAMPLE_RBT => {
                parse_data.sample.rbt = Some(Duration::from_secs(value.rbt as u64));
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

            ffi::DC_SAMPLE_CNS => {
                parse_data.sample.cns = value.cns;
            }

            ffi::DC_SAMPLE_DECO => {
                parse_data.sample.deco = Some(Deco {
                    kind: DecoKind::new(value.deco.type_, value.deco.depth),
                    time: Duration::from_secs(value.deco.time as u64),
                    tts: Duration::from_secs(value.deco.tts as u64),
                });
            }

            ffi::DC_SAMPLE_TTS => {
                parse_data.sample.tts = Some(Duration::from_secs(value.time as u64));
            }

            ffi::DC_SAMPLE_VENDOR => {
                // Vendor samples are ignored for now.
            }

            _ => {}
        }
    }));
}
