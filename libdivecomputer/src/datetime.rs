use libdivecomputer_sys as ffi;

/// Get the current time as a `jiff::Timestamp`.
#[allow(dead_code)]
pub(crate) fn now() -> jiff::Timestamp {
    let ticks = unsafe { ffi::dc_datetime_now() };
    jiff::Timestamp::from_second(ticks).expect("invalid timestamp from dc_datetime_now")
}

/// Convert a `dc_datetime_t` to a `jiff::Timestamp`.
pub(crate) fn ffi_to_timestamp(dt: &ffi::dc_datetime_t) -> Result<jiff::Timestamp, jiff::Error> {
    let s = if dt.timezone == i32::MIN {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}Z",
            dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
        )
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
    };
    s.parse()
}

/// Convert a `jiff::Timestamp` to a `dc_datetime_t` in local time.
pub(crate) fn timestamp_to_ffi(ts: jiff::Timestamp) -> ffi::dc_datetime_t {
    let mut dt = unsafe { std::mem::MaybeUninit::<ffi::dc_datetime_t>::zeroed().assume_init() };
    unsafe { ffi::dc_datetime_localtime(&mut dt, ts.as_second()) };
    dt
}
