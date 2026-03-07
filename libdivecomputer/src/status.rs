use std::fmt;

use libdivecomputer_sys as ffi;
use serde::Serialize;
use serde_repr::Deserialize_repr;

use crate::error::{LibError, Result};

/// FFI status codes returned by libdivecomputer C functions.
#[repr(i32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr)]
#[non_exhaustive]
pub enum Status {
    Success = 0,
    Done = 1,
    Unsupported = -1,
    InvalidArgs = -2,
    NoMemory = -3,
    NoDevice = -4,
    NoAccess = -5,
    Io = -6,
    Timeout = -7,
    Protocol = -8,
    DataFormat = -9,
    Cancelled = -10,
}

impl Status {
    /// Check an FFI return code. Returns `Ok(())` on success, `Err` otherwise.
    pub(crate) fn check(rc: ffi::dc_status_t, context: &str) -> Result<()> {
        if rc == ffi::DC_STATUS_SUCCESS {
            Ok(())
        } else {
            Err(LibError::status_with_context(rc, context))
        }
    }

    /// Check an FFI return code that may return `DC_STATUS_UNSUPPORTED`.
    /// Returns `Ok(true)` if supported and successful, `Ok(false)` if unsupported,
    /// and `Err` for real errors.
    pub(crate) fn check_unsupported(rc: ffi::dc_status_t, context: &str) -> Result<bool> {
        if rc == ffi::DC_STATUS_SUCCESS {
            Ok(true)
        } else if rc == ffi::DC_STATUS_UNSUPPORTED {
            Ok(false)
        } else {
            Err(LibError::status_with_context(rc, context))
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Done => write!(f, "done"),
            Self::Unsupported => write!(f, "unsupported"),
            Self::InvalidArgs => write!(f, "invalid arguments"),
            Self::NoMemory => write!(f, "out of memory"),
            Self::NoDevice => write!(f, "no device"),
            Self::NoAccess => write!(f, "no access"),
            Self::Io => write!(f, "I/O error"),
            Self::Timeout => write!(f, "timeout"),
            Self::Protocol => write!(f, "protocol error"),
            Self::DataFormat => write!(f, "data format error"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl TryFrom<u32> for Status {
    type Error = String;

    fn try_from(value: u32) -> std::result::Result<Status, Self::Error> {
        Self::try_from(value as i32)
    }
}

impl TryFrom<i32> for Status {
    type Error = String;

    fn try_from(value: i32) -> std::result::Result<Status, Self::Error> {
        match value {
            0 => Ok(Self::Success),
            1 => Ok(Self::Done),
            -1 => Ok(Self::Unsupported),
            -2 => Ok(Self::InvalidArgs),
            -3 => Ok(Self::NoMemory),
            -4 => Ok(Self::NoDevice),
            -5 => Ok(Self::NoAccess),
            -6 => Ok(Self::Io),
            -7 => Ok(Self::Timeout),
            -8 => Ok(Self::Protocol),
            -9 => Ok(Self::DataFormat),
            -10 => Ok(Self::Cancelled),
            _ => Err(format!("Invalid status: {value}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_i32_all_known_codes() {
        let cases: &[(i32, Status)] = &[
            (0, Status::Success),
            (1, Status::Done),
            (-1, Status::Unsupported),
            (-2, Status::InvalidArgs),
            (-3, Status::NoMemory),
            (-4, Status::NoDevice),
            (-5, Status::NoAccess),
            (-6, Status::Io),
            (-7, Status::Timeout),
            (-8, Status::Protocol),
            (-9, Status::DataFormat),
            (-10, Status::Cancelled),
        ];
        for &(code, expected) in cases {
            assert_eq!(Status::try_from(code).unwrap(), expected);
        }
    }

    #[test]
    fn try_from_i32_unknown_returns_err() {
        assert!(Status::try_from(42i32).is_err());
        assert!(Status::try_from(-100i32).is_err());
    }

    #[test]
    fn try_from_u32_delegates() {
        assert_eq!(Status::try_from(0u32).unwrap(), Status::Success);
        assert_eq!(Status::try_from(1u32).unwrap(), Status::Done);
    }

    #[test]
    fn check_success() {
        assert!(Status::check(ffi::DC_STATUS_SUCCESS, "test").is_ok());
    }

    #[test]
    fn check_error() {
        let err = Status::check(ffi::DC_STATUS_IO, "test io").unwrap_err();
        match err {
            LibError::Status(Status::Io, Some(ctx)) => assert_eq!(ctx, "test io"),
            _ => panic!("Expected Status(Io, Some), got {err:?}"),
        }
    }

    #[test]
    fn check_unsupported_success() {
        assert_eq!(
            Status::check_unsupported(ffi::DC_STATUS_SUCCESS, "test").unwrap(),
            true
        );
    }

    #[test]
    fn check_unsupported_unsupported() {
        assert_eq!(
            Status::check_unsupported(ffi::DC_STATUS_UNSUPPORTED, "test").unwrap(),
            false
        );
    }

    #[test]
    fn check_unsupported_error() {
        assert!(Status::check_unsupported(ffi::DC_STATUS_IO, "test").is_err());
    }
}
