//! Read-only evidence requests have their own authority, independent of system changes.
//! Installed contracts select sources and callers; each query passes an approval gate.
//! Automatic approval is explicit and is never described as a human decision.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod authority;

mod protocol;

pub use protocol::*;

#[cfg(feature = "runtime")]
pub mod runtime;

#[cfg(feature = "runtime")]
pub mod cli;

#[cfg(test)]
mod tests;
