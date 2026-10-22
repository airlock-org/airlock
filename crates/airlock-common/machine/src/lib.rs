//! Shared machine records and pure planning used by authoring and dom0.
//! These operations describe intended effects; they perform no observation or activation.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod model;

pub mod boot;

pub mod planning;

pub mod relationships;

pub mod inspection;

pub mod services;

pub mod host_store;

/// Read-only input transport evidence shared by the host and GUI guest.
pub mod input;
