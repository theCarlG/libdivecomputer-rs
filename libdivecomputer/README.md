<div align="center">

# libdivecomputer

![Build Status](https://github.com/theCarlG/libdivecomputer-rs/workflows/CI/badge.svg)
[![Crates.io](https://img.shields.io/crates/v/libdivecomputer.svg)](https://crates.io/crates/libdivecomputer)
[![Docs](https://docs.rs/libdivecomputer/badge.svg)](https://docs.rs/libdivecomputer)

</div>

Safe, idiomatic Rust bindings for [libdivecomputer](https://github.com/libdivecomputer/libdivecomputer) -- a cross-platform C library for communicating with dive computers from various manufacturers.

See also the [repository](https://github.com/theCarlG/libdivecomputer-rs) containing the low-level [`libdivecomputer-sys`](../libdivecomputer-sys/) FFI bindings.

## Features

- Context with builder pattern and configurable logging
- Device descriptor enumeration and search
- Device scanning across all transports (Serial, USB, USB HID, IrDA, Bluetooth, BLE, USB Storage)
- One-call dive downloading with `download_dives()`, or low-level `foreach` for advanced control
- Dive data parsing (depth, temperature, gas mixes, tank pressure, deco stops, etc.)
- Auto-dispatching `IoStream::open()` -- no manual transport matching
- Device memory read/write/dump and clock sync
- Vendor-specific APIs (Heinrichs Weikamp, Atomics, Suunto, Oceanic, etc.)
- BLE support via btleplug (feature-gated)

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
libdivecomputer = "0.2"
```

To disable BLE support (removes btleplug/tokio dependencies):

```toml
[dependencies]
libdivecomputer = { version = "0.2", default-features = false }
```

## Examples

### List supported dive computers

```rust
use libdivecomputer::{Context, Descriptor, LogLevel};

fn main() -> libdivecomputer::Result<()> {
    let ctx = Context::builder()
        .log_level(LogLevel::Warning)
        .build()?;

    for desc in Descriptor::iter(&ctx)? {
        println!("{desc} (family: {})", desc.family());
    }

    Ok(())
}
```

### Find a specific device descriptor

```rust
use libdivecomputer::{Context, Descriptor, LogLevel};

fn main() -> libdivecomputer::Result<()> {
    let ctx = Context::builder()
        .log_level(LogLevel::Warning)
        .build()?;

    if let Some(desc) = Descriptor::find_by_name(&ctx, "Shearwater Petrel 3")? {
        println!("Found: {desc} (family: {})", desc.family());
        println!("Transports: {}", desc.transports());
    }

    Ok(())
}
```

### Scan for connected devices

```rust
use libdivecomputer::{Context, LogLevel, scan};

fn main() -> libdivecomputer::Result<()> {
    let ctx = Context::builder()
        .log_level(LogLevel::Warning)
        .build()?;

    for transport in &ctx.get_transports() {
        println!("Scanning {transport}...");
        match scan(&ctx, transport).execute() {
            Ok(devices) => {
                for device in &devices {
                    println!("  Found: {} ({})", device.name, device.connection);
                }
            }
            Err(e) => eprintln!("  Error: {e}"),
        }
    }

    Ok(())
}
```

### Download dives from a device

```rust
use libdivecomputer::{
    Context, Descriptor, Device, DeviceEvent, DownloadOptions, IoStream, LogLevel, Transport, scan,
};

fn main() -> libdivecomputer::Result<()> {
    let ctx = Context::builder()
        .log_level(LogLevel::Warning)
        .build()?;

    let desc = Descriptor::find_by_name(&ctx, "Shearwater Petrel 3")?
        .expect("device not found in descriptor database");

    // Scan and connect.
    let devices = scan(&ctx, Transport::Ble).execute()?;
    let device_info = devices.into_iter().next().expect("no device found");

    let iostream = IoStream::open(&ctx, &device_info.connection)?;
    let dev = Device::open(&ctx, &desc, iostream)?;

    // Download and parse all dives.
    let dives = dev.download_dives(&mut DownloadOptions {
        on_event: Some(Box::new(|event| {
            if let DeviceEvent::Progress { current, maximum } = event {
                println!("Progress: {:.0}%", 100.0 * current as f64 / maximum as f64);
            }
        })),
        ..Default::default()
    })?;

    for dive in &dives {
        println!(
            "Dive: {:.1}m, {} min, {}",
            dive.max_depth,
            dive.duration.as_secs() / 60,
            dive.start,
        );
    }

    Ok(())
}
```

### Parse previously saved dive data

```rust
use libdivecomputer::{Context, Descriptor, LogLevel, Parser};

fn main() -> libdivecomputer::Result<()> {
    let ctx = Context::builder()
        .log_level(LogLevel::Warning)
        .build()?;

    let desc = Descriptor::find_by_name(&ctx, "Suunto EON Steel")?
        .expect("device not found");

    let data = std::fs::read("dive.bin").expect("failed to read dive file");
    let fingerprint = &data[12..16]; // device-specific fingerprint location

    let parser = Parser::from_descriptor(&ctx, &desc, &data)?;
    let dive = parser.parse(fingerprint)?;

    println!("Date: {}", dive.start);
    println!("Max depth: {:.1} m", dive.max_depth);
    println!("Duration: {} min", dive.duration.as_secs() / 60);
    println!("Gas mixes: {:?}", dive.gasmixes);
    println!("Samples: {}", dive.samples.len());

    Ok(())
}
```

## Running the included examples

```bash
cargo run --example device_scanner                                      # scan for devices
cargo run --example device_download -- -d "Shearwater Petrel 3" -t BLE  # download dives
cargo run --example dive_parser -- -d "Suunto EON Steel" dives/*.bin    # parse saved data
```

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Note that [libdivecomputer](https://github.com/libdivecomputer/libdivecomputer) has its [own LGPL-2.1 license](https://github.com/libdivecomputer/libdivecomputer/blob/master/COPYING).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
