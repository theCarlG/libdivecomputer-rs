# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust FFI bindings for [libdivecomputer](https://github.com/libdivecomputer/libdivecomputer), a C library for communicating with dive computers. The workspace has two crates:

- **`libdivecomputer-sys`** — Auto-generated unsafe FFI bindings (via `bindgen`)
- **`libdivecomputer`** — High-level safe wrapper (WIP), with async support via tokio

The C library is included as a git submodule at `libdivecomputer-sys/libdivecomputer/` (fork: theCarlG/libdc, branch: libdivecomputer-rs).

## Build Commands

```bash
# Prerequisites: autoreconf, gcc
git submodule update --init          # fetch C library (first time)
cargo build --release                # build everything
cargo build -p libdivecomputer-sys   # build just the sys crate
cargo build -p libdivecomputer       # build just the safe wrapper
cargo test                           # run tests
cargo deny check                     # license/dependency audit
```

### Running Examples

```bash
cargo run --example device_scanner        # scan for dive computers (clap CLI)
cargo run --example device_download       # download dives
cargo run --example dive_parser           # parse saved dive data
cargo run -p libdivecomputer-sys --example list     # list devices (unsafe FFI)
cargo run -p libdivecomputer-sys --example version  # print version
```

## Architecture

### Three-Layer Design

1. **C Library** (submodule) — Actual device communication, built from source by `build.rs`
2. **`libdivecomputer-sys`** — `bindgen`-generated bindings from `wrapper.h`. The `build.rs` compiles the C library for Linux (autotools), Android (ndk-build), macOS/iOS (autotools + xcrun SDK), and Windows (direct `cc` crate), plus bindgen invocation with custom `ParseCallbacks` to rename conflicting symbols. Version numbers are parsed live from the submodule's `configure.ac`.
3. **`libdivecomputer`** — Safe Rust API wrapping the FFI layer

### Key Modules (libdivecomputer/src/)

- **`lib.rs`** — Module declarations and public re-exports
- **`scanner.rs`** — `scan(ctx, transport).execute()` builder for enumerating devices across transports
- **`device.rs`** — `Device`, `DeviceInfo`, `ConnectionInfo`, `DownloadOptions`, `DeviceEvent`
- **`ble/mod.rs`** — BLE transport via `btleplug`; owns the worker thread + event-loop + bounded `mpsc` channel (see "BLE transport")
- **`ble/services.rs`** — Known BLE service UUIDs and per-device quirks (e.g. random-address selection on Android)
- **`bluetooth/mod.rs`** + **`bluetooth/android.rs`** — Classic BT (RFCOMM/SPP) via JNI on Android; delegated to libdivecomputer's native BlueZ/WinSock paths elsewhere
- **`android.rs`** — JNI attach/detach helpers shared by BLE and classic BT on Android
- **`iostream.rs`** — `IoStream` wrapper around `dc_iostream_t`, transport-specific openers
- **`parser.rs`** + **`parser/types.rs`** — Dive data parsing via FFI callbacks; data types (`Dive`, `DiveSample`, `Gasmix`, `Tank`, etc.)
- **`descriptor.rs`** — Device descriptor iteration / lookup by name
- **`context.rs`** — libdivecomputer context with logging and transport detection
- **`error.rs`** — `LibError` unified error enum
- **`common.rs`** — Shared enums (`SampleKind`, `EventKind`, …) and `ffi_guard` for `catch_unwind` at callback boundaries
- **`status.rs`** — `dc_status_t` → `Result` conversions
- **`transport.rs`** — `Transport` enum + `TransportSet` bitflags
- **`family.rs`** — `Family` enum for device families
- **`datetime.rs`**, **`buffer.rs`**, **`version.rs`** — small utility modules

### BLE transport

`BleTransport` runs its own `std::thread` with a current-thread tokio runtime. Sync FFI callbacks (`ble_read`, `ble_write`, …) send request events over a bounded `mpsc::Sender<BleEvent>` (capacity 8) and block on a `oneshot` reply — there is no global runtime. The worker's `JoinHandle` is captured; `Drop for BleTransport` signals disconnect and joins, and the event loop's `tokio::select!` has an explicit channel-close branch so dropping the sender terminates the worker. Panics in the worker are caught by `catch_unwind` and logged via `tracing::error!`.

Entry points: `crate::ble::ble_iostream_open` (sync, used from `IoStream::open`) and `crate::ble::scan_ble` (sync, builds a temporary runtime).

### Cross-Compilation

`.cargo/config.toml` defines targets for Linux, Android (NDK, API 21), iOS, macOS, and Windows (mingw). Linux and Android are the actively-tested targets.

## Conventions

- Rust edition 2024, MSRV 1.87 (workspace-enforced via `rust-version`)
- Error handling: `thiserror` with a unified `LibError` enum and `Result<T>` alias
- Serialization: `serde` + `serde_repr` on all data types
- Date/time: `jiff` crate
- Observability: `tracing` (with `#[instrument]` spans on BLE/device/scanner entry points); examples install `tracing_subscriber::fmt`. Filter via `RUST_LOG=libdivecomputer=debug`.
- `#![warn(missing_docs)]` on the library crate; public items document *why* and list `# Errors` where non-obvious.
- The `libdivecomputer` crate builds as both `cdylib` and `rlib`
- Licensing: Apache-2.0 OR MIT (the C library itself is LGPL-2.1)
- `cargo deny` bans openssl and duplicate dependency versions
