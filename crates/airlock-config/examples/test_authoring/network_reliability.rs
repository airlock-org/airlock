//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test network configuration has recovery firmware and one
/// dns owner.
pub(super) fn network_recovery(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([
            NixValue::from("path:"),
            inputs.clone().select("root").to_text(),
        ]))
        .bind(|f| {
            f.clone()
                .select_segments(["nixosConfigurations", "airlock-domain-sys-net", "config"])
                .bind(|net| network_reliability_network_recovery_body(net, f.clone()))
        })
}

/// Build the network reliability test network configuration has recovery firmware and one dns
/// owner contribution using the caller supplied deferred values.
fn network_reliability_network_recovery_body(net: NixValue, f: NixValue) -> NixValue {
    f.clone()
        .select_segments([
            "nixosConfigurations",
            "airlock-domain-sys-firewall",
            "config",
        ])
        .bind(|fw| {
            NixValue::record([
                (
                    "firmware",
                    net.clone()
                        .select_segments(["hardware", "enableRedistributableFirmware"]),
                ),
                (
                    "netResolver",
                    net.clone()
                        .select_segments(["networking", "resolvconf", "enable"]),
                ),
                (
                    "fwResolver",
                    fw.clone()
                        .select_segments(["networking", "resolvconf", "enable"]),
                ),
                (
                    "fwCommands",
                    fw.clone().select_segments(["networking", "localCommands"]),
                ),
                (
                    "netRestart",
                    net.clone().select_segments([
                        "systemd",
                        "services",
                        "qubes-firewall",
                        "serviceConfig",
                        "Restart",
                    ]),
                ),
                (
                    "fwRestart",
                    fw.clone().select_segments([
                        "systemd",
                        "services",
                        "qubes-firewall",
                        "serviceConfig",
                        "Restart",
                    ]),
                ),
            ])
        })
}
