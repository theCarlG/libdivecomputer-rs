pub mod common;

#[cfg(feature = "bindings")]
pub mod device;

#[cfg(feature = "bindings")]
pub mod parser;

#[cfg(feature = "bindings")]
mod descriptor;
#[cfg(feature = "bindings")]
pub use descriptor::{Descriptor, DiveComputer};

#[cfg(feature = "bindings")]
mod version;
#[cfg(feature = "bindings")]
pub use version::version;

#[cfg(feature = "bindings")]
mod context;
#[cfg(feature = "bindings")]
pub use context::{Context, LogLevel};

#[cfg(feature = "bindings")]
pub mod error;
