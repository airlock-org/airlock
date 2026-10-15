//! Foundations for Airlock's Nix subsystem, alongside the existing Python runtime.
//!
//! Parsed records describe data; they do not authenticate its sender. In
//! particular, the checked workflow in [`review`] is not a replacement for dom0's
//! protected approval state or Qubes qrexec policy. There is no execution backend
//! in this crate yet.

#![forbid(unsafe_code)]

pub mod error;
pub mod identity;
pub mod job;
pub mod review;
pub mod state;
mod wire;

pub use error::{Error, Result};
