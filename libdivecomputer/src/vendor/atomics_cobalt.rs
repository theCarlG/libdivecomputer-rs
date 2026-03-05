use std::ffi::c_uint;

use libdivecomputer_sys as ffi;

use crate::device::Device;
use crate::error::Result;
use crate::status::Status;

/// Read the device version.
pub fn version(device: &Device, buf: &mut [u8]) -> Result<()> {
    let status = unsafe {
        ffi::atomics_cobalt_device_version(device.raw_ptr(), buf.as_mut_ptr(), buf.len() as c_uint)
    };
    Status::check(status, "atomics_cobalt: failed to read version")
}

/// Set simulation mode.
pub fn set_simulation(device: &Device, simulation: u32) -> Result<()> {
    let status = unsafe { ffi::atomics_cobalt_device_set_simulation(device.raw_ptr(), simulation) };
    Status::check(status, "atomics_cobalt: failed to set simulation mode")
}
