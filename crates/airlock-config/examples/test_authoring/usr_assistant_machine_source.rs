//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test shallow seed archive contains only the pinned checkout.
pub(super) fn seed_archive(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([
            inputs.clone().select("root").to_text(),
            NixValue::from("/airlock-machine"),
        ]))
        .bind(|flake| {
            flake
                .clone()
                .select_segments(["lib", "mkMachineGuestEntries"])
                .call(NixValue::from("x86_64-linux"))
                .call(field(
                    ["airlockMachineSeedArchive"],
                    NixValue::from("/seed.tar.gz"),
                ))
                .bind(|entries| {
                    entries.clone().select_segments([
                        "usr-assistant",
                        "configuration",
                        "config",
                        "systemd",
                        "services",
                        "airlock-machine-source-seed",
                        "script",
                    ])
                })
        })
}
