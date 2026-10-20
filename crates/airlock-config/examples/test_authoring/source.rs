//! Minimal source fixtures for tests that inspect archives or review diffs.

use super::*;

/// Supply an inert flake when only repository structure matters.
pub(super) fn empty() -> NixValue {
    field(
        ["outputs"],
        NixValue::function(|_| NixValue::record(Vec::<(&str, NixValue)>::new())),
    )
}

/// Fail if controller startup evaluates the root flake while reading source changes.
pub(super) fn forbidden_root() -> NixValue {
    NixValue::record([
        (
            "inputs",
            field(["self", "submodules"], NixValue::from(true)),
        ),
        (
            "outputs",
            NixValue::function(|_| {
                NixValue::builtin("throw").call("root flake must not be loaded")
            }),
        ),
    ])
}
