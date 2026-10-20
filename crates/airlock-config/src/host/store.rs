//! Assemble the selected host pool before stage 2 and keep the daemon on that same overlay.

use super::diagnostics;
use airlock_build::{Application, Workspace};
use airlock_machine_core::host_store;
use rusix::{
    interop::raw::{NixRepresentation, NixValue},
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody},
};

/// Install early host-store assembly and the daemon backend shared with the runtime service.
pub(super) fn module(workspace: &Workspace) -> NixValue {
    ContextualModule::new(|context| {
        let package = |name| context.pkgs.clone().select(name);
        let storage = Application::Dom0
            .build(&context.pkgs, workspace)
            .as_expression()
            .select("unwrapped");
        let tools = NixValue::list(["nix", "util-linux", "lvm2", "coreutils"].map(package));
        let prerequisite = NixValue::list(["airlock-host-store.service".into()]);
        let closure_ordering = context
            .lib
            .clone()
            .select("mkIf")
            .call(
                context
                    .config
                    .clone()
                    .select("system.nixos-init.enable")
                    .equals(false),
            )
            .call(nix_record! {
                "requires": prerequisite.clone(),
                "after": prerequisite,
            });

        ModuleBody::new(nix_record! {
            "assertions": NixValue::list([nix_record! {
                "assertion": context.config.select("boot.initrd.systemd.enable"),
                "message": "The host pool requires the systemd initramfs.",
            }]),
            "boot": nix_record! {
                "supportedFilesystems": NixValue::list(["erofs".into(), "overlay".into()]),
                "initrd": nix_record! {
                    "kernelModules": NixValue::list(["erofs", "overlay", "loop", "dm_mod"].map(Into::into)),
                    "systemd": nix_record! {
                        "storePaths": NixValue::concat_lists([
                            NixValue::list([storage.clone()]),
                            tools.clone(),
                        ]),
                        "services": nix_record! {
                            "airlock-host-store": nix_record! {
                                "description": "Assemble selected immutable host pool before stage 2",
                                "requiredBy": NixValue::list(["initrd-root-fs.target".into()]),
                                "after": NixValue::list(["sysroot.mount".into()]),
                                "requires": NixValue::list(["sysroot.mount".into()]),
                                "before": NixValue::list(
                                    [
                                        "initrd-root-fs.target",
                                        "initrd-nixos-activation.service",
                                        "initrd-find-nixos-closure.service",
                                    ]
                                    .map(Into::into),
                                ),
                                "unitConfig": nix_record! {
                                    "DefaultDependencies": false,
                                },
                                "path": tools,
                                "serviceConfig": nix_record! {
                                    "Type": "oneshot",
                                    "RemainAfterExit": true,
                                    "PrivateMounts": false,
                                    "StandardOutput": "journal+console",
                                    "StandardError": "journal+console",
                                    "ExecStart": nix_text!(
                                        "{storage}/bin/airlock-dom0-service storage host assemble",
                                        storage = storage,
                                    ),
                                },
                            },
                            // The overlay is mounted by a service, so mount-unit ordering is insufficient.
                            "initrd-find-nixos-closure": closure_ordering,
                        },
                    },
                },
            },
            "nix": nix_record! {
                "settings": nix_record! {
                    "experimental-features": NixValue::list([
                        "local-overlay-store".into(),
                        "read-only-local-store".into(),
                    ]),
                    "store": "daemon",
                },
            },
            "environment": nix_record! {
                "etc": nix_record! {
                    "airlock/host-store": nix_record! {
                        "text": format!("{uri}\n", uri = host_store::URI),
                    },
                },
            },
            "systemd": nix_record! {
                // Clients use the daemon; the daemon itself must open the local backend.
                // systemd also expands percent specifiers in Environment assignments.
                "services": nix_record! {
                    "nix-daemon": nix_record! {
                        "environment": nix_record! {
                            "NIX_CONFIG": format!(
                                "store = {uri}",
                                uri = host_store::URI.replace('%', "%%")
                            ),
                        },
                    },
                },
                // Stage 1 mounts survive switch-root; never remount an active host store here.
                "tmpfiles": nix_record! {
                    "rules": NixValue::list([format!(
                        "d {root} 0700 root root - -",
                        root = host_store::ROOT
                    )
                    .into()]),
                },
            },
        })
        .key("airlock.host.store")
        .import_value(diagnostics::module(workspace))
    })
    .into_expression()
}
