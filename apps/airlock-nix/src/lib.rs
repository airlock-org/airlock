//! Foundations for Airlock's Nix subsystem, alongside the existing Python runtime.
//!
//! Parsed records describe data; they do not authenticate its sender. In
//! particular, the checked workflow in [`review`] is not a replacement for dom0's
//! protected approval state or Qubes qrexec policy. Role-specific runtime
//! executables use those existing services and keep observation read-only.

#![forbid(unsafe_code)]

pub mod error;
pub mod identity;
pub mod job;
pub mod review;
#[cfg(any(feature = "client", feature = "service"))]
pub mod runtime;
pub mod state;
mod wire;

pub use error::{Error, Result};
