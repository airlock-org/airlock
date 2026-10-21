//! Inspect the minimal system VMs and their admitted authoring inputs.

use super::*;

/// Inspect service and authoring-input contracts for the minimal system VMs.
pub(super) fn minimal_vm(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([
            NixValue::from("path:"),
            inputs.clone().select("repo_root").to_text(),
        ]))
        .bind(|flake| {
            NixValue::function(|name| {
                NixValue::function(|values| {
                    NixValue::builtin("elem")
                        .call(name.clone())
                        .call(values.clone())
                })
            })
            .bind(|has| {
                flake
                    .clone()
                    .select_segments(["inputs", "nixpkgs", "lib"])
                    .bind(|lib| select_configurations(lib, flake.clone(), has.clone()))
            })
        })
}

/// Report service counts, runtime support and admitted inputs across the selected VMs.
fn inspect_services(
    authoring_inputs: NixValue,
    all_cfgs: NixValue,
    cfgs: NixValue,
    enabled_services: NixValue,
    has: NixValue,
    lib: NixValue,
) -> NixValue {
    {
        let field_service_counts = lib
            .clone()
            .select_segments(["mapAttrs"])
            .call(NixValue::function(|_name| {
                NixValue::function(|cfg| {
                    NixValue::builtin("length").call(enabled_services.clone().call(cfg.clone()))
                })
            }))
            .call(cfgs.clone());

        let field_documentation_disabled = all_cfgs.clone().call(NixValue::function(|cfg| {
            cfg.clone()
                .select_segments(["documentation", "enable"])
                .equals(NixValue::from(false))
                .and(
                    cfg.clone()
                        .select_segments(["documentation", "doc", "enable"])
                        .equals(NixValue::from(false)),
                )
                .and(
                    cfg.clone()
                        .select_segments(["documentation", "info", "enable"])
                        .equals(NixValue::from(false)),
                )
                .and(
                    cfg.clone()
                        .select_segments(["documentation", "man", "enable"])
                        .equals(NixValue::from(false)),
                )
                .and(
                    cfg.clone()
                        .select_segments(["documentation", "nixos", "enable"])
                        .equals(NixValue::from(false)),
                )
        }));

        let field_perlless_mechanics_enabled = all_cfgs.clone().call(NixValue::function(|cfg| {
            cfg.clone()
                .select_segments(["system", "etc", "overlay", "enable"])
                .equals(NixValue::from(true))
                .and(
                    cfg.clone()
                        .select_segments(["services", "userborn", "enable"])
                        .equals(NixValue::from(true)),
                )
        }));

        let field_nix_cli_capability = NixValue::record([
            (
                "staging",
                NixValue::builtin("elem")
                    .call(cfgs.clone().select_segments(["staging", "nix", "package"]))
                    .call(cfgs.clone().select_segments([
                        "staging",
                        "environment",
                        "systemPackages",
                    ])),
            ),
            (
                "validator",
                NixValue::builtin("elem")
                    .call(
                        cfgs.clone()
                            .select_segments(["validator", "nix", "package"]),
                    )
                    .call(cfgs.clone().select_segments([
                        "validator",
                        "environment",
                        "systemPackages",
                    ])),
            ),
            (
                "vault",
                NixValue::builtin("elem")
                    .call(cfgs.clone().select_segments(["vault", "nix", "package"]))
                    .call(
                        cfgs.clone()
                            .select_segments(["vault", "environment", "systemPackages"]),
                    ),
            ),
        ]);

        let field_stock_runtime_retained = all_cfgs.clone().call(NixValue::function(|cfg| {
            cfg.clone()
                .attr_or("systemd", NixValue::record(Vec::<(&str, NixValue)>::new()))
                .attr_or("package", NixValue::record(Vec::<(&str, NixValue)>::new()))
                .attr_or("pname", NixValue::from(""))
                .equals(NixValue::from("systemd"))
                .and(
                    cfg.clone()
                        .select_segments(["services", "dbus", "enable"])
                        .equals(NixValue::from(true)),
                )
                .and(
                    cfg.clone()
                        .select_segments(["networking", "useDHCP"])
                        .equals(NixValue::from(false)),
                )
                .and(
                    cfg.clone()
                        .select_segments(["systemd", "services"])
                        .has_attr("qubes-network-uplink@"),
                )
        }));

        let field_required_qubes_services_retained =
            all_cfgs.clone().call(NixValue::function(|cfg| {
                lib.clone()
                    .select_segments(["all"])
                    .call(NixValue::function(|name| {
                        has.clone()
                            .call(name.clone())
                            .call(enabled_services.clone().call(cfg.clone()))
                    }))
                    .call(NixValue::list([
                        NixValue::from("airlock-qubes-rw-init"),
                        NixValue::from("qubes-db"),
                        NixValue::from("qubes-qrexec-agent"),
                        NixValue::from("qubes-sysinit"),
                    ]))
            }));

        let field_config_inputs_import_rpc = NixValue::record([
            (
                "staging",
                cfgs.clone()
                    .select_segments(["staging", "environment", "etc"])
                    .has_attr("qubes-rpc/airlock.Config.InputsImport"),
            ),
            (
                "validator",
                cfgs.clone()
                    .select_segments(["validator", "environment", "etc"])
                    .has_attr("qubes-rpc/airlock.Config.InputsImport"),
            ),
            (
                "vault",
                cfgs.clone()
                    .select_segments(["vault", "environment", "etc"])
                    .has_attr("qubes-rpc/airlock.Config.InputsImport"),
            ),
        ]);

        NixValue::record([
            (
                "enabled_services",
                lib.clone()
                    .select_segments(["mapAttrs"])
                    .call(NixValue::function(|_name| {
                        NixValue::function(|cfg| enabled_services.clone().call(cfg.clone()))
                    }))
                    .call(cfgs.clone()),
            ),
            ("service_counts", field_service_counts),
            ("documentation_disabled", field_documentation_disabled),
            (
                "perlless_mechanics_enabled",
                field_perlless_mechanics_enabled,
            ),
            (
                "installer_tools_disabled",
                all_cfgs.clone().call(NixValue::function(|cfg| {
                    cfg.clone()
                        .select_segments(["system", "disableInstallerTools"])
                })),
            ),
            ("nix_cli_capability", field_nix_cli_capability),
            ("stock_runtime_retained", field_stock_runtime_retained),
            (
                "rust_authoring_inputs_available",
                all_cfgs.clone().call(authoring_inputs.clone()),
            ),
            (
                "required_qubes_services_retained",
                field_required_qubes_services_retained,
            ),
            ("config_inputs_import_rpc", field_config_inputs_import_rpc),
        ])
    }
}

/// Share the all-VM predicate and check the two admitted authoring mounts.
fn inspect_authoring_inputs(
    enabled_services: NixValue,
    cfg_list: NixValue,
    cfgs: NixValue,
    has: NixValue,
    lib: NixValue,
) -> NixValue {
    NixValue::function(|pred| lib.clone().select("all").call(pred).call(cfg_list.clone())).bind(
        |all_cfgs| {
            let authoring_inputs = NixValue::function(|cfg| {
                let files = cfg.select("environment.etc");
                files
                    .clone()
                    .has_attr("airlock/authoring/vendor")
                    .and(files.has_attr("airlock/config-inputs/airlock-apps"))
            });

            inspect_services(
                authoring_inputs,
                all_cfgs,
                cfgs.clone(),
                enabled_services.clone(),
                has.clone(),
                lib.clone(),
            )
        },
    )
}

/// Select controller and assistant configurations and share their enabled-service reader.
fn select_configurations(lib: NixValue, flake: NixValue, has: NixValue) -> NixValue {
    NixValue::record([
        (
            "staging",
            flake.clone().select_segments([
                "nixosConfigurations",
                "airlock-domain-sys-ctrl",
                "config",
            ]),
        ),
        (
            "validator",
            flake.clone().select_segments([
                "nixosConfigurations",
                "airlock-domain-usr-assistant",
                "config",
            ]),
        ),
        (
            "vault",
            flake.clone().select_segments([
                "nixosConfigurations",
                "airlock-domain-sys-ctrl",
                "config",
            ]),
        ),
    ])
    .bind(|cfgs| {
        NixValue::builtin("attrValues")
            .call(cfgs.clone())
            .bind(|cfg_list| {
                NixValue::function(|cfg| {
                    lib.clone().select_segments(["attrNames"]).call(
                        lib.clone()
                            .select_segments(["filterAttrs"])
                            .call(NixValue::function(|_name| {
                                NixValue::function(|service| {
                                    service.clone().attr_or("enable", NixValue::from(false))
                                })
                            }))
                            .call(cfg.clone().select_segments(["systemd", "services"])),
                    )
                })
                .bind(|enabled_services| {
                    inspect_authoring_inputs(
                        enabled_services,
                        cfg_list.clone(),
                        cfgs.clone(),
                        has.clone(),
                        lib.clone(),
                    )
                })
            })
    })
}
