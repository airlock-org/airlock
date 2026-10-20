//! Reusable VM contributions connecting typed Airlock configuration to NixOS modules.

pub mod desktop;

#[path = "machine-source.rs"]
mod machine_source;

#[path = "screen-capture.rs"]
pub mod screen_capture;
