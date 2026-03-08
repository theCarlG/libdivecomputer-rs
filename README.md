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

| Platform | Status | Transports |
| --- | --- | --- |
| Linux | Fully supported | Serial, USB, USBHID, IrDA, Bluetooth, BLE |
| Android | Supported (requires NDK) | Serial, BLE |
| macOS | Supported | Serial, USB, USBHID, BLE |
| iOS | Supported | Serial, BLE |
| Windows | Supported (MSVC) | Serial, USB, USBHID, IrDA, BLE |

## Prerequisites

- `autoreconf` (autotools) — Linux, macOS, iOS
- `gcc` or compatible C compiler
- `git` (submodules)

### Linux

```bash
sudo apt install autoconf automake libtool pkg-config \
  libusb-1.0-0-dev libhidapi-dev libbluetooth-dev libdbus-1-dev libmtp-dev
```

### macOS

```bash
brew install automake autoconf libtool pkg-config libusb hidapi
```

### Windows (MSVC)

1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with "Desktop development with C++" and Windows SDK
2. Install [LLVM](https://releases.llvm.org/) (for `libclang` used by `bindgen`)
3. Download [libusb](https://github.com/libusb/libusb/releases) and [hidapi](https://github.com/libusb/hidapi/releases) and extract them
4. Set environment variables pointing to the extracted directories:

```powershell
$env:LIBUSB_DIR = "C:\path\to\libusb"
$env:HIDAPI_DIR = "C:\path\to\hidapi"
```

Build from a **Developer Command Prompt** (or run `vcvarsall.bat x64` first):

```powershell
git submodule update --init --recursive
cargo build --release
```

## Building

```bash
git submodule update --init --recursive
cargo build --release
```

## Cross-Compilation

### macOS: Intel (x86_64) to Apple Silicon (arm64)

When cross-compiling on an Intel Mac for `aarch64-apple-darwin`, the Homebrew-installed
libraries (libusb, hidapi) are x86_64 and cannot be used. You need to build arm64
versions from source:

```bash
# Install the Rust target
rustup target add aarch64-apple-darwin

# Build libusb for arm64
cd /tmp && git clone https://github.com/libusb/libusb && cd libusb
./autogen.sh
./configure --host=aarch64-apple-darwin --prefix=/tmp/libusb-arm64 \
  CFLAGS="-arch arm64" LDFLAGS="-arch arm64" --disable-shared --enable-static
make && make install

# Build hidapi for arm64
cd /tmp && git clone https://github.com/libusb/hidapi && cd hidapi
cmake -B build -DCMAKE_OSX_ARCHITECTURES=arm64 \
  -DCMAKE_INSTALL_PREFIX=/tmp/hidapi-arm64 -DBUILD_SHARED_LIBS=OFF
cmake --build build && cmake --install build

# Build with arm64 libraries
LIBUSB_DIR=/tmp/libusb-arm64 HIDAPI_DIR=/tmp/hidapi-arm64 \
  cargo build --release --target aarch64-apple-darwin
```

Without `LIBUSB_DIR`/`HIDAPI_DIR`, the build will succeed but USB and USBHID
transports will be disabled for the cross-compiled target.

### macOS: Apple Silicon to Intel (x86_64)

```bash
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin
```

Homebrew libraries from `/opt/homebrew/lib` (arm64) will be skipped automatically.
Set `LIBUSB_DIR`/`HIDAPI_DIR` pointing to x86_64 builds for USB/USBHID support.

### iOS

Requires full Xcode (not just Command Line Tools) for the iOS SDK:

```bash
# Install Xcode from the App Store, then:
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer

# Verify the iOS SDK is available
xcrun --show-sdk-path --sdk iphoneos

rustup target add aarch64-apple-ios
cargo build --release --target aarch64-apple-ios
```

Note: USB/USBHID are not available on iOS. Serial and BLE are supported.

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
