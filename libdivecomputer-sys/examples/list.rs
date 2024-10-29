use std::ffi::{c_void, CStr};
use std::ptr;

use libdivecomputer_sys::{
    dc_descriptor_free, dc_descriptor_get_product, dc_descriptor_get_vendor,
    dc_descriptor_iterator, dc_descriptor_t, dc_iterator_free, dc_iterator_next, dc_iterator_t,
    dc_status_t_DC_STATUS_SUCCESS,
};

fn main() {
    unsafe {
        let mut iterator: *mut dc_iterator_t = ptr::null_mut();
        let mut descriptor: *mut dc_descriptor_t = ptr::null_mut();

        dc_descriptor_iterator(&mut iterator);
        while dc_iterator_next(iterator, &mut descriptor as *mut _ as *mut c_void)
            == dc_status_t_DC_STATUS_SUCCESS
        {
            let vendor = CStr::from_ptr(dc_descriptor_get_vendor(descriptor));
            let product = CStr::from_ptr(dc_descriptor_get_product(descriptor));

            println!("{} {}", vendor.to_string_lossy(), product.to_string_lossy());

            dc_descriptor_free(descriptor);
        }
        dc_iterator_free(iterator);
    }
}
