//! Expose the shared starter configuration as data for native configuration-role initialization.

use airlock_build::Workspace;
use rusix::{interop::raw::NixValue, nix_record};

/// Snapshot the initial candidate without embedding filesystem or input-locking shell behavior.
pub(super) fn configuration(workspace: &Workspace) -> NixValue {
    let candidate = workspace.file("airlock-config/templates/initial-runtime-config");
    nix_record! {
        "initialRuntimeConfigRepoSource": candidate.snapshot(),
    }
}
