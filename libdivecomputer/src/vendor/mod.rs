//! Vendor-specific device commands that aren't part of the generic download
//! API. Each submodule wraps a small set of C functions from the corresponding
//! libdivecomputer vendor driver (version strings, keepalives, maker-specific
//! configuration).

/// Atomic Aquatics Cobalt — custom USB handshake helpers.
pub mod atomics_cobalt;
/// DiveSystem — iX3M family vendor commands.
pub mod divesystem;
/// Heinrichs-Weikamp Frog — firmware version query and friends.
pub mod hw_frog;
/// Heinrichs-Weikamp OSTC (first generation).
pub mod hw_ostc;
/// Heinrichs-Weikamp OSTC3 and later.
pub mod hw_ostc3;
/// Oceanic — version query and keepalive.
pub mod oceanic;
/// Reefnet — handshake, sense, and user-data read/write.
pub mod reefnet;
/// Suunto — version query, max-depth reset, name and interval configuration.
pub mod suunto;
