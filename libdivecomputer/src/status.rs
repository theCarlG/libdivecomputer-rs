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
