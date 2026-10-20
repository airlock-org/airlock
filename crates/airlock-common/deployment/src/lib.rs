//! Bind deployment effects to approval, durable publication and execution requests.
//! Machine planning determines required effects; this crate independently checks
//! their permission. Callers authenticate records and perform filesystem/Qubes I/O.
//! Cargo executes the same Rust bodies that Verus checks.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod review;

pub mod publication;

pub mod effects;

pub mod inventory;

pub mod operation;

pub mod planning;

pub mod recovery;

pub mod plan;

pub mod configuration;

pub mod copy;

pub mod host;

pub mod retirement;

pub mod pci;

pub mod devices;

pub mod storage;

pub mod purge;
