//! Exercise upstream NixOS /etc materialization with the real native entry modes.
//! A supplied launcher package avoids realizing the production VM applications.

use super::*;
use rusix::{nix_record, nix_text};

/// Retain native executable entries and their installation modes from one evaluated system.
fn entries(config: NixValue, scope: NixValue) -> NixValue {
    let candidates =
        NixValue::builtin("attrValues").call(config.select_segments(["environment", "etc"]));
    let native = NixValue::builtin("filter")
        .call(NixValue::function(|entry| {
            entry.clone().select("enable").and(
                !NixValue::builtin("match")
                    .call(".*/bin/airlock-[^/]+")
                    .call(entry.select("source").to_text())
                    .equals(NixValue::null()),
            )
        }))
        .call(candidates);

    NixValue::builtin("map")
        .call(NixValue::function(|entry| {
            NixValue::record([
                ("scope", scope.clone()),
                ("target", entry.clone().select("target")),
                ("mode", entry.clone().select("mode")),
                (
                    "source",
                    // Report production paths as data without realizing their application packages.
                    NixValue::builtin("unsafeDiscardStringContext")
                        .call(entry.select("source").to_text()),
                ),
            ])
        }))
        .call(native)
}

/// Build only upstream /etc artifacts, replacing executable contents while retaining authored modes.
pub(super) fn installation(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(inputs.clone().select("root"))
        .bind(|flake| {
            let configurations = flake.clone().select("nixosConfigurations");
            let host = configurations.clone().select_segments([
                "usb-dev",
                "config",
                "specialisation",
                "usb-lab",
                "configuration",
            ]);

            let domains = NixValue::builtin("filter")
                .call(NixValue::function(|name| {
                    !NixValue::builtin("match")
                        .call("airlock-domain-.*")
                        .call(name)
                        .equals(NixValue::null())
                }))
                .call(NixValue::builtin("attrNames").call(configurations.clone()));
            let guests = NixValue::builtin("map")
                .call(NixValue::function(|name| {
                    entries(
                        NixValue::builtin("getAttr")
                            .call(name.clone())
                            .call(configurations.clone())
                            .select("config"),
                        name,
                    )
                }))
                .call(domains);
            let all = NixValue::concat_lists([
                entries(host, "dom0".into()),
                NixValue::builtin("concatLists").call(guests),
            ]);

            let fixture = NixValue::builtin("path").call(nix_record! {
                "path": NixPath::from_source_text(
                    inputs.clone().select("fixture").to_text().into_expr::<String>(),
                ).as_expression(),
                "name": "airlock-native-etc-fixture",
            });

            all.bind(|all| {
                let files = NixValue::builtin("listToAttrs").call(
                    NixValue::builtin("map")
                        .call(NixValue::function(|entry| {
                            nix_record! {
                                "name": nix_text!(
                                    "{scope}/{target}",
                                    scope = entry.clone().select("scope"),
                                    target = entry.clone().select("target"),
                                ),
                                "value": nix_record! {
                                    "source": nix_text!(
                                        "{fixture}/bin/airlock-test-operation",
                                        fixture = fixture.clone(),
                                    ),
                                    "mode": entry.select("mode"),
                                },
                            }
                        }))
                        .call(all.clone()),
                );

                let module = NixValue::function_attrs_open(["lib"], |arguments| {
                    let config = nix_record! {
                        "system": nix_record! {
                            "stateVersion": "26.11",
                            "etc": nix_record! {
                                "overlay": nix_record! { "enable": true },
                            },
                        },
                        "environment": nix_record! {
                            "etc": arguments.select_segments(["lib", "mkForce"]).call(files),
                        },
                    };
                    (Vec::<(&str, NixValue)>::new(), config)
                });
                let system = flake
                    .select_segments(["inputs", "nixpkgs", "lib", "nixosSystem"])
                    .call(nix_record! {
                        "system": "x86_64-linux",
                        "modules": NixValue::list([module]),
                    });

                system.bind(|system| {
                    let pkgs = system.clone().select("pkgs");
                    let build = system.select_segments(["config", "system", "build"]);
                    let report = pkgs
                        .clone()
                        .select("writeText")
                        .call("native-etc-entries.json")
                        .call(NixValue::builtin("toJSON").call(all));

                    pkgs.select("linkFarm")
                        .call("airlock-native-etc-test")
                        .call(NixValue::list([
                            nix_record! {
                                "name": "etc",
                                "path": build.clone().select("etc"),
                            },
                            nix_record! {
                                "name": "basedir",
                                "path": build.select("etcBasedir"),
                            },
                            nix_record! { "name": "fixture", "path": fixture },
                            nix_record! { "name": "entries.json", "path": report },
                        ]))
                })
            })
        })
}

/// Build the host's actual upstream RPC join with a tiny immutable root for command preflight.
pub(super) fn host_rpcs(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(inputs.select("root"))
        .bind(|flake| {
            let host = flake
                .clone()
                .select("nixosConfigurations.usb-dev.config.specialisation.usb-lab.configuration");
            let pkgs = flake.select("inputs.nixpkgs.legacyPackages.x86_64-linux");
            let join = pkgs
                .clone()
                .select("lib.removeSuffix")
                .call("/*")
                .call(host.select_segments(["environment", "etc", "qubes-rpc", "source"]));
            let init = pkgs
                .clone()
                .select("writeShellScript")
                .call("rpc-preflight-init")
                .call("exit 0\n");
            let system = pkgs
                .clone()
                .select("runCommand")
                .call("airlock-host-rpc-preflight")
                .call(nix_record! {})
                .call(nix_text!(
                    r#"
                mkdir -p "$out/etc" "$out/sw/bin"
                ln -s {shell}/bin/sh "$out/sw/bin/sh"
                ln -s {coreutils}/bin/env "$out/sw/bin/env"
                ln -s {init} "$out/activate"
                ln -s {init} "$out/init"
                ln -s {join} "$out/etc/qubes-rpc"
            "#,
                    init = init,
                    join = join,
                    shell = pkgs.clone().select("bash"),
                    coreutils = pkgs.clone().select("coreutils"),
                ));
            system.bind(|system| {
                let closure = pkgs.clone().select("closureInfo").call(nix_record! {
                    "rootPaths": NixValue::list([system.clone()]),
                });
                pkgs.select("linkFarm")
                    .call("airlock-host-rpc-preflight-test")
                    .call(NixValue::list([
                        nix_record! { "name": "system", "path": system },
                        nix_record! { "name": "closure", "path": closure },
                    ]))
            })
        })
}
