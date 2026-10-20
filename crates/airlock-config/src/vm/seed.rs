//! Expose machine-owned initial Rust source as data for native configuration-role initialization.

use rusix::{
    interop::{NixPath, raw::NixValue},
    nix_record,
};

/// Snapshot the initial candidate without embedding filesystem or input-locking shell behavior.
pub(super) fn configuration(source: &NixPath) -> NixValue {
    let candidate = source
        .join_relative("bootstrap/initial-runtime-config")
        .unwrap();
    nix_record! {
        "initialRuntimeConfigRepoSource": candidate.snapshot(),
    }
}
