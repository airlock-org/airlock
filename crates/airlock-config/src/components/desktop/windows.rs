//! Supervise Qubes window forwarding and restore trusted VM borders at boot.
//! Applications retain their own title bars; the GUI daemon supplies VM identity borders.

use super::{MachineFiles, ModuleContext};
use airlock_sys_gui_service::command::DesktopCommand;
use rusix::{interop::raw::NixValue, nix_record, nix_text};

/// Installs the upstream watcher, declarative border restoration and GUI-daemon patch.
/// The watcher retries when login precedes dom0's Admin API readiness.
pub(super) fn configuration(inputs: &ModuleContext, service: &super::Deployment) -> NixValue {
    let style = NixValue::builtin("fromJSON")
        .call(
            NixValue::builtin("readFile").call(inputs.file("assets/sys-gui/gnome/gui-style.json")),
        )
        .merge_attrs(nix_record! {
            "revision": NixValue::null(),
        });
    let boot_style = inputs
        .pkgs
        .clone()
        .select("writeText")
        .call("airlock-gui-style-boot.json")
        .call(NixValue::builtin("toJSON").call(style));
    let watcher = nix_text!(
        "{client}/bin/qvm-start-daemon --all --watch",
        client = inputs
            .pkgs
            .clone()
            .select("python3.pkgs.qubes-core-admin-client"),
    );

    nix_record! {
        "systemd": nix_record! {
            "services": nix_record! {
                "airlock-gui-style-default": nix_record! {
                    "description": "Restore the booted generation's declarative VM border style",
                    "wantedBy": NixValue::list(["multi-user.target".into()]),
                    "after": NixValue::list(["airlock-qubes-rw-init.service".into()]),
                    "requires": NixValue::list(["airlock-qubes-rw-init.service".into()]),
                    "before": NixValue::list(["display-manager.service".into()]),
                    "serviceConfig": nix_record! {
                        "Type": "oneshot",
                    },
                    "script": nix_text!(
                        r#"
                        {command} < {style}
                    "#,
                        command = service.shell_command(&DesktopCommand::GuiStyle {
                            boot: true,
                        }),
                        style = boot_style,
                    ),
                },
            },
            "user": nix_record! {
                "services": nix_record! {
                    "airlock-gui-daemons": nix_record! {
                        "description": "Forward windows from VMs assigned to this GUI session",
                        "wantedBy": NixValue::list(["graphical-session.target".into()]),
                        "after": NixValue::list(["graphical-session-pre.target".into()]),
                        "partOf": NixValue::list(["graphical-session.target".into()]),
                        "unitConfig": nix_record! {
                            "StartLimitIntervalSec": 0_i64,
                        },
                        // The upstream event reader runs qrexec-client-vm by name.
                        "path": NixValue::list([
                            inputs.pkgs.clone().select("coreutils"),
                            inputs.pkgs.clone().select("qubes-gui-daemon"),
                            inputs
                                .pkgs
                                .clone()
                                .select("python3.pkgs.qubes-core-admin-client"),
                            inputs
                                .pkgs
                                .clone()
                                .select("python3.pkgs.qubes-core-qrexec.domU"),
                        ]),
                        "serviceConfig": nix_record! {
                            "ExecStart": watcher,
                            "Restart": "always",
                            "RestartSec": "5s",
                        },
                    },
                },
            },
        },
        // The supervised watcher replaces the upstream one-shot autostart entry.
        "environment": nix_record! {
            "etc": nix_record! {
                "xdg/autostart/qvm-start-daemon.desktop": nix_record! {
                    "text": nix_text!(r#"
                        [Desktop Entry]
                        Type=Application
                        Name=Qubes Guid/Pacat
                        Hidden=true
                    "#),
                },
            },
        },
        "nixpkgs": nix_record! {
            "overlays": inputs
                .lib
                .clone()
                .select("mkAfter")
                .call(NixValue::list([border_overlay(inputs)])),
        },
    }
}

/// Adds application-owned window chrome and advertises runtime border-reload support.
/// Appending preserves the upstream daemon's installation steps and existing patches.
fn border_overlay(inputs: &ModuleContext) -> NixValue {
    NixValue::function(|_final| {
        NixValue::function(|previous| {
            nix_record! {
                "qubes-gui-daemon": previous
                    .select("qubes-gui-daemon")
                    .override_attrs(NixValue::function(|old| nix_record! {
                        "postInstall": NixValue::concat_text([
                            old.clone().attr_or("postInstall", ""),
                            nix_text!(r#"
                                        mkdir -p "$out/share/airlock"
                                        touch "$out/share/airlock/gui-border-reload-v1"
                                    "#)
                            .into(),
                        ]),
                        "patches": NixValue::concat_lists([
                            old.attr_or("patches", NixValue::list([])),
                            NixValue::list([inputs.file(
                                "patches/qubes-gui-daemon/\
                                                 0003-gui-daemon-leave-window-chrome-to-applications.patch",
                            )]),
                        ]),
                    })),
            }
        })
    })
}
