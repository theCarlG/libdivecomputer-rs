pub(crate) mod buffer;
pub(crate) mod common;
pub mod context;
pub(crate) mod datetime;
pub mod descriptor;
pub mod device;
pub mod error;
pub mod family;
pub mod iostream;
pub mod parser;
pub mod scanner;
pub mod status;
pub mod transport;
pub mod vendor;
pub mod version;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(feature = "ble")]
pub mod ble;

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
