//! Inspect command-role isolation and minimal VM services as separate authoring scenarios.

use super::*;

mod command;

mod minimal;

/// Inspect command-role isolation and integration using the supplied source paths.
pub(super) fn command_role(inputs: NixValue) -> NixValue {
    command::command_role(inputs)
}

/// Inspect minimal system VM services without evaluating Nix during generation.
pub(super) fn minimal_vm(inputs: NixValue) -> NixValue {
    minimal::minimal_vm(inputs)
}
