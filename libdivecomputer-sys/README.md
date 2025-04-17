<div align="center">

# 🧭 libdivecomputer-sys

**Unsafe automatically-generated Rust bindings for [libdivecomputer](https://github.com/libdivecomputer/libdivecomputer).**

![Build Status](https://github.com/theCarlG/libdivecomputer-rs/workflows/CI/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/libdivecomputer-sys.svg)](https://crates.io/crates/libdivecomputer-sys)
[![Docs](https://docs.rs/libdivecomputer-sys/badge.svg)](https://docs.rs/libdivecomputer-sys)

</div>

Please also see the [repository](https://github.com/theCarlg/libdivecomputer-rs) containing a work-in-progress safe wrapper.

## Basic usage

```rust
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
```

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Note that [libdivecomputer](https://github.com/libdivecomputer/libdivecomputer) has its [own LGPL-2.1 license](https://github.com/libdivecomputer/libdivecomputer/blob/master/COPYING).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
