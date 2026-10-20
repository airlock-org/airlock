//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test prepared VMs use the patched reporter.
pub(super) fn patched_reporter(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(memory_accounting_nix_patched_reporter_body)
}

/// Build the memory accounting nix test prepared VMs use the patched reporter contribution
/// using the caller supplied deferred values.
fn memory_accounting_nix_patched_reporter_body(f: NixValue) -> NixValue {
    NixValue::function(|name| {
        NixValue::builtin("map")
            .call(NixValue::builtin("baseNameOf"))
            .call(
                NixValue::builtin("getAttr")
                    .call(name.clone())
                    .call(f.clone().select_segments(["nixosConfigurations"]))
                    .select_segments(["config", "nixpkgs", "pkgs", "qubes-linux-utils", "patches"]),
            )
    })
    .bind(|patches| {
        NixValue::record([
            (
                "assistant",
                patches
                    .clone()
                    .call(NixValue::from("airlock-domain-usr-assistant")),
            ),
            (
                "home",
                patches
                    .clone()
                    .call(NixValue::from("airlock-domain-usr-home")),
            ),
            (
                "nativeCheckEnabled",
                f.clone().select_segments([
                    "nixosConfigurations",
                    "airlock-domain-usr-assistant",
                    "config",
                    "nixpkgs",
                    "pkgs",
                    "qubes-linux-utils",
                    "doCheck",
                ]),
            ),
        ])
    })
}
