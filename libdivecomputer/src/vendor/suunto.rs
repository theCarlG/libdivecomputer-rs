use std::ffi::c_uint;

use libdivecomputer_sys as ffi;

use crate::device::Device;
use crate::error::Result;
use crate::status::Status;

/// Suunto D9 operations.
pub mod d9 {
    use super::*;

    pub fn version(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::suunto_d9_device_version(device.raw_ptr(), buf.as_mut_ptr(), buf.len() as c_uint)
        };
        Status::check(status, "suunto_d9: failed to read version")
    }

    pub fn reset_maxdepth(device: &Device) -> Result<()> {
        let status = unsafe { ffi::suunto_d9_device_reset_maxdepth(device.raw_ptr()) };
        Status::check(status, "suunto_d9: failed to reset max depth")
    }
}

/// Suunto EON operations.
pub mod eon {
    use super::*;

    pub fn write_name(device: &Device, data: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::suunto_eon_device_write_name(
                device.raw_ptr(),
                data.as_mut_ptr(),
                data.len() as c_uint,
            )
        };
        Status::check(status, "suunto_eon: failed to write name")
    }

    pub fn write_interval(device: &Device, interval: u8) -> Result<()> {
        let status = unsafe { ffi::suunto_eon_device_write_interval(device.raw_ptr(), interval) };
        Status::check(status, "suunto_eon: failed to write interval")
    }
}

/// Suunto Vyper 2 operations.
pub mod vyper2 {
    use super::*;

    pub fn version(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::suunto_vyper2_device_version(
                device.raw_ptr(),
                buf.as_mut_ptr(),
                buf.len() as c_uint,
            )
        };
        Status::check(status, "suunto_vyper2: failed to read version")
    }

    pub fn reset_maxdepth(device: &Device) -> Result<()> {
        let status = unsafe { ffi::suunto_vyper2_device_reset_maxdepth(device.raw_ptr()) };
        Status::check(status, "suunto_vyper2: failed to reset max depth")
    }
}
