use std::ffi::CString;

use libdivecomputer_sys as ffi;

use crate::device::Device;
use crate::error::Result;
use crate::status::Status;

/// Update firmware on a DiveSystem iDive device.
pub fn firmware_update(device: &Device, filename: &str) -> Result<()> {
    let c_filename = CString::new(filename)?;
    let status =
        unsafe { ffi::divesystem_idive_device_fwupdate(device.raw_ptr(), c_filename.as_ptr()) };
    Status::check(status, "divesystem_idive: failed to update firmware")
}
