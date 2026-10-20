//! Reusable Rust-authored OpenSSL family.
mod inputs;

mod lowering;

pub mod model;

mod scripts;

pub use lowering::{factory, family_factory};
