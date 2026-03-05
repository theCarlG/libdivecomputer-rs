use std::ffi::{CString, c_uint};

use libdivecomputer_sys as ffi;

use crate::device::Device;
use crate::error::Result;
use crate::status::Status;

/// Read the device version.
pub fn version(device: &Device, buf: &mut [u8]) -> Result<()> {
    let status = unsafe {
        ffi::hw_frog_device_version(device.raw_ptr(), buf.as_mut_ptr(), buf.len() as c_uint)
    };
    Status::check(status, "hw_frog: failed to read version")
}

/// Display text on the device screen.
pub fn display(device: &Device, text: &str) -> Result<()> {
    let c_text = CString::new(text)?;
    let status = unsafe { ffi::hw_frog_device_display(device.raw_ptr(), c_text.as_ptr()) };
    Status::check(status, "hw_frog: failed to display text")
}

/// Set custom text on the device.
pub fn customtext(device: &Device, text: &str) -> Result<()> {
    let c_text = CString::new(text)?;
    let status = unsafe { ffi::hw_frog_device_customtext(device.raw_ptr(), c_text.as_ptr()) };
    Status::check(status, "hw_frog: failed to set custom text")
}
