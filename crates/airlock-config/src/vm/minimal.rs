//! Trim optional NixOS conveniences while retaining its ordinary systemd and Qubes runtime.
//! Personal application VMs select the full NixOS environment at construction time.

use rusix::{
    interop::{NixPath, raw::NixValue},
    nix_record,
    nixos::{ContextualModule, ModuleBody},
};

/// Select the upstream minimal profile and explicit Nix CLI capability before module evaluation.
pub(super) fn module(enabled: NixValue, nix_cli: NixValue) -> NixValue {
    ContextualModule::with_arguments(["modulesPath"], |context| {
        let minimal = context
            .argument::<NixPath>("modulesPath")
            .join_relative("profiles/minimal.nix")
            .expect("the upstream module path is relative");
        let trim = |value: NixValue| {
            context
                .lib
                .clone()
                .select("mkOverride")
                .call(50_i64)
                .call(value)
        };
        let default_off = context.lib.clone().select("mkDefault").call(false);
        let documentation = nix_record! {
            "enable": default_off.clone(),
            "doc": nix_record! {
                "enable": default_off.clone(),
            },
            "info": nix_record! {
                "enable": default_off.clone(),
            },
            "man": nix_record! {
                "enable": default_off.clone(),
            },
            "nixos": nix_record! {
                "enable": default_off,
            },
        };
        let configuration = nix_record! {
            "environment": nix_record! {
                "defaultPackages": trim(NixValue::list([])),
                "systemPackages": context
                    .lib
                    .clone()
                    .select("mkIf")
                    .call(nix_cli)
                    .call(NixValue::list([context.pkgs.select("nix")])),
            },
            "system": nix_record! {
                "etc": nix_record! {
                    "overlay": nix_record! {
                        "enable": context.lib.clone().select("mkDefault").call(true),
                    },
                },
                "disableInstallerTools": trim(true.into()),
            },
            "services": nix_record! {
                "userborn": nix_record! {
                    "enable": context.lib.clone().select("mkDefault").call(true),
                },
            },
            "documentation": documentation,
            "programs": nix_record! {
                "command-not-found": nix_record! {
                    "enable": trim(false.into()),
                },
                "bash": nix_record! {
                    "completion": nix_record! {
                        "enable": trim(false.into()),
                    },
                },
                "less": nix_record! {
                    "enable": trim(false.into()),
                },
                "nano": nix_record! {
                    "enable": trim(false.into()),
                },
            },
        };

        ModuleBody::new(
            context
                .lib
                .select("mkIf")
                .call(enabled.clone())
                .call(configuration),
        )
        .key("airlock.vm.minimal")
        .import_value(NixValue::if_else(enabled, minimal, nix_record! {}))
    })
    .into_expression()
}
