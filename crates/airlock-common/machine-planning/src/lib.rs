//! Interpret machine relationships and calculate required VM lifecycle effects.
//! These decisions grant no authority. Cargo executes the same Rust bodies that
//! Verus checks; configuration decoding and external observations remain adapters.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod names;

pub mod graph;

pub mod topology;

pub mod deployment;

pub mod effects;
