//! Error types for the libdivecomputer crate.

use std::fmt;

use crate::common::Status;

/// The main error type for this crate.
#[derive(Debug, thiserror::Error)]
pub enum LibError {
    /// A libdivecomputer status error
    #[error("libdivecomputer: {1:?}: {0:?}")]
    Status(Status, Option<String>),

    /// Invalid arguments provided
    #[error("invalid argument: {0}")]
    InvalidArguments(String),

    /// Device not found or not accessible
    #[error("device error: {0}")]
    DeviceError(String),

    /// Parse error when reading dive data
    #[error("parse error: {0}")]
    ParseError(String),

    /// I/O error
    #[error(transparent)]
    Io(#[from] std::io::Error),
    //
    // /// Btleplug error
    #[error(transparent)]
    Btleplug(#[from] btleplug::Error),

    /// Parse error when reading dive data
    #[error("parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    /// Incompatible library version
    #[error("invalid version (expected: {expected}), (found: {found})")]
    InvalidVersion {
        /// Expected version
        expected: String,
        /// Found version
        found: String,
    },

    /// UTF-8 conversion error
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),

    /// std lib channel recv error
    #[error(transparent)]
    Recv(#[from] std::sync::mpsc::RecvError),

    /// Null pointer error
    #[error("null pointer")]
    NullPointer,

    /// Generic error with message
    #[error("unknown error: {0}")]
    Other(String),

    #[error("unknown error")]
    Unknown,
}

impl LibError {
    pub fn status<T>(rc: T) -> Self
    where
        T: TryInto<Status>,
        <T as TryInto<Status>>::Error: fmt::Debug,
    {
        Self::Status(rc.try_into().unwrap(), None)
    }

    pub fn status_with_context<T>(rc: T, context: impl ToString) -> Self
    where
        T: TryInto<Status>,
        <T as TryInto<Status>>::Error: fmt::Debug,
    {
        Self::Status(rc.try_into().unwrap(), Some(context.to_string()))
    }
}

impl From<Status> for LibError {
    fn from(status: Status) -> Self {
        Self::Status(status, None)
    }
}

impl From<std::ffi::NulError> for LibError {
    fn from(_: std::ffi::NulError) -> Self {
        Self::InvalidArguments("String contains null byte".to_string())
    }
}

/// A specialized Result type for this crate.
pub type Result<T> = std::result::Result<T, LibError>;

/// Convert a libdivecomputer status code to a Result
pub fn status_to_result(status: i32) -> Result<()> {
    match Status::try_from(status) {
        Ok(Status::Success) => Ok(()),
        Ok(status) => Err(LibError::Status(status, None)),
        Err(_) => Err(LibError::Other(format!("Unknown status code: {status}"))),
    }
}

/// Check for null pointer and return error if null
pub fn check_null_ptr<T>(ptr: *mut T) -> Result<*mut T> {
    if ptr.is_null() {
        Err(LibError::NullPointer)
    } else {
        Ok(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_conversion() {
        let error = LibError::from(Status::NoDevice);
        match error {
            LibError::Status(Status::NoDevice, None) => {}
            _ => panic!("Expected Status error"),
        }
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let error = LibError::from(io_error);
        match error {
            LibError::Io(_) => {}
            _ => panic!("Expected Io error"),
        }
    }

    #[test]
    fn test_status_to_result() {
        assert!(status_to_result(0).is_ok()); // Success
        assert!(status_to_result(-1).is_err()); // Unsupported
        assert!(status_to_result(-4).is_err()); // NoDevice
    }

    #[test]
    fn test_check_null_ptr() {
        let valid_ptr = &mut 42 as *mut i32;
        assert!(check_null_ptr(valid_ptr).is_ok());

        let null_ptr: *mut i32 = std::ptr::null_mut();
        assert!(check_null_ptr(null_ptr).is_err());
    }

    #[test]
    fn test_error_display() {
        let error = LibError::DeviceError("Test device error".to_string());
        assert_eq!(error.to_string(), "device error: Test device error");
    }
}
