use std::{
    ffi::{CStr, c_char, c_uint, c_void},
    fmt::Display,
    ptr,
};

use libdivecomputer_sys as ffi;

use crate::{
    device::Transport,
    error::{LibError, Result},
};

#[derive(Debug, Clone, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub struct Context {
    pub(crate) ptr: *mut ffi::dc_context_t,
}

impl Default for Context {
    fn default() -> Self {
        let mut ptr = ptr::null_mut();

        let status = unsafe { ffi::dc_context_new(&mut ptr) };
        if status != ffi::DC_STATUS_SUCCESS {
            panic!("failed to create context:{status}")
        }

        Self { ptr }
    }
}

impl Context {
    pub(crate) fn ptr(&self) -> *mut ffi::dc_context_t {
        self.ptr
    }

    pub fn set_loglevel(&mut self, loglevel: LogLevel) -> Result<()> {
        let status = unsafe { ffi::dc_context_set_loglevel(self.ptr, loglevel as _) };

        if status == ffi::DC_STATUS_SUCCESS {
            Ok(())
        } else {
            Err(LibError::status_with_context(
                status,
                "failed to set loglevel",
            ))
        }
    }

    pub fn set_logfunc<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(LogLevel, &str) + 'static,
    {
        let status = unsafe {
            ffi::dc_context_set_logfunc(
                self.ptr,
                Some(log_callback_wrapper::<F>),
                Box::into_raw(Box::new(callback)) as *mut _,
            )
        };

        if status == ffi::DC_STATUS_SUCCESS {
            Ok(())
        } else {
            Err(LibError::status_with_context(
                status,
                "failed to set logfunc",
            ))
        }
    }

    pub fn get_transports(&self) -> Vec<Transport> {
        if self.ptr.is_null() {
            return Vec::new();
        }
        unsafe { Transport::vec_from_bitflag(ffi::dc_context_get_transports(self.ptr as *mut _)) }
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

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

// Log level enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
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

// Callback wrapper
extern "C" fn log_callback_wrapper<F>(
    _context: *mut ffi::dc_context_t,
    loglevel: ffi::dc_loglevel_t,
    _file: *const c_char,
    _line: c_uint,
    _function: *const c_char,
    message: *const c_char,
    userdata: *mut c_void,
) where
    F: Fn(LogLevel, &str),
{
    unsafe {
        let callback = &*(userdata as *const F);
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
    }
}
