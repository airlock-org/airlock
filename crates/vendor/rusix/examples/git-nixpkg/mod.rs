//! Reusable Rust-authored Git package and its normal package-set wiring.
mod inputs;

mod lowering;

pub mod model;

pub use inputs::Arguments;

pub use lowering::factory;

pub fn arguments() -> Arguments {
    inputs::arguments(model::Git::defaults())
}
