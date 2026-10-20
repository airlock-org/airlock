//! Persist early boot evidence before the normal host storage view is available.

use airlock_build::{Application, Workspace};
use rusix::{
    interop::raw::{NixRepresentation, NixValue},
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody},
};

/// Install the initramfs recorder and bounded persistent journald storage.
/// Recovery builds its own recorder against its own package set.
pub(super) fn module(workspace: &Workspace) -> NixValue {
    ContextualModule::new(|context| {
        let service = Application::Dom0
            .build(&context.pkgs, workspace)
            .as_expression();
        let recorder = nix_text!(
            "{service}/bin/airlock-boot-journal",
            service = service.select("unwrapped"),
        );
        let default = |value| context.lib.clone().select("mkDefault").call(value);

        ModuleBody::new(nix_record! {
            "boot": nix_record! {
                "consoleLogLevel": context.lib.clone().select("mkForce").call(7_i64),
                "kernelParams": NixValue::list([
                    "systemd.show_status=1".into(),
                    "rd.systemd.show_status=1".into(),
                ]),
                "initrd": nix_record! {
                    "verbose": true,
                    "systemd": nix_record! {
                        "storePaths": NixValue::list([recorder.clone().into()]),
                        "services": nix_record! {
                            "airlock-boot-journal": nix_record! {
                                "description": "Save initramfs journal to the root filesystem",
                                "wantedBy": NixValue::list(["initrd.target".into()]),
                                "after": NixValue::list([
                                    "sysroot.mount".into(),
                                    "systemd-journald.service".into(),
                                ]),
                                "requires": NixValue::list(["sysroot.mount".into()]),
                                "before": NixValue::list([
                                    "airlock-host-store.service".into(),
                                    "initrd-find-nixos-closure.service".into(),
                                ]),
                                "unitConfig": nix_record! {
                                    "DefaultDependencies": false,
                                    "IgnoreOnIsolate": true,
                                },
                                "serviceConfig": nix_record! {
                                    "Type": "simple",
                                    "ExecStart": nix_text!(
                                        "{recorder} {systemd}/bin/journalctl",
                                        recorder = recorder,
                                        systemd =
                                            context.config.select("boot.initrd.systemd.package"),
                                    ),
                                    "StandardOutput": "journal+console",
                                    "StandardError": "journal+console",
                                },
                            },
                        },
                    },
                },
            },
            // Journald owns stage 2 and the handoff's /run journal on bounded USB storage.
            "services": nix_record! {
                "journald": nix_record! {
                    "settings": nix_record! {
                        "Journal": nix_record! {
                            "Storage": "persistent",
                            "SystemMaxUse": default("128M"),
                            "SyncIntervalSec": default("5s"),
                        },
                    },
                },
            },
            "systemd": nix_record! {
                "tmpfiles": nix_record! {
                    "rules": NixValue::list(["d /var/log/airlock-boot 0700 root root - -".into()]),
                },
            },
        })
        .key("airlock.host.boot-diagnostics")
    })
    .into_expression()
}
