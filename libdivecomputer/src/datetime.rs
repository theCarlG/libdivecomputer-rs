use libdivecomputer_sys as ffi;

/// Convert a `dc_datetime_t` to a `jiff::Timestamp`.
pub(crate) fn ffi_to_timestamp(dt: &ffi::dc_datetime_t) -> Result<jiff::Timestamp, jiff::Error> {
    let civil = jiff::civil::date(dt.year as i16, dt.month as i8, dt.day as i8)
        .at(dt.hour as i8, dt.minute as i8, dt.second as i8, 0);
    if dt.timezone == i32::MIN {
        // DC_TIMEZONE_NONE — treat as UTC
        Ok(civil.to_zoned(jiff::tz::TimeZone::UTC)?.timestamp())
    } else {
        let offset = jiff::tz::Offset::from_seconds(dt.timezone)?;
        Ok(offset.to_timestamp(civil)?)
    }
}

/// Convert a `jiff::Timestamp` to a `dc_datetime_t` in local time.
pub(crate) fn timestamp_to_ffi(ts: jiff::Timestamp) -> ffi::dc_datetime_t {
    let mut dt: ffi::dc_datetime_t = unsafe { std::mem::zeroed() };
    unsafe { ffi::dc_datetime_localtime(&mut dt, ts.as_second()) };
    dt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dt(
        year: i32,
        month: i32,
        day: i32,
        hour: i32,
        minute: i32,
        second: i32,
        timezone: i32,
    ) -> ffi::dc_datetime_t {
        ffi::dc_datetime_t {
            year,
            month,
            day,
            hour,
            minute,
            second,
            timezone,
        }
    }

    #[test]
    fn ffi_to_timestamp_utc() {
        // timezone == i32::MIN means UTC (DC_TIMEZONE_NONE)
        let dt = make_dt(2025, 6, 15, 12, 30, 45, i32::MIN);
        let ts = ffi_to_timestamp(&dt).unwrap();
        assert_eq!(ts.to_string(), "2025-06-15T12:30:45Z");
    }

    #[test]
    fn ffi_to_timestamp_positive_offset() {
        // +05:30 = 19800 seconds
        let dt = make_dt(2025, 6, 15, 18, 0, 0, 5 * 3600 + 30 * 60);
        let ts = ffi_to_timestamp(&dt).unwrap();
        // 18:00 at +05:30 = 12:30 UTC
        assert_eq!(ts.to_string(), "2025-06-15T12:30:00Z");
    }

    #[test]
    fn ffi_to_timestamp_negative_offset() {
        // -08:00 = -28800 seconds
        let dt = make_dt(2025, 1, 1, 0, 0, 0, -8 * 3600);
        let ts = ffi_to_timestamp(&dt).unwrap();
        // 00:00 at -08:00 = 08:00 UTC
        assert_eq!(ts.to_string(), "2025-01-01T08:00:00Z");
    }

    #[test]
    fn ffi_to_timestamp_zero_offset() {
        // +00:00
        let dt = make_dt(2025, 3, 7, 10, 0, 0, 0);
        let ts = ffi_to_timestamp(&dt).unwrap();
        assert_eq!(ts.to_string(), "2025-03-07T10:00:00Z");
    }

    #[test]
    fn ffi_to_timestamp_negative_sub_hour_offset() {
        // -00:30 = -1800 seconds — previously produced "+00:30" due to truncation bug
        let dt = make_dt(2025, 1, 1, 0, 0, 0, -1800);
        let ts = ffi_to_timestamp(&dt).unwrap();
        // 00:00 at -00:30 = 00:30 UTC
        assert_eq!(ts.to_string(), "2025-01-01T00:30:00Z");
    }

    #[test]
    fn timestamp_to_ffi_roundtrip() {
        let ts = jiff::Timestamp::from_second(1750000000).unwrap();
        let dt = timestamp_to_ffi(ts);
        // Fields should be valid date components
        assert!(dt.year >= 2025);
        assert!((1..=12).contains(&dt.month));
        assert!((1..=31).contains(&dt.day));
        assert!((0..=23).contains(&dt.hour));
        assert!((0..=59).contains(&dt.minute));
        assert!((0..=59).contains(&dt.second));

        // Converting back should recover the original timestamp
        let ts2 = ffi_to_timestamp(&dt).unwrap();
        assert_eq!(ts.as_second(), ts2.as_second());
    }
}
