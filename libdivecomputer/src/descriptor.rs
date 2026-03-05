use std::ffi::{CStr, c_void};
use std::{fmt, ptr};

use libdivecomputer_sys as ffi;

use crate::context::Context;
use crate::error::Result;
use crate::family::Family;
use crate::status::Status;
use crate::transport::{Transport, TransportSet};

/// Metadata for a specific dive computer model. Wraps `dc_descriptor_t`.
pub struct Descriptor {
    pub(crate) ptr: *mut ffi::dc_descriptor_t,
}

unsafe impl Send for Descriptor {}
unsafe impl Sync for Descriptor {}

impl Descriptor {
    /// Iterate over all known dive computer descriptors.
    pub fn iter(_ctx: &Context) -> Result<DescriptorIter> {
        let mut iterator: *mut ffi::dc_iterator_t = ptr::null_mut();
        let status = unsafe { ffi::dc_descriptor_iterator_new(&mut iterator, ptr::null_mut()) };
        Status::check(status, "failed to create descriptor iterator")?;
        Ok(DescriptorIter { iterator })
    }

    /// Find a descriptor by vendor and product name.
    pub fn find(ctx: &Context, vendor: &str, product: &str) -> Result<Option<Descriptor>> {
        for desc in Self::iter(ctx)? {
            if desc.vendor() == vendor && desc.product() == product {
                return Ok(Some(desc));
            }
        }
        Ok(None)
    }

    /// Find a descriptor by full name ("Vendor Product").
    pub fn find_by_name(ctx: &Context, name: &str) -> Result<Option<Descriptor>> {
        for desc in Self::iter(ctx)? {
            let full_name = format!("{} {}", desc.vendor(), desc.product());
            if full_name == name || desc.product() == name {
                return Ok(Some(desc));
            }
        }
        Ok(None)
    }

    /// Vendor name.
    pub fn vendor(&self) -> &str {
        if self.ptr.is_null() {
            return "";
        }
        unsafe {
            CStr::from_ptr(ffi::dc_descriptor_get_vendor(self.ptr))
                .to_str()
                .unwrap_or("")
        }
    }

    /// Product name.
    pub fn product(&self) -> &str {
        if self.ptr.is_null() {
            return "";
        }
        unsafe {
            CStr::from_ptr(ffi::dc_descriptor_get_product(self.ptr))
                .to_str()
                .unwrap_or("")
        }
    }

    /// Model number.
    pub fn model(&self) -> u32 {
        if self.ptr.is_null() {
            return 0;
        }
        unsafe { ffi::dc_descriptor_get_model(self.ptr) }
    }

    /// Device family.
    pub fn family(&self) -> Family {
        if self.ptr.is_null() {
            return Family::None;
        }
        unsafe { Family::from(ffi::dc_descriptor_get_type(self.ptr)) }
    }

    /// Supported transports as a set.
    pub fn transports(&self) -> TransportSet {
        if self.ptr.is_null() {
            return TransportSet::from_bits(0);
        }
        unsafe { TransportSet::from_bits(ffi::dc_descriptor_get_transports(self.ptr)) }
    }

    /// Supported transports as a Vec.
    pub fn transport_list(&self) -> Vec<Transport> {
        self.transports().to_vec()
    }
}

impl fmt::Display for Descriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.vendor(), self.product())
    }
}

impl fmt::Debug for Descriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Descriptor({}, {}, {}, {:?}, {:?})",
            self.vendor(),
            self.product(),
            self.model(),
            self.family(),
            self.transport_list(),
        )
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                ffi::dc_descriptor_free(self.ptr);
            }
        }
    }
}

/// Iterator over all known dive computer descriptors.
pub struct DescriptorIter {
    iterator: *mut ffi::dc_iterator_t,
}

unsafe impl Send for DescriptorIter {}
unsafe impl Sync for DescriptorIter {}

impl Iterator for DescriptorIter {
    type Item = Descriptor;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let mut ptr: *mut ffi::dc_descriptor_t = ptr::null_mut();
            let status = ffi::dc_iterator_next(self.iterator, &mut ptr as *mut _ as *mut c_void);

            if status != Status::Success as i32 {
                return None;
            }

            Some(Descriptor { ptr })
        }
    }
}

impl Drop for DescriptorIter {
    fn drop(&mut self) {
        unsafe {
            if !self.iterator.is_null() {
                ffi::dc_iterator_free(self.iterator);
                self.iterator = ptr::null_mut();
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::Context;

    use super::Descriptor;

    #[test]
    fn test_descriptor_iter() {
        let ctx = Context::new().unwrap();
        let count = Descriptor::iter(&ctx).unwrap().count();
        assert!(count > 0);
    }
}
