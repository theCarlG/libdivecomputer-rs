//! Safe Rust bindings for [libdivecomputer], a C library for talking to dive
//! computers over serial, USB, IrDA, Bluetooth (classic), and BLE transports.
//!
//! # Entry points
//!
//! Most callers start with one of:
//!
//! - [`scan`] — enumerate devices reachable over a given [`Transport`]. Returns
//!   a [`Vec<DeviceInfo>`](DeviceInfo) carrying the connection info needed to
//!   open the device.
//! - [`Descriptor::find_by_name`] / [`Descriptor::find`] — look up a specific
//!   device model from libdivecomputer's built-in catalog.
//! - [`IoStream::open`] + [`Device::open`] — open the physical I/O stream
//!   (serial port, BLE session, etc.) and then bind it to a device model for
//!   download.
//! - [`Device::download_dives`] — pull dive logs off the computer; produces
//!   [`Vec<Dive>`](Dive) plus any parse errors in a [`DownloadResult`].
//!
//! # Feature flags
//!
//! - `ble` (default on) — enable BLE transport via `btleplug`.
//! - `bluetooth` — classic Bluetooth (Android only; desktop platforms use the
//!   C library's built-in classic BT support).
//!
//! # Errors
//!
//! All fallible operations return [`Result<T>`] with [`LibError`] as the error
//! type. FFI failures are mapped to [`LibError::Status`] carrying a
//! [`Status`] code plus optional context; see [`error`] for the full variant
//! list.
//!
//! [libdivecomputer]: https://github.com/libdivecomputer/libdivecomputer
#![warn(missing_docs)]

pub(crate) mod buffer;
pub(crate) mod common;
/// libdivecomputer [`Context`] + logging configuration.
pub mod context;
pub(crate) mod datetime;
/// Descriptor catalog: look up device models by name, family, or model code.
pub mod descriptor;
/// Device connections, scan result types, download events, and the
/// [`Device::download_dives`] entry point.
pub mod device;
/// Crate-wide error type [`LibError`] and the [`Result`] alias.
pub mod error;
/// Device [`Family`] enum — high-level grouping of vendor-specific protocols.
pub mod family;
/// [`IoStream`] — the transport-level I/O handle that sits between a connection
/// and a [`Device`].
pub mod iostream;
/// Dive log [`Parser`] + the concrete dive data types (`Dive`, `DiveSample`,
/// `Fingerprint`, …).
pub mod parser;
/// Device discovery — [`scan`] enumerates all devices reachable over a given
/// [`Transport`].
pub mod scanner;
/// libdivecomputer [`Status`] enum and FFI-return-code checking helpers.
pub mod status;
/// [`Transport`] enum and the [`TransportSet`] bitmask decoder.
pub mod transport;
/// Vendor-specific hooks for Oceanic, Reefnet, Suunto, and friends.
pub mod vendor;
/// Version string of the underlying C library.
pub mod version;

/// Android JNI glue — guards, attach helpers, classic Bluetooth socket wrapper.
#[cfg(target_os = "android")]
pub mod android;

/// BLE transport — peripheral scan, GATT session, iostream bridge.
#[cfg(feature = "ble")]
pub mod ble;

/// Classic Bluetooth (RFCOMM) on Android. A no-op shim on other platforms.
#[cfg(feature = "bluetooth")]
pub mod bluetooth;

// Re-exports for convenience.
pub use common::{EventKind, SampleFlag, SampleKind};
pub use context::{Context, ContextBuilder, LogLevel};
pub use descriptor::{Descriptor, DescriptorIter};
pub use device::{
    ConnectionInfo, Device, DeviceEvent, DeviceInfo, DownloadOptions, DownloadResult,
};
pub use error::{LibError, Result};
pub use family::Family;
pub use iostream::IoStream;
pub use parser::{
    Deco, DecoKind, DecoModel, Dive, DiveEvent, DiveMode, DiveSample, Fingerprint, GasUsage,
    Gasmix, Location, O2Sensor, Parser, Ppo2, STRING_KEY_FIRMWARE_VERSION,
    STRING_KEY_SERIAL_NUMBER, Salinity, SalinityKind, Sensor, Tank, TankKind, TankUsage,
};
pub use scanner::scan;
pub use status::Status;
pub use transport::{Transport, TransportSet};
pub use version::version;
