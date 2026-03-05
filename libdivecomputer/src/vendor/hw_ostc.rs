use std::ffi::{CString, c_uint};

use libdivecomputer_sys as ffi;

use crate::buffer::Buffer;
use crate::device::Device;
use crate::error::Result;
use crate::status::Status;

/// Read the MD2 hash from an OSTC device.
pub fn md2hash(device: &Device, buf: &mut [u8]) -> Result<()> {
    let status = unsafe {
        ffi::hw_ostc_device_md2hash(device.raw_ptr(), buf.as_mut_ptr(), buf.len() as c_uint)
    };
    Status::check(status, "hw_ostc: failed to read MD2 hash")
}

/// Read EEPROM data from an OSTC device.
pub fn eeprom_read(device: &Device, bank: u32, buf: &mut [u8]) -> Result<()> {
    let status = unsafe {
        ffi::hw_ostc_device_eeprom_read(
            device.raw_ptr(),
            bank,
            buf.as_mut_ptr(),
            buf.len() as c_uint,
        )
    };
    Status::check(status, "hw_ostc: failed to read EEPROM")
}

/// Write EEPROM data to an OSTC device.
pub fn eeprom_write(device: &Device, bank: u32, data: &[u8]) -> Result<()> {
    let status = unsafe {
        ffi::hw_ostc_device_eeprom_write(
            device.raw_ptr(),
            bank,
            data.as_ptr(),
            data.len() as c_uint,
        )
    };
    Status::check(status, "hw_ostc: failed to write EEPROM")
}

/// Reset an OSTC device.
pub fn reset(device: &Device) -> Result<()> {
    let status = unsafe { ffi::hw_ostc_device_reset(device.raw_ptr()) };
    Status::check(status, "hw_ostc: failed to reset device")
}

/// Take a screenshot from an OSTC device.
pub fn screenshot(device: &Device, format: u32) -> Result<Vec<u8>> {
    let buffer = Buffer::new(0);
    let status = unsafe { ffi::hw_ostc_device_screenshot(device.raw_ptr(), buffer.ptr, format) };
    Status::check(status, "hw_ostc: failed to take screenshot")?;
    Ok(buffer.to_vec())
}

/// Update firmware on an OSTC device.
pub fn firmware_update(device: &Device, filename: &str) -> Result<()> {
    let c_filename = CString::new(filename)?;
    let status = unsafe { ffi::hw_ostc_device_fwupdate(device.raw_ptr(), c_filename.as_ptr()) };
    Status::check(status, "hw_ostc: failed to update firmware")
}
