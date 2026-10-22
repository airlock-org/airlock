//! Run installed production input receivers in a small disposable Linux VM without Qubes control services.

use super::Build;
use airlock_build::{Application, vm::Images};
use rusix::{
    interop::{NixExpression, raw::NixValue},
    nix_record, nix_text,
};

/// Project installed receiver entries exactly, including native launcher links and root RPC configuration.
pub(super) fn fixture(build: &Build) -> NixValue {
    let pkgs = build.pkgs("x86_64-linux".into());
    let gui = build.output("nixosConfigurations.airlock-domain-sys-gui-usb-gui-lab.config");
    let services = [
        "airlock.InputDevice",
        "airlock.InputReceiverReady",
        "qubes.InputKeyboard",
        "qubes.InputMouse",
        "qubes.InputTablet",
    ];
    let files = NixValue::record(
        services
            .into_iter()
            .flat_map(|service| {
                [
                    format!("qubes-rpc/{service}"),
                    format!("qubes/rpc-config/{service}"),
                ]
            })
            .map(|key| {
                let entry = gui.clone().select_segments(["environment", "etc", &key]);
                (
                    key,
                    nix_record! {
                        "source": entry.clone().select("source"),
                        "mode": entry.select("mode"),
                    },
                )
            }),
    );
    let probe = Application::InputTest.build(&pkgs, &build.workspace);
    let module = nix_record! {
        "imports": NixValue::list([nix_text!(
            "{nixpkgs}/nixos/modules/virtualisation/qemu-vm.nix",
            nixpkgs = build.inputs.clone().select("nixpkgs"),
        ).into()]),
        "system": nix_record! {
            "stateVersion": "26.11",
        },
        "networking": nix_record! {
            "hostName": "airlock-input-test",
            "useDHCP": false,
        },
        "boot": nix_record! {
            "kernelModules": NixValue::list(["uinput".into()]),
        },
        "environment": nix_record! {
            "etc": files.merge_attrs(nix_record! {
                "airlock-input-test": nix_record! {
                    "text": "disposable installed input RPC regression\n",
                },
            }),
        },
        "systemd": nix_record! {
            "services": nix_record! {
                "airlock-input-smoke": nix_record! {
                    "wantedBy": NixValue::list(["multi-user.target".into()]),
                    "after": NixValue::list([
                        "systemd-udev-trigger.service".into(),
                        "systemd-modules-load.service".into(),
                    ]),
                    "serviceConfig": nix_record! {
                        "Type": "oneshot",
                        "ExecStart": nix_text!(
                            "{probe}/bin/airlock-input-probe --inside-test-vm",
                            probe = probe,
                        ),
                        "ExecStopPost": nix_text!(
                            "{systemd}/bin/systemctl --no-block poweroff",
                            systemd = pkgs.clone().select("systemd"),
                        ),
                        "TimeoutStartSec": 60_i64,
                        "StandardOutput": "journal+console",
                        "StandardError": "journal+console",
                    },
                },
            },
        },
        "virtualisation": nix_record! {
            "memorySize": 768_i64,
            "cores": 2_i64,
            "graphics": false,
            // A real closure image prevents a host-store mount from hiding missing runtime objects.
            "useNixStoreImage": true,
        },
    };
    let system = build.lib.clone().select("nixosSystem").call(nix_record! {
        "system":"x86_64-linux", "pkgs":pkgs.clone(), "modules":NixValue::list([module]),
    });
    system.bind(|system| {
        let root = system.clone().select("config.system.build.toplevel");
        let manifest = Images {
            pkgs: &pkgs,
            workspace: &build.workspace,
        }
        .boot_manifest(
            "input-test".into(),
            NixValue::list([root.clone()]),
            "/nix/store".into(),
            "/nix/store".into(),
        );
        pkgs.clone()
            .select("linkFarm")
            .call("airlock-input-receiver-test")
            .call(NixValue::list([
                nix_record! { "name":"vm", "path":system.select("config.system.build.vm") },
                nix_record! { "name":"manifest", "path":manifest },
            ]))
    })
}
