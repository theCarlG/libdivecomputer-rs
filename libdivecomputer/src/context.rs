use std::{
    ffi::{CStr, c_char, c_uint, c_void},
    fmt::Display,
    ptr,
};

use libdivecomputer_sys as ffi;

use serde::{Deserialize, Serialize};

use crate::{
    error::{LibError, Result},
    status::Status,
    transport::TransportSet,
};

type LogCallback = Box<dyn Fn(LogLevel, &str) + Send + Sync>;

/// Wrapper around `dc_context_t`.
pub struct Context {
    pub(crate) ptr: *mut ffi::dc_context_t,
    /// Stored so the closure is freed on drop.
    _log_callback: Option<Box<LogCallback>>,
}

impl Context {
    /// Create a new context. Prefer `Context::builder()` for configuration.
    pub fn new() -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let status = unsafe { ffi::dc_context_new(&mut ptr) };
        Status::check(status, "failed to create context")?;
        Ok(Self {
            ptr,
            _log_callback: None,
        })
    }

    /// Create a context builder for configuration.
    pub fn builder() -> ContextBuilder {
        ContextBuilder::default()
    }

    pub(crate) fn ptr(&self) -> *mut ffi::dc_context_t {
        self.ptr
    }

    /// Set the log level.
    pub fn set_loglevel(&mut self, loglevel: LogLevel) -> Result<()> {
        let status = unsafe { ffi::dc_context_set_loglevel(self.ptr, loglevel as _) };
        Status::check(status, "failed to set loglevel")
    }

    /// Set the log callback function.
    pub fn set_logfunc<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(LogLevel, &str) + Send + Sync + 'static,
    {
        // Double-box: Box<dyn Fn> is a fat pointer, but we need a thin *mut c_void
        // for the C callback. Box<Box<dyn Fn>> gives us a thin pointer.
        let boxed: LogCallback = Box::new(callback);
        let raw = Box::into_raw(Box::new(boxed));

        let status = unsafe {
            ffi::dc_context_set_logfunc(self.ptr, Some(log_callback_wrapper), raw as *mut _)
        };

        if status != ffi::DC_STATUS_SUCCESS {
            // Reclaim the box to avoid leak on error.
            unsafe { drop(Box::from_raw(raw)) };
            return Err(LibError::status_with_context(
                status,
                "failed to set logfunc",
            ));
        }

        // Keep the double-boxed pointer alive — C holds `raw` as userdata.
        self._log_callback = Some(unsafe { Box::from_raw(raw) });

        Ok(())
    }

    /// Get the set of transports supported on this platform.
    pub fn get_transports(&self) -> TransportSet {
        if self.ptr.is_null() {
            return TransportSet::from_bits(0);
        }
        let bits = unsafe { ffi::dc_context_get_transports(self.ptr as *mut _) };
        TransportSet::from_bits(bits)
    }
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("open", &!self.ptr.is_null())
            .finish()
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                ffi::dc_context_free(self.ptr);
            }
        }
    }
}

// SAFETY: dc_context_t is only used to pass configuration to other FFI calls.
// The context pointer is not mutated after creation except through &mut self methods.
unsafe impl Send for Context {}
unsafe impl Sync for Context {}

/// Builder for `Context`.
#[derive(Default)]
pub struct ContextBuilder {
    log_level: Option<LogLevel>,
    log_fn: Option<LogCallback>,
}

impl std::fmt::Debug for ContextBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextBuilder")
            .field("log_level", &self.log_level)
            .field("log_fn", &self.log_fn.as_ref().map(|_| ".."))
            .finish()
    }
}

impl ContextBuilder {
    pub fn log_level(mut self, level: LogLevel) -> Self {
        self.log_level = Some(level);
        self
    }

    pub fn log_fn<F>(mut self, f: F) -> Self
    where
        F: Fn(LogLevel, &str) + Send + Sync + 'static,
    {
        self.log_fn = Some(Box::new(f));
        self
    }

    pub fn build(self) -> Result<Context> {
        let mut ctx = Context::new()?;

        if let Some(level) = self.log_level {
            ctx.set_loglevel(level)?;
        }

        if let Some(callback) = self.log_fn {
            ctx.set_logfunc(callback)?;
        }

        Ok(ctx)
    }
}

/// Log level for the libdivecomputer context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
#[non_exhaustive]
pub enum LogLevel {
    None = ffi::DC_LOGLEVEL_NONE,
    Error = ffi::DC_LOGLEVEL_ERROR,
    Warning = ffi::DC_LOGLEVEL_WARNING,
    Info = ffi::DC_LOGLEVEL_INFO,
    Debug = ffi::DC_LOGLEVEL_DEBUG,
    All = ffi::DC_LOGLEVEL_ALL,
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, ""),
            Self::Error => write!(f, "Error"),
            Self::Warning => write!(f, "Warning"),
            Self::Info => write!(f, "Info"),
            Self::Debug => write!(f, "Debug"),
            Self::All => write!(f, "All"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_new_succeeds() {
        let ctx = Context::new();
        assert!(ctx.is_ok());
    }

    #[test]
    fn context_builder_build_succeeds() {
        let ctx = Context::builder().build();
        assert!(ctx.is_ok());
    }

    #[test]
    fn context_builder_with_log_level() {
        let ctx = Context::builder().log_level(LogLevel::Debug).build();
        assert!(ctx.is_ok());
    }

    #[test]
    fn log_level_display() {
        assert_eq!(LogLevel::Error.to_string(), "Error");
        assert_eq!(LogLevel::Warning.to_string(), "Warning");
        assert_eq!(LogLevel::Info.to_string(), "Info");
        assert_eq!(LogLevel::Debug.to_string(), "Debug");
        assert_eq!(LogLevel::All.to_string(), "All");
        assert_eq!(LogLevel::None.to_string(), "");
    }

    #[test]
    fn context_get_transports() {
        let ctx = Context::new().unwrap();
        let transports = ctx.get_transports();
        // On a real system, at least serial should be available
        let _ = transports.to_vec();
    }
}

extern "C" fn log_callback_wrapper(
    _context: *mut ffi::dc_context_t,
    loglevel: ffi::dc_loglevel_t,
    _file: *const c_char,
    _line: c_uint,
    _function: *const c_char,
    message: *const c_char,
    userdata: *mut c_void,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        let callback = &*(userdata as *const Box<dyn Fn(LogLevel, &str) + Send + Sync>);
        let level = match loglevel {
            ffi::DC_LOGLEVEL_ERROR => LogLevel::Error,
            ffi::DC_LOGLEVEL_WARNING => LogLevel::Warning,
            ffi::DC_LOGLEVEL_INFO => LogLevel::Info,
            ffi::DC_LOGLEVEL_DEBUG => LogLevel::Debug,
            _ => LogLevel::None,
        };

        if let Ok(msg) = CStr::from_ptr(message).to_str() {
            callback(level, msg);
        }
    }));
}
