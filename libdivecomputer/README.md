<div align="center">

# 🧭 libdivecomputer

![Build Status](https://github.com/theCarlG/libdivecomputer-rs/workflows/CI/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/libdivecomputer.svg)](https://crates.io/crates/libdivecomputer)
[![Docs](https://docs.rs/libdivecomputer/badge.svg)](https://docs.rs/libdivecomputer)

</div>

**This is a work in progress** 🚧

`libdivecomputer` is intended to be an easy to use high-level wrapper for [libdivecomputer](https://github.com/libdivecomputer/libdivecomputer).

Please also see the [repository](https://github.com/theCarlG/libdivecomputer-rs) containing an unsafe low-level binding.

## Basic usage

``` rust
use libdivecomputer::Descriptor;

let descriptor = Descriptor::default();

for dive_computer in descriptor {
    println!("{dive_computer:?}");
}
```

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Note that the [PhysX C++ SDK](https://github.com/NVIDIAGameWorks/PhysX) has it's [own BSD 3 license](https://gameworksdocs.nvidia.com/PhysX/4.1/documentation/physxguide/Manual/License.html) and depends on [additional C++ third party libraries](https://github.com/NVIDIAGameWorks/PhysX/tree/4.1/externals).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.