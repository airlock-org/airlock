//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test network uplink hotplug.
pub(super) fn uplink_hotplug(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([
            NixValue::from("path:"),
            inputs.clone().select("root").to_text(),
        ]))
        .bind(|flake| {
            flake
                .clone()
                .select_segments(["airlock", "tests", "uplinkHotplug"])
        })
}

/// Select the disposable VM that exercises the actual installed production input RPCs.
pub(super) fn input_receiver(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([
            NixValue::from("path:"),
            inputs.select("root").to_text(),
        ]))
        .select_segments(["airlock", "tests", "inputReceiver"])
}
