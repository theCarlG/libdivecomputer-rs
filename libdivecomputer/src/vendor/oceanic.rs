use std::ffi::c_uint;

use libdivecomputer_sys as ffi;

use crate::device::Device;
use crate::error::Result;
use crate::status::Status;

/// Oceanic Atom2 operations.
pub mod atom2 {
    use super::*;

    pub fn version(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::oceanic_atom2_device_version(
                device.raw_ptr(),
                buf.as_mut_ptr(),
                buf.len() as c_uint,
            )
        };
        Status::check(status, "oceanic_atom2: failed to read version")
    }

    pub fn keepalive(device: &Device) -> Result<()> {
        let status = unsafe { ffi::oceanic_atom2_device_keepalive(device.raw_ptr()) };
        Status::check(status, "oceanic_atom2: failed to send keepalive")
    }
}

/// Oceanic VT Pro operations.
pub mod vtpro {
    use super::*;

    pub fn version(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::oceanic_vtpro_device_version(
                device.raw_ptr(),
                buf.as_mut_ptr(),
                buf.len() as c_uint,
            )
        };
        Status::check(status, "oceanic_vtpro: failed to read version")
    }

    pub fn keepalive(device: &Device) -> Result<()> {
        let status = unsafe { ffi::oceanic_vtpro_device_keepalive(device.raw_ptr()) };
        Status::check(status, "oceanic_vtpro: failed to send keepalive")
    }
}

/// Oceanic Veo 250 operations.
pub mod veo250 {
    use super::*;

    pub fn version(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::oceanic_veo250_device_version(
                device.raw_ptr(),
                buf.as_mut_ptr(),
                buf.len() as c_uint,
            )
        };
        Status::check(status, "oceanic_veo250: failed to read version")
    }

    pub fn keepalive(device: &Device) -> Result<()> {
        let status = unsafe { ffi::oceanic_veo250_device_keepalive(device.raw_ptr()) };
        Status::check(status, "oceanic_veo250: failed to send keepalive")
    }
}
