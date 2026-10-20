//! Define machine deployment policy consumed by the native coordinator and release authoring.

use rusix::{interop::raw::NixValue, nix_record, nixos::ModuleContext};

/// Preserve explicit source identity, human review and recoverable VM retirement settings.
pub(super) fn declarations(context: &ModuleContext) -> NixValue {
    let lib = &context.lib;
    let boolean = lib.clone().select("types.bool");
    let string = lib.clone().select("types.str");
    let option = |kind, default, description| {
        lib.clone().select("mkOption").call(nix_record! {
            "type": kind,
            "default": default,
            "description": description,
        })
    };
    let hash = |pattern| {
        lib.clone().select("mkOption").call(nix_record! {
            "type": lib.clone().select("types.strMatching").call(pattern),
        })
    };
    let source = lib.clone().select("types.submodule").call(nix_record! {
        "options": nix_record! {
            "revision": hash("[0-9a-f]{40}"),
            "lockHash": hash("[0-9a-f]{64}"),
        },
    });

    nix_record! {
        "options": nix_record! {
            "airlock": nix_record! {
                "machineDeployment": nix_record! {
                    "enable": lib
                        .clone()
                        .select("mkEnableOption")
                        .call("persistent repository-owned machine deployments"),
                    "userReview": option(
                        boolean.clone(),
                        true.into(),
                        "Require human source and impact approval before builds and activation.",
                    ),
                    "profile": option(
                        string.clone(),
                        "default".into(),
                        "Machine profile selected for deployment.",
                    ),
                    "initialSource": option(
                        lib.clone().select("types.nullOr").call(source),
                        NixValue::null(),
                        "Machine source built into the image independently of its parent repository.",
                    ),
                    "generationRetention": nix_record! {
                        "enable": option(
                            boolean,
                            true.into(),
                            "Retain verified machine generations and queue serialized pool collection \
                     after verification.",
                        ),
                    },
                    "retireDomains": option(
                        lib.clone().select("types.listOf").call(string.clone()),
                        NixValue::list([]),
                        "VMs intentionally removed by this release and moved to recoverable trash.",
                    ),
                    "privateVolumeCopies": option(
                        lib.clone().select("types.attrsOf").call(string),
                        nix_record! {},
                        "New VM names mapped to retired source VMs whose private volumes are copied once.",
                    ),
                    "trashRetentionDays": option(
                        lib.clone()
                            .select("types.ints.between")
                            .call(1_i64)
                            .call(3650_i64),
                        30_i64.into(),
                        "Minimum age before unreferenced VM recovery data may be explicitly purged.",
                    ),
                },
            },
        },
    }
}
