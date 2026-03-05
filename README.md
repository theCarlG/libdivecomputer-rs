<div align="center">

# libdivecomputer-rs

**Rust bindings for [libdivecomputer](https://github.com/libdivecomputer/libdivecomputer), a cross-platform and open source library for communication with dive computers from various manufacturers.**

[![Crates.io](https://img.shields.io/crates/v/libdivecomputer.svg)](https://crates.io/crates/libdivecomputer)
[![Docs](https://docs.rs/libdivecomputer/badge.svg)](https://docs.rs/libdivecomputer)
[![dependency status](https://deps.rs/repo/github/theCarlG/libdivecomputer-rs/status.svg)](https://deps.rs/repo/github/theCarlG/libdivecomputer-rs)
[![Build status](https://github.com/theCarlG/libdivecomputer-rs/workflows/CI/badge.svg)](https://github.com/theCarlG/libdivecomputer-rs/actions)

</div>

This repository contains 2 crates:

| Name | Description | Links |
| --- | --- | --- |
| [`libdivecomputer`](libdivecomputer/) | Safe, idiomatic high-level Rust bindings | [![Crates.io](https://img.shields.io/crates/v/libdivecomputer.svg)](https://crates.io/crates/libdivecomputer) [![Docs](https://docs.rs/libdivecomputer/badge.svg)](https://docs.rs/libdivecomputer) |
| [`libdivecomputer-sys`](libdivecomputer-sys/) | Unsafe auto-generated FFI bindings | [![Crates.io](https://img.shields.io/crates/v/libdivecomputer-sys.svg)](https://crates.io/crates/libdivecomputer-sys) [![Docs](https://docs.rs/libdivecomputer-sys/badge.svg)](https://docs.rs/libdivecomputer-sys) |

## Quick Start

```rust
use libdivecomputer::{Context, Descriptor, LogLevel};

fn main() -> libdivecomputer::Result<()> {
    let ctx = Context::builder()
        .log_level(LogLevel::Warning)
        .build()?;

    // List all supported dive computers.
    for desc in Descriptor::iter(&ctx)? {
        println!("{desc} (family: {})", desc.family());
    }

    Ok(())
}
```

See the [libdivecomputer crate README](libdivecomputer/) for more examples, including scanning for devices, downloading dives, and parsing dive data.

## Supported Transports

Serial, USB, USB HID, IrDA, Bluetooth, BLE, and USB Storage.

BLE support requires the `ble` feature (enabled by default), which uses [btleplug](https://crates.io/crates/btleplug).

## Platform Support

- Linux (fully supported)
- Android (supported, requires NDK)
- macOS, iOS, Windows (cross-compilation targets defined, not fully tested)

## Prerequisites

- `autoreconf` (autotools)
- `gcc` or compatible C compiler

## Building

```bash
git submodule update --init
cargo build --release
```

## Examples

```bash
cargo run --example device_scanner                        # scan for dive computers
cargo run --example device_download -- -d "Shearwater Petrel 3" -t BLE  # download dives
cargo run --example dive_parser -- -d "Suunto EON Steel" dives/*.bin    # parse saved dives

# Low-level sys crate examples
cargo run -p libdivecomputer-sys --example list           # list supported devices
cargo run -p libdivecomputer-sys --example version        # print library version
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
