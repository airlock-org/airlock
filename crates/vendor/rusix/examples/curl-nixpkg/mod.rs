//! Reusable Rust-authored curl package.
mod inputs;

mod lowering;

pub mod model;

pub use lowering::factory;
