/// Concrete dive data types produced by [`Parser::parse`]: `Dive`, `DiveSample`,
/// `Gasmix`, `Tank`, `Fingerprint`, and friends.
pub mod types;

use std::{
    ffi::{CStr, c_void},
    mem::MaybeUninit,
    ptr,
    time::Duration,
};

pub use types::*;

use libdivecomputer_sys as ffi;

use crate::{
    common::{EventKind, as_void_ptr, ffi_guard, from_void_ptr},
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
    #[must_use = "the created Parser owns a C allocation"]
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
    #[must_use = "parsed dive data should not be silently discarded"]
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

/// Read an arbitrary scalar/struct field from the parser.
///
/// Returns `Ok(None)` on `DC_STATUS_UNSUPPORTED`, `Err` on a real error, and
/// `Ok(Some(value))` on success. The buffer is left uninitialised until the
/// C library writes to it on success — avoiding the `MaybeUninit::zeroed`
/// fast-path means we don't rely on "all zeros is a valid bit pattern" for
/// the target FFI struct.
///
/// # Safety
/// `parser` must be a valid `dc_parser_t`, and the caller must pick a `T`
/// whose layout matches what `dc_parser_get_field` populates for the given
/// `field` index. Picking the wrong `T` is UB.
unsafe fn get_field<T>(
    parser: *mut ffi::dc_parser_t,
    field: ffi::dc_field_type_t,
    idx: u32,
    ctx: &str,
) -> Result<Option<T>> {
    let mut value = MaybeUninit::<T>::uninit();
    let status =
        unsafe { ffi::dc_parser_get_field(parser, field, idx, value.as_mut_ptr().cast()) };
    if Status::check_unsupported(status, ctx)? {
        Ok(Some(unsafe { value.assume_init() }))
    } else {
        Ok(None)
    }
}

fn parse_fields(parser: *mut ffi::dc_parser_t) -> Result<Dive> {
    let mut dive = Dive::default();

    // Datetime (uses a dedicated FFI entry point, not dc_parser_get_field).
    let mut dt = MaybeUninit::<ffi::dc_datetime_t>::uninit();
    let status = unsafe { ffi::dc_parser_get_datetime(parser, dt.as_mut_ptr()) };
    if Status::check_unsupported(status, "failed to parse datetime")? {
        let dt = unsafe { dt.assume_init() };
        dive.start = crate::datetime::ffi_to_timestamp(&dt)?;
    }

    // Required-ish scalar fields. If UNSUPPORTED, fall back to default.
    if let Some(divetime) =
        unsafe { get_field::<u32>(parser, ffi::DC_FIELD_DIVETIME, 0, "divetime")? }
    {
        dive.duration = Duration::from_secs(divetime as u64);
    }
    if let Some(max_depth) =
        unsafe { get_field::<f64>(parser, ffi::DC_FIELD_MAXDEPTH, 0, "max depth")? }
    {
        dive.max_depth = max_depth;
    }

    // Optional scalar fields.
    dive.avg_depth = unsafe { get_field(parser, ffi::DC_FIELD_AVGDEPTH, 0, "avg depth")? };
    dive.temperature_maximum = unsafe {
        get_field(
            parser,
            ffi::DC_FIELD_TEMPERATURE_MAXIMUM,
            0,
            "max temperature",
        )?
    };
    dive.temperature_minimum = unsafe {
        get_field(
            parser,
            ffi::DC_FIELD_TEMPERATURE_MINIMUM,
            0,
            "min temperature",
        )?
    };
    dive.temperature_surface = unsafe {
        get_field(
            parser,
            ffi::DC_FIELD_TEMPERATURE_SURFACE,
            0,
            "surface temperature",
        )?
    };
    dive.atmospheric_pressure = unsafe {
        get_field(
            parser,
            ffi::DC_FIELD_ATMOSPHERIC,
            0,
            "atmospheric pressure",
        )?
    };

    // Gas mixes.
    let num_gases: u32 =
        unsafe { get_field(parser, ffi::DC_FIELD_GASMIX_COUNT, 0, "gasmix count")? }
            .unwrap_or(0);
    for i in 0..num_gases {
        if let Some(gm) =
            unsafe { get_field::<ffi::dc_gasmix_t>(parser, ffi::DC_FIELD_GASMIX, i, "gasmix")? }
        {
            dive.gasmixes.push(Gasmix::from(gm));
        }
    }

    // Tanks.
    let num_tanks: u32 =
        unsafe { get_field(parser, ffi::DC_FIELD_TANK_COUNT, 0, "tank count")? }.unwrap_or(0);
    for i in 0..num_tanks {
        if let Some(tank) =
            unsafe { get_field::<ffi::dc_tank_t>(parser, ffi::DC_FIELD_TANK, i, "tank")? }
        {
            dive.tanks.push(Tank::from(tank));
        }
    }

    // Dive mode — fall back to open-circuit if unsupported, matching the
    // previous behaviour.
    let divemode = unsafe {
        get_field::<ffi::dc_divemode_t>(parser, ffi::DC_FIELD_DIVEMODE, 0, "dive mode")?
    }
    .unwrap_or(ffi::DC_DIVEMODE_OC);
    dive.dive_mode = DiveMode::from(divemode);

    // Deco model.
    if let Some(dm) = unsafe {
        get_field::<ffi::dc_decomodel_t>(parser, ffi::DC_FIELD_DECOMODEL, 0, "deco model")?
    } {
        dive.deco_model = DecoModel::from(dm);
    }

    // Salinity and location.
    dive.salinity = unsafe {
        get_field::<ffi::dc_salinity_t>(parser, ffi::DC_FIELD_SALINITY, 0, "salinity")?
    }
    .map(Salinity::from);
    dive.location = unsafe {
        get_field::<ffi::dc_location_t>(parser, ffi::DC_FIELD_LOCATION, 0, "location")?
    }
    .map(Location::from);

    // String fields (metadata). Iterate until the C library reports
    // UNSUPPORTED or returns NULL description/value pointers.
    for idx in 0u32.. {
        let Some(field) = (unsafe {
            get_field::<ffi::dc_field_string_t>(
                parser,
                ffi::DC_FIELD_STRING,
                idx,
                "string field",
            )?
        }) else {
            break;
        };
        if field.desc.is_null() || field.value.is_null() {
            break;
        }
        let key = unsafe { CStr::from_ptr(field.desc).to_string_lossy().into_owned() };
        let value = unsafe { CStr::from_ptr(field.value).to_string_lossy().into_owned() };
        dive.metadata.insert(key, value);
    }

    Ok(dive)
}

extern "C" fn sample_callback(
    kind: ffi::dc_sample_type_t,
    pvalue: *const ffi::dc_sample_value_t,
    userdata: *mut c_void,
) {
    ffi_guard(|| unsafe {
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
    })
}
