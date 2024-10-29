use std::ffi::CStr;

use libdivecomputer_sys::dc_version;

fn main() {
    unsafe {
        let res = dc_version(std::ptr::null_mut());
        let version = CStr::from_ptr(res.cast_mut());

        println!("libdivecomputer version {}\n", version.to_string_lossy());
    }
}
