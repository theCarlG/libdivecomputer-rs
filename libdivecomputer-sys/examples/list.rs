use std::ffi::{CStr, c_void};
use std::ptr;

use libdivecomputer_sys::{
    DC_STATUS_SUCCESS, dc_context_free, dc_context_t, dc_descriptor_free,
    dc_descriptor_get_product, dc_descriptor_get_vendor, dc_descriptor_iterator_new,
    dc_descriptor_t, dc_iterator_free, dc_iterator_next, dc_iterator_t,
};

fn main() {
    unsafe {
        let mut iterator: *mut dc_iterator_t = ptr::null_mut();
        let mut descriptor: *mut dc_descriptor_t = ptr::null_mut();
        let context: *mut dc_context_t = ptr::null_mut();

        dc_descriptor_iterator_new(&mut iterator, context);
        while dc_iterator_next(iterator, &mut descriptor as *mut _ as *mut c_void)
            == DC_STATUS_SUCCESS
        {
            let vendor = CStr::from_ptr(dc_descriptor_get_vendor(descriptor));
            let product = CStr::from_ptr(dc_descriptor_get_product(descriptor));

            println!("{} {}", vendor.to_string_lossy(), product.to_string_lossy());

            dc_descriptor_free(descriptor);
        }
        dc_iterator_free(iterator);
        dc_context_free(context);
    }
}
