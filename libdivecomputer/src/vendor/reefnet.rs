use std::ffi::c_uint;

use libdivecomputer_sys as ffi;

use crate::device::Device;
use crate::error::Result;
use crate::status::Status;

/// Reefnet Sensus operations.
pub mod sensus {
    use super::*;

    pub fn get_handshake(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::reefnet_sensus_device_get_handshake(
                device.raw_ptr(),
                buf.as_mut_ptr(),
                buf.len() as c_uint,
            )
        };
        Status::check(status, "reefnet_sensus: failed to get handshake")
    }
}

/// Reefnet Sensus Pro operations.
pub mod sensuspro {
    use super::*;

    pub fn get_handshake(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::reefnet_sensuspro_device_get_handshake(
                device.raw_ptr(),
                buf.as_mut_ptr(),
                buf.len() as c_uint,
            )
        };
        Status::check(status, "reefnet_sensuspro: failed to get handshake")
    }

    pub fn write_interval(device: &Device, interval: u8) -> Result<()> {
        let status =
            unsafe { ffi::reefnet_sensuspro_device_write_interval(device.raw_ptr(), interval) };
        Status::check(status, "reefnet_sensuspro: failed to write interval")
    }
}

/// Reefnet Sensus Ultra operations.
pub mod sensusultra {
    use super::*;

    pub fn get_handshake(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::reefnet_sensusultra_device_get_handshake(
                device.raw_ptr(),
                buf.as_mut_ptr(),
                buf.len() as c_uint,
            )
        };
        Status::check(status, "reefnet_sensusultra: failed to get handshake")
    }

    pub fn read_user(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::reefnet_sensusultra_device_read_user(
                device.raw_ptr(),
                buf.as_mut_ptr(),
                buf.len() as c_uint,
            )
        };
        Status::check(status, "reefnet_sensusultra: failed to read user data")
    }

    pub fn write_user(device: &Device, data: &[u8]) -> Result<()> {
        let status = unsafe {
            ffi::reefnet_sensusultra_device_write_user(
                device.raw_ptr(),
                data.as_ptr(),
                data.len() as c_uint,
            )
        };
        Status::check(status, "reefnet_sensusultra: failed to write user data")
    }

    pub fn sense(device: &Device, buf: &mut [u8]) -> Result<()> {
        let status = unsafe {
            ffi::reefnet_sensusultra_device_sense(
                device.raw_ptr(),
                buf.as_mut_ptr(),
                buf.len() as c_uint,
            )
        };
        Status::check(status, "reefnet_sensusultra: failed to sense")
    }
}
