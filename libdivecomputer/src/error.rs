use crate::status::Status;

/// The main error type for this crate.
#[derive(Debug, thiserror::Error)]
pub enum LibError {
    /// A libdivecomputer FFI status error.
    #[error("libdivecomputer: {1:?}: {0:?}")]
    Status(Status, Option<String>),

    /// Invalid arguments provided.
    #[error("invalid argument: {0}")]
    InvalidArguments(String),

    /// Device not found or not accessible.
    #[error("device error: {0}")]
    DeviceError(String),

    /// Parse error when reading dive data.
    #[error("parse error: {0}")]
    ParseError(String),

    /// Requested descriptor not found.
    #[error("descriptor not found: {0}")]
    DescriptorNotFound(String),

    /// Transport not supported for this operation.
    #[error("transport not supported: {0}")]
    TransportNotSupported(String),

    /// No Bluetooth adapter available.
    #[error("no bluetooth adapter found")]
    NoBluetoothAdapter,

    /// BLE device not found during scan.
    #[error("BLE device not found: {0}")]
    BleDeviceNotFound(String),

    /// BLE service not found on device.
    #[error("BLE service not found: {0}")]
    BleServiceNotFound(String),

    /// I/O error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// Btleplug error.
    #[cfg(feature = "ble")]
    #[error(transparent)]
    Btleplug(#[from] btleplug::Error),

    /// Integer parse error.
    #[error("parse error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    /// Incompatible library version.
    #[error("invalid version (expected: {expected}), (found: {found})")]
    InvalidVersion { expected: String, found: String },

    /// UTF-8 conversion error.
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),

    /// Jiff error.
    #[error(transparent)]
    Jiff(#[from] jiff::Error),

    /// Null pointer error.
    #[error("null pointer")]
    NullPointer,

    /// Operation was cancelled.
    #[error("cancelled")]
    Cancelled,

    /// Unknown error.
    #[error("unknown error")]
    Unknown,

    /// Download partially succeeded: some dives parsed, but errors occurred.
    #[error("partial download: {} dives ok, {} errors", dives.len(), errors.len())]
    PartialDownload {
        /// Successfully parsed dives.
        dives: Vec<crate::parser::Dive>,
        /// Errors encountered during parsing.
        errors: Vec<LibError>,
    },
}

impl LibError {
    /// Create a status error from an FFI return code.
    ///
    /// Returns `Unknown` if the code doesn't map to a known `Status` variant,
    /// which can happen if the C library adds new status codes.
    pub fn status<T>(rc: T) -> Self
    where
        T: TryInto<Status>,
    {
        match rc.try_into() {
            Ok(status) => Self::Status(status, None),
            Err(_) => Self::Unknown,
        }
    }

    /// Create a status error with additional context about the operation that failed.
    ///
    /// Returns `Unknown` if the code doesn't map to a known `Status` variant,
    /// which can happen if the C library adds new status codes.
    pub fn status_with_context<T>(rc: T, context: impl ToString) -> Self
    where
        T: TryInto<Status>,
    {
        match rc.try_into() {
            Ok(status) => Self::Status(status, Some(context.to_string())),
            Err(_) => Self::Unknown,
        }
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
    fn test_error_display() {
        let error = LibError::DeviceError("Test device error".to_string());
        assert_eq!(error.to_string(), "device error: Test device error");
    }

    #[test]
    fn status_with_valid_code() {
        let error = LibError::status(libdivecomputer_sys::DC_STATUS_IO);
        match error {
            LibError::Status(Status::Io, None) => {}
            _ => panic!("Expected Status(Io, None), got {error:?}"),
        }
    }

    #[test]
    fn status_with_unknown_code_returns_unknown() {
        let error = LibError::status(999i32);
        assert!(matches!(error, LibError::Unknown));
    }

    #[test]
    fn status_with_context_preserves_context() {
        let error =
            LibError::status_with_context(libdivecomputer_sys::DC_STATUS_TIMEOUT, "test context");
        match error {
            LibError::Status(Status::Timeout, Some(ctx)) => {
                assert_eq!(ctx, "test context");
            }
            _ => panic!("Expected Status(Timeout, Some), got {error:?}"),
        }
    }

    #[test]
    fn status_with_context_unknown_code() {
        let error = LibError::status_with_context(999i32, "ignored");
        assert!(matches!(error, LibError::Unknown));
    }

    #[test]
    fn from_nul_error() {
        let nul_err = std::ffi::CString::new("hello\0world").unwrap_err();
        let error = LibError::from(nul_err);
        assert!(matches!(error, LibError::InvalidArguments(_)));
    }
}
