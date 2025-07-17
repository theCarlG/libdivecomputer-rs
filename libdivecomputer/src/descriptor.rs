use std::ffi::{CStr, c_void};
use std::sync::Arc;
use std::{fmt, ptr};

use libdivecomputer_sys as ffi;
use serde::{Deserialize, Serialize};

use crate::common::Status;
use crate::context::Context;
use crate::device::{Family, Transport};
use crate::error::LibError;

/// A struct representing a DiveComputer.
///
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub struct DiveComputer {
    pub vendor: String,
    pub product: String,
    pub kind: Family,
    pub model: u32,
    pub firmware: u32,
    pub serial: u32,
    pub transports: Vec<Transport>,
}

impl Default for DiveComputer {
    fn default() -> Self {
        Self {
            vendor: String::new(),
            product: String::new(),
            kind: Family::None,
            model: 0,
            firmware: 0,
            serial: 0,
            transports: Vec::new(),
        }
    }
}

impl TryFrom<&DescriptorItem> for DiveComputer {
    type Error = LibError;

    fn try_from(value: &DescriptorItem) -> Result<Self, Self::Error> {
        if value.ptr.is_null() {
            return Err(LibError::NullPointer);
        }

        let dive_computer = Self {
            vendor: value.vendor(),
            product: value.product(),
            model: value.model(),
            kind: value.family(),
            firmware: 0,
            serial: 0,
            transports: value.transports(),
        };

        Ok(dive_computer)
    }
}

#[derive(Clone, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub struct DescriptorItem {
    pub(crate) ptr: *mut ffi::dc_descriptor_t,
    context: Arc<Context>,
}

unsafe impl Send for DescriptorItem {}
unsafe impl Sync for DescriptorItem {}

impl From<&Arc<Context>> for DescriptorItem {
    fn from(context: &Arc<Context>) -> Self {
        Self {
            ptr: ptr::null_mut(),
            context: context.clone(),
        }
    }
}

impl fmt::Debug for DescriptorItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DescriptorItem({}, {}, {}, {:?}, {:?})",
            self.vendor(),
            self.product(),
            self.model(),
            self.family(),
            self.transports(),
        )
    }
}

impl DescriptorItem {
    pub fn vendor(&self) -> String {
        if self.ptr.is_null() {
            return String::new();
        }

        unsafe {
            CStr::from_ptr(ffi::dc_descriptor_get_vendor(self.ptr as *mut _))
                .to_string_lossy()
                .to_string()
        }
    }
    pub fn product(&self) -> String {
        if self.ptr.is_null() {
            return String::new();
        }

        unsafe {
            CStr::from_ptr(ffi::dc_descriptor_get_product(self.ptr as *mut _))
                .to_string_lossy()
                .to_string()
        }
    }

    pub fn model(&self) -> u32 {
        if self.ptr.is_null() {
            return 0;
        }

        unsafe { ffi::dc_descriptor_get_model(self.ptr as *mut _) }
    }

    pub fn family(&self) -> Family {
        if self.ptr.is_null() {
            return Family::None;
        }
        unsafe { Family::from(ffi::dc_descriptor_get_type(self.ptr as *mut _)) }
    }

    pub fn transports(&self) -> Vec<Transport> {
        if self.ptr.is_null() {
            return Vec::new();
        }
        unsafe {
            Transport::vec_from_bitflag(ffi::dc_descriptor_get_transports(self.ptr as *mut _))
        }
    }
}

impl Drop for DescriptorItem {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                ffi::dc_descriptor_free(self.ptr);
            }
        }
    }
}
/// A struct representing a Descriptor.
///
/// # Examples
///
/// ```
/// use libdivecomputer::Descriptor;
/// use libdivecomputer::Context;
///
/// let context = Context::default();
/// let descriptor = Descriptor::from(&context);
///
/// for dive_computer in descriptor {
///     println!("{dive_computer:?}");
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Descriptor {
    pub(crate) iterator: *mut ffi::dc_iterator_t,

    context: Arc<Context>,
}

unsafe impl Send for Descriptor {}
unsafe impl Sync for Descriptor {}

impl From<&Arc<Context>> for Descriptor {
    fn from(context: &Arc<Context>) -> Self {
        let mut iterator: *mut ffi::dc_iterator_t = ptr::null_mut();

        let status = unsafe { ffi::dc_descriptor_iterator_new(&mut iterator, context.ptr) };

        if status != ffi::DC_STATUS_SUCCESS {
            panic!("failed to create iterator: {status}");
        }

        Self {
            iterator,
            context: context.clone(),
        }
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            if !self.iterator.is_null() {
                ffi::dc_iterator_free(self.iterator);
                self.iterator = ptr::null_mut();
            }
        }
    }
}

impl Iterator for Descriptor {
    type Item = DescriptorItem;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let mut item = DescriptorItem::from(&self.context);

            let status =
                ffi::dc_iterator_next(self.iterator, &mut item.ptr as *mut _ as *mut c_void);

            if status != Status::Success as i32 {
                return None;
            }

            Some(item)
        }
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::Context;

    use super::Descriptor;

    #[test]
    fn test_descriptor() {
        let context = Arc::new(Context::default());
        let descriptor = Descriptor::from(&context);
        let computers = descriptor.count();

        assert!(computers > 0);
    }
}
