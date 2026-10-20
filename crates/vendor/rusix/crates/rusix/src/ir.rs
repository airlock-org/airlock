//! Semantic representation of deferred Nix computations and their Rust origins.
//!
//! Configuration authors normally use [`crate::prelude`]. These types support
//! compiler implementations and explicit inspection without evaluating Nix.

mod node;

mod origin;

mod reference;

pub(crate) mod validation;

pub use node::{Assignment, IntoNode, Node, ReferencedExpression, ValueKind};
pub use origin::Origin;
pub use reference::{Reference, Source};
