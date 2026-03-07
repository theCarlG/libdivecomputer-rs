use std::ffi::{CStr, c_void};
use std::{fmt, ptr};

use libdivecomputer_sys as ffi;

use crate::error::{LibError, Result};
use crate::family::Family;
use crate::status::Status;
use crate::transport::{Transport, TransportSet};

/// Metadata for a specific dive computer model. Wraps `dc_descriptor_t`.
pub struct Descriptor {
    pub(crate) ptr: *mut ffi::dc_descriptor_t,
}

// SAFETY: dc_descriptor_t is read-only metadata about device models.
// All accessor methods only read from the C struct.
unsafe impl Send for Descriptor {}
unsafe impl Sync for Descriptor {}

impl Descriptor {
    /// Iterate over all known dive computer descriptors.
    pub fn iter() -> Result<DescriptorIter> {
        let mut iterator: *mut ffi::dc_iterator_t = ptr::null_mut();
        let status = unsafe { ffi::dc_descriptor_iterator_new(&mut iterator, ptr::null_mut()) };
        Status::check(status, "failed to create descriptor iterator")?;
        Ok(DescriptorIter { iterator })
    }

    /// Find a descriptor by vendor and product name.
    pub fn find(vendor: &str, product: &str) -> Result<Option<Descriptor>> {
        for desc in Self::iter()? {
            if desc.vendor() == vendor && desc.product() == product {
                return Ok(Some(desc));
            }
        }
        Ok(None)
    }

    /// Find a descriptor by full name ("Vendor Product").
    pub fn find_by_name(name: &str) -> Result<Descriptor> {
        for desc in Self::iter()? {
            let full_name = format!("{} {}", desc.vendor(), desc.product());
            if full_name == name || desc.product() == name {
                return Ok(desc);
            }
        }
        Err(LibError::DescriptorNotFound(name.to_string()))
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

// SAFETY: dc_iterator_t for descriptors only reads from a static internal table.
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
    use super::*;

    #[test]
    fn test_descriptor_iter() {
        let count = Descriptor::iter().unwrap().count();
        assert!(count > 0);
    }

    #[test]
    fn find_known_vendor_product() {
        // Suunto EON Steel is a well-known device that should always be in the descriptor table
        let result = Descriptor::find("Suunto", "EON Steel").unwrap();
        assert!(result.is_some());
        let desc = result.unwrap();
        assert_eq!(desc.vendor(), "Suunto");
        assert_eq!(desc.product(), "EON Steel");
    }

    #[test]
    fn find_unknown_returns_none() {
        let result = Descriptor::find("NonExistent", "Device").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn find_by_name_known() {
        let desc = Descriptor::find_by_name("Suunto EON Steel").unwrap();
        assert_eq!(desc.vendor(), "Suunto");
        assert_eq!(desc.product(), "EON Steel");
    }

    #[test]
    fn find_by_name_unknown() {
        let err = Descriptor::find_by_name("Nonexistent Device 9999").unwrap_err();
        assert!(matches!(err, LibError::DescriptorNotFound(_)));
    }

    #[test]
    fn descriptor_accessors() {
        let desc = Descriptor::iter().unwrap().next().unwrap();
        assert!(!desc.vendor().is_empty());
        assert!(!desc.product().is_empty());
        // family and model are valid (no panic)
        let _ = desc.family();
        let _ = desc.model();
    }

    #[test]
    fn descriptor_transports_non_empty() {
        // At least some descriptors should have transports
        let has_transports = Descriptor::iter()
            .unwrap()
            .any(|d| !d.transports().to_vec().is_empty());
        assert!(has_transports);
    }

    #[test]
    fn descriptor_display() {
        let desc = Descriptor::find("Suunto", "EON Steel").unwrap().unwrap();
        assert_eq!(desc.to_string(), "Suunto EON Steel");
    }
}
