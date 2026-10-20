//! Build a host-only repair against an explicitly recorded deployed machine baseline.

use airlock_build::{Workspace, tools::Tool};
use rusix::{
    interop::{
        NixExpression, NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody},
};

/// Keep the baseline topology exact while replacing the deployment implementation.
pub(crate) fn constructor(workspace: &Workspace) -> NixValue {
    NixValue::function_attrs(
        ["snapshot", "baseline", "machine", "repairRevision"],
        |args| {
            let snapshot = args.clone().select("snapshot");
            let baseline = args.clone().select("baseline");
            let source = NixPath::from_expression(args.clone().select("machine"));
            let repair =
                crate::host::deployment::named_module(&source, workspace, "airlock.host.repair");
            let configuration = baseline.clone().select("lib.mkMachineConfiguration").call(
                nix_record! {
                    "system": "x86_64-linux",
                    "includeRelease": false,
                    "includeAlternativeGuiProfiles": false,
                    "extraSpecialArgs": nix_record! {
                        "airlockAppsSource": workspace.root(),
                    },
                    "extraModules": NixValue::list([ContextualModule::new(|context| {
                        let override_value = |value| {
                            context
                                .lib
                                .clone()
                                .select("mkOverride")
                                .call(40_i64)
                                .call(value)
                        };
                        let definitions = nix_record! {
                            "airlock": nix_record! {
                                "machineDeployment": nix_record! {
                                    "initialSource": nix_record! {
                                        "revision": snapshot.clone().select("revision"),
                                        "lockHash": snapshot.clone().select("lockHash"),
                                    },
                                },
                                "qubes": nix_record! {
                                    "domains": override_value(declarative(
                                        &context.lib,
                                        snapshot.clone().select("topology.domains"),
                                    )),
                                    "kernels": override_value(
                                        snapshot.clone().select("topology.kernels"),
                                    ),
                                    "prefs": override_value(
                                        snapshot.clone().select("topology.prefs"),
                                    ),
                                    "qrexecPolicy": override_value(
                                        snapshot.clone().select("topology.qrexec_policy"),
                                    ),
                                },
                            },
                            "environment": nix_record! {
                                "etc": nix_record! {
                                    "airlock/host-repair.json": nix_record! {
                                        "text": NixValue::builtin("toJSON").call(nix_record! {
                                            "repairRevision": args.clone().select("repairRevision"),
                                            "baselineHost": snapshot.clone().select("host"),
                                            "baselineRevision": snapshot.clone().select("revision"),
                                        }),
                                    },
                                },
                            },
                        };

                        ModuleBody::new(definitions)
                            .import_value(nix_record! {
                                "disabledModules": NixValue::list([nix_record! {
                                    "key": "airlock.host.deployment",
                                }]),
                            })
                            .import_value(repair)
                    })
                    .into_expression()]),
                },
            );

            let result = configuration.bind(|configuration| {
            let config = configuration
                .clone()
                .select("config.specialisation.usb-lab.configuration");
            let pkgs = configuration.select("pkgs");
            let engine = config.clone().select("system.build.machineDeploymentCoordinator");
            let setup = Tool::Setup.build(&pkgs, workspace, &source).as_expression();
            let manifest = nix_record! {
                "schema": "airlock.machine-release.v1",
                "profile": snapshot.clone().select("profile"),
                "revision": snapshot.clone().select("revision"),
                "lockHash": snapshot.clone().select("lockHash"),
                "host": config.clone().select("system.build.toplevel"),
                "coordinator": nix_text!("{engine}/bin/airlock-dom0-service", engine = engine),
                "setup": nix_text!("{setup}/bin/airlock-setup-config", setup = setup),
                "retireDomains": NixValue::list([]),
                "hostRepair": nix_record! {
                    "baselineHost": snapshot.clone().select("host"),
                    "topologyHash": snapshot.clone().select("topologyHash"),
                    "repairRevision": args.select("repairRevision"),
                },
            };
            let valid = baseline
                .clone()
                .select("rev")
                .equals(snapshot.clone().select("revision"))
                .and(
                    snapshot.clone().select("lockHash").equals(
                        NixValue::builtin("hashFile")
                            .call("sha256")
                            .call(nix_text!("{baseline}/flake.lock", baseline = baseline)),
                    ),
                )
                .and(
                    config
                        .select("airlock.qubes.topology")
                        .equals(snapshot.clone().select("topology")),
                );
            let output = pkgs
                .clone()
                .select("runCommand")
                .call("airlock-machine-host-repair")
                .call(nix_record! {
                    "releaseJson": super::store::json(&pkgs, "airlock-host-repair-release.json", manifest),
                    "topologyJson": super::store::json(
                        &pkgs,
                        "airlock-host-repair-topology.json",
                        snapshot.select("topology"),
                    ),
                })
                .call(
                    r#"
                mkdir -p "$out"
                ln -s "$releaseJson" "$out/release.json"
                ln -s "$topologyJson" "$out/topology.json"
            "#,
                );

            NixValue::if_else(
                valid,
                output,
                NixValue::builtin("throw")
                    .call("Host repair requires the exact recorded source, lock and runtime topology."),
            )
        });

            (Vec::<(&str, NixValue)>::new(), result)
        },
    )
}

/// Translate runtime aliases back to option names while retaining the admitted topology's values.
fn declarative(lib: &NixValue, value: NixValue) -> NixValue {
    let transform = lib
        .clone()
        .select("fix")
        .call(NixValue::function(|recurse| {
            NixValue::function(|value| {
                let attributes = lib
                    .clone()
                    .select("mapAttrs'")
                    .call(NixValue::function(|name| {
                        NixValue::function(|item| {
                            let renamed = nix_record! {
                                "source_ref": "sourceRef",
                                "read_only": "readOnly",
                                "persistent_size": "persistentSize",
                            };

                            nix_record! {
                                "name": renamed.attr_or(name.clone(), name),
                                "value": recurse.clone().call(item),
                            }
                        })
                    }))
                    .call(NixValue::builtin("removeAttrs").call(value.clone()).call(
                        NixValue::list(["blob_device".into(), "view_backend".into()]),
                    ));

                NixValue::if_else(
                    NixValue::builtin("isList").call(value.clone()),
                    NixValue::builtin("map").call(recurse).call(value.clone()),
                    NixValue::if_else(
                        NixValue::builtin("isAttrs").call(value.clone()),
                        attributes,
                        value,
                    ),
                )
            })
        }));

    transform.call(value)
}
