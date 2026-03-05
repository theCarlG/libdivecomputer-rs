use std::ffi::{CString, c_uint};

use libdivecomputer_sys as ffi;

use crate::device::Device;
use crate::error::Result;
use crate::status::Status;

/// Read the device version.
pub fn version(device: &Device, buf: &mut [u8]) -> Result<()> {
    let status = unsafe {
        ffi::hw_ostc3_device_version(device.raw_ptr(), buf.as_mut_ptr(), buf.len() as c_uint)
    };
    Status::check(status, "hw_ostc3: failed to read version")
}

/// Read the hardware info.
pub fn hardware(device: &Device, buf: &mut [u8]) -> Result<()> {
    let status = unsafe {
        ffi::hw_ostc3_device_hardware(device.raw_ptr(), buf.as_mut_ptr(), buf.len() as c_uint)
    };
    Status::check(status, "hw_ostc3: failed to read hardware info")
}

/// Display text on the device screen.
pub fn display(device: &Device, text: &str) -> Result<()> {
    let c_text = CString::new(text)?;
    let status = unsafe { ffi::hw_ostc3_device_display(device.raw_ptr(), c_text.as_ptr()) };
    Status::check(status, "hw_ostc3: failed to display text")
}

/// Set custom text on the device.
pub fn customtext(device: &Device, text: &str) -> Result<()> {
    let c_text = CString::new(text)?;
    let status = unsafe { ffi::hw_ostc3_device_customtext(device.raw_ptr(), c_text.as_ptr()) };
    Status::check(status, "hw_ostc3: failed to set custom text")
}

/// Read device configuration.
pub fn config_read(device: &Device, config: u32, buf: &mut [u8]) -> Result<()> {
    let status = unsafe {
        ffi::hw_ostc3_device_config_read(
            device.raw_ptr(),
            config,
            buf.as_mut_ptr(),
            buf.len() as c_uint,
        )
    };
    Status::check(status, "hw_ostc3: failed to read config")
}

/// Write device configuration.
pub fn config_write(device: &Device, config: u32, data: &[u8]) -> Result<()> {
    let status = unsafe {
        ffi::hw_ostc3_device_config_write(
            device.raw_ptr(),
            config,
            data.as_ptr(),
            data.len() as c_uint,
        )
    };
    Status::check(status, "hw_ostc3: failed to write config")
}

/// Reset device configuration to defaults.
pub fn config_reset(device: &Device) -> Result<()> {
    let status = unsafe { ffi::hw_ostc3_device_config_reset(device.raw_ptr()) };
    Status::check(status, "hw_ostc3: failed to reset config")
}

/// Update firmware.
pub fn firmware_update(device: &Device, filename: &str, force: bool) -> Result<()> {
    let c_filename = CString::new(filename)?;
    let status =
        unsafe { ffi::hw_ostc3_device_fwupdate(device.raw_ptr(), c_filename.as_ptr(), force) };
    Status::check(status, "hw_ostc3: failed to update firmware")
}
