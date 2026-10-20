//! Reusable Rust-authored MariaDB client/server family.
mod inputs;

mod lowering;

pub mod model;

mod scripts;

pub use lowering::{factory, family};
