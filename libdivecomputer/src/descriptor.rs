use std::ffi::{CStr, c_void};
use std::ptr;

use libdivecomputer_sys::{
    dc_descriptor_free, dc_descriptor_get_model, dc_descriptor_get_product,
    dc_descriptor_get_transports, dc_descriptor_get_type, dc_descriptor_get_vendor,
    dc_descriptor_iterator, dc_descriptor_t, dc_iterator_free, dc_iterator_next, dc_iterator_t,
};

use crate::common::{Family, Status, Transport};

/// A struct representing a DiveComputer.
///
#[derive(Debug)]
pub struct DiveComputer {
    vendor: String,
    product: String,
    model: u32,
    kind: Family,
    transports: Vec<Transport>,
}

impl DiveComputer {
    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    pub fn product(&self) -> &str {
        &self.product
    }

    pub fn model(&self) -> u32 {
        self.model
    }

    pub fn kind(&self) -> Family {
        self.kind
    }

    pub fn transport(&self) -> &[Transport] {
        &self.transports
    }
}

impl TryFrom<Item> for DiveComputer {
    type Error = String;

    fn try_from(value: Item) -> Result<Self, Self::Error> {
        if value.ptr.is_null() {
            return Err("null pointer".into());
        }

        unsafe {
            let vendor = CStr::from_ptr(dc_descriptor_get_vendor(value.ptr as *mut _))
                .to_string_lossy()
                .to_string();
            let product = CStr::from_ptr(dc_descriptor_get_product(value.ptr as *mut _))
                .to_string_lossy()
                .to_string();
            let model = dc_descriptor_get_model(value.ptr as *mut _);
            let kind = Family::from(dc_descriptor_get_type(value.ptr as *mut _));
            let transports =
                Transport::vec_from_bitflag(dc_descriptor_get_transports(value.ptr as *mut _));

            let dive_computer = Self {
                vendor,
                product,
                model,
                kind,
                transports,
            };

            Ok(dive_computer)
        }
    }
}

struct Item {
    ptr: *mut dc_descriptor_t,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            ptr: ptr::null_mut(),
        }
    }
}

impl Drop for Item {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                dc_descriptor_free(self.ptr);
                self.ptr = ptr::null_mut();
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
///
/// let descriptor = Descriptor::default();
///
/// for dive_computer in descriptor {
///     println!("{dive_computer:?}");
/// }
/// ```
pub struct Descriptor {
    ptr: *mut dc_iterator_t,
}

impl Default for Descriptor {
    fn default() -> Self {
        unsafe {
            let mut ptr: *mut dc_iterator_t = ptr::null_mut();
            dc_descriptor_iterator(&mut ptr);

            Self { ptr }
        }
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                dc_iterator_free(self.ptr);
                self.ptr = ptr::null_mut();
            }
        }
    }
}

impl Iterator for Descriptor {
    type Item = DiveComputer;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let mut item = Item::default();
            let status = dc_iterator_next(self.ptr, &mut item.ptr as *mut _ as *mut c_void);

            if status != Status::Success as i32 {
                return None;
            }

            DiveComputer::try_from(item).ok()
        }
    }
}
