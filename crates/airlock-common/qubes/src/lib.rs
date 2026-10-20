//! Call upstream Qubes objects from Rust without replacing their descriptors,
//! storage abstractions, event dispatch or database locking.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod descriptor;

pub mod gpu;

pub mod snapshot;

pub mod storage;

pub mod values;
