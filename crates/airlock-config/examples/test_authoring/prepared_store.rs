//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test usb prepared store preserves sealed view mode.
pub(super) fn sealed_store(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .select_segments(["nixosConfigurations", "usb-dev", "config"])
        .bind(prepared_store_sealed_store_body)
}

/// Build the prepared store test usb prepared store preserves sealed view mode contribution using
/// the caller supplied deferred values.
fn prepared_store_sealed_store_body(config: NixValue) -> NixValue {
    {
        let field_recovery_recorder = config.clone().select_segments([
            "specialisation",
            "usb-text-recovery",
            "configuration",
            "boot",
            "initrd",
            "systemd",
            "services",
            "airlock-boot-journal",
            "serviceConfig",
            "ExecStart",
        ]);

        NixValue::record([
            (
                "mode",
                config
                    .clone()
                    .select_segments(["airlock", "image", "preparedStoreMode"]),
            ),
            (
                "image",
                config
                    .clone()
                    .select_segments(["system", "build", "image", "drvPath"]),
            ),
            (
                "validation",
                config.clone().select_segments([
                    "system",
                    "build",
                    "usbPreparedGuestStore",
                    "validation",
                    "outPath",
                ]),
            ),
            (
                "checkCommand",
                config.clone().select_segments([
                    "system",
                    "build",
                    "usbPreparedGuestStore",
                    "checkCommand",
                ]),
            ),
            (
                "preVM",
                config
                    .clone()
                    .select_segments(["system", "build", "image", "preVM"]),
            ),
            (
                "imageCommand",
                config
                    .clone()
                    .select_segments(["system", "build", "image", "buildCommand"]),
            ),
            (
                "runtimeSeed",
                config.clone().select_segments([
                    "system",
                    "build",
                    "usbPreparedRuntime",
                    "outPath",
                ]),
            ),
            (
                "dependencies",
                NixValue::builtin("map")
                    .call(NixValue::builtin("toString"))
                    .call(
                        config
                            .clone()
                            .select_segments(["system", "extraDependencies"]),
                    ),
            ),
            (
                "hostOrdering",
                config.clone().select_segments([
                    "boot",
                    "initrd",
                    "systemd",
                    "services",
                    "initrd-find-nixos-closure",
                    "after",
                ]),
            ),
            (
                "earlyLogs",
                config.clone().select_segments([
                    "boot",
                    "initrd",
                    "systemd",
                    "services",
                    "airlock-boot-journal",
                    "serviceConfig",
                    "ExecStart",
                ]),
            ),
            (
                "recoveryLogs",
                config
                    .clone()
                    .select_segments([
                        "specialisation",
                        "usb-text-recovery",
                        "configuration",
                        "boot",
                        "initrd",
                        "systemd",
                        "services",
                    ])
                    .has_attr("airlock-boot-journal"),
            ),
            ("recoveryRecorder", field_recovery_recorder),
            (
                "journalConfig",
                config
                    .clone()
                    .select_segments(["services", "journald", "settings", "Journal"]),
            ),
            (
                "daemonConfig",
                config.clone().select_segments([
                    "systemd",
                    "services",
                    "nix-daemon",
                    "environment",
                    "NIX_CONFIG",
                ]),
            ),
        ])
    }
}
