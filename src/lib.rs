//! Pulsline: journal your coding-agent work and see your day as a timeline.
//!
//! The crate is three layers, one directory each (see AGENTS.md,
//! "Architecture"): [`core`] is what Pulsline is and does no I/O,
//! [`interface`] is the ways in, and [`infra`] is the systems it calls on.
//! The binary is `src/main.rs`; the desktop client (`desktop/`) uses this
//! library for the core and the infra.

pub mod config;
pub mod core;
pub mod infra;
pub mod interface;
pub mod logging;

/// This build's version, as Cargo knows it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
