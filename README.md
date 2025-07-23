<div align="center">

# 🧭 libdivecomputer-rs

**Rust bindings for [libdivecomputer](https://github.com/libdivecomputer/libdivecomputer), a cross-platform and open source library for communication with dive computers from various manufacturers.**

[![Crates.io](https://img.shields.io/crates/v/libdivecomputer.svg)](https://crates.io/crates/libdivecomputer)
[![Docs](https://docs.rs/libdivecomputer/badge.svg)](https://docs.rs/libdivecomputer)
[![dependency status](https://deps.rs/repo/github/theCarlG/libdivecomputer-rs/status.svg)](https://deps.rs/repo/github/theCarlG/libdivecomputer-rs)
[![Build status](https://github.com/theCarlG/libdivecomputer-rs/workflows/CI/badge.svg)](https://github.com/theCarlG/libdivecomputer-rs/actions)

</div>

This repository contains 2 crates:

| Name | Description | Links |
| --- | --- | --- |
| [`libdivecomputer`](libdivecomputer/) | High-level interface on top of `libdivecomputer-sys` 🚧 | [![Crates.io](https://img.shields.io/crates/v/libdivecomputer.svg)](https://crates.io/crates/libdivecomputer) [![Docs](https://docs.rs/libdivecomputer/badge.svg)](https://docs.rs/libdivecomputer) |
| [`libdivecomputer-sys`](libdivecomputer-sys/) | Unsafe bindings for [libdivecomputer](https://github.com/libdivecomputer/libdivecomputer) | [![Crates.io](https://img.shields.io/crates/v/libdivecomputer-sys.svg)](https://crates.io/crates/libdivecomputer-sys) [![Docs](https://docs.rs/libdivecomputer-sys/badge.svg)](https://docs.rs/libdivecomputer-sys) |

## Caveats

* The high-level `libdivecomputer` wrapper is work-in-progress, and only covers a part of libdivecomputer functionality.

* Any other features have to be accessed through the unsafe [libdivecomputer-sys](libdivecomputer-sys/) crate.

* Only supports Linux and Android at the moment.

## Usage

The following code example shows how [`libdivecomputer`](libdivecomputer/) can be initialized.

``` rust
let dive_computer = DiveComputer::new();
for vendor in dive_computer.vendors().unwrap() {
    println!("{}", vendor.name);
    for product in vendor.products() {
        println!("\t{}", product.name)
    }
}
```

Information about all wrapper functionality can be found in the [libdivecomputer](libdivecomputer/) crate docs.

## Prerequisites

* autoreconf
* gcc

## How to build

```bash
git submodule update --init
cargo build --release
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
