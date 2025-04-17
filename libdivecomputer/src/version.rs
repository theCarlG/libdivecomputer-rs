use std::ffi::CStr;

use libdivecomputer_sys::dc_version;

/// Returns the libdivecomputer version.
pub fn version() -> String {
    unsafe {
        let res = dc_version(std::ptr::null_mut());
        CStr::from_ptr(res.cast_mut()).to_string_lossy().to_string()
    }
}
