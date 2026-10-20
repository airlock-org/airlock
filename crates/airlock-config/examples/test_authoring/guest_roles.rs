//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for check cmd role.
pub(super) fn command_role(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([
            NixValue::from("path:"),
            inputs.clone().select("repo_root").to_text(),
        ]))
        .bind(|flake| guest_roles_command_role_23(flake, inputs.clone()))
}

/// Inspect evaluated configuration for check airlock minimal VM.
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
                    .bind(|lib| guest_roles_minimal_vm_3(lib, flake.clone(), has.clone()))
            })
        })
}

/// Build the VM roles check airlock minimal VM contribution using the caller supplied
/// deferred values.
fn guest_roles_minimal_vm_body(
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

/// Build the VM roles check airlock minimal VM contribution using the caller supplied
/// deferred values.
fn guest_roles_minimal_vm_2(
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

            guest_roles_minimal_vm_body(
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

/// Build the VM roles check airlock minimal VM contribution using the caller supplied
/// deferred values.
fn guest_roles_minimal_vm_3(lib: NixValue, flake: NixValue, has: NixValue) -> NixValue {
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
                    guest_roles_minimal_vm_2(
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

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_body(dev_service_script: NixValue, captures: Bindings) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_sys_firewall_domain = captures.get("bootstrap_sys_firewall_domain");
    let bootstrap_sys_gui_domain = captures.get("bootstrap_sys_gui_domain");
    let bootstrap_sys_net_domain = captures.get("bootstrap_sys_net_domain");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_cfg = captures.get("dev_cfg");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let dom0_sys_gui_policy = captures.get("dom0_sys_gui_policy");
    let etc_names = captures.get("etc_names");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let generated_dev_domain = captures.get("generated_dev_domain");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_gui_manager = captures.get("sys_gui_manager");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    {
        let field_sys_gui_rpc_present = lib
            .clone()
            .select_segments(["all"])
            .call(NixValue::function(|name| {
                NixValue::builtin("hasAttr")
                    .call(name.clone())
                    .call(sys_gui_cfg.clone().select_segments(["environment", "etc"]))
            }))
            .call(NixValue::list([
                NixValue::from("qubes-rpc/qubes.GuiVMSession"),
                NixValue::from("qubes-rpc/qubes.SetMonitorLayout"),
                NixValue::from("qubes-rpc/qubes.WindowIconUpdater"),
            ]));

        let field_dom0_sys_gui_session_policy = lib
            .clone()
            .select_segments(["hasInfix"])
            .call(NixValue::from("qubes.GuiVMSession * dom0 sys-gui allow"))
            .call(dom0_sys_gui_policy.clone())
            .and(
                lib.clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::from("qubes.GuiVMSession * @anyvm @anyvm deny"))
                    .call(dom0_sys_gui_policy.clone()),
            );

        let field_dom0_sys_gui_system_domain_visibility = lib
            .clone()
            .select_segments(["all"])
            .call(NixValue::function(|name| {
                lib.clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::concat_text([
                        NixValue::from("admin.vm.List * sys-gui "),
                        name.clone().to_text(),
                        NixValue::from(" allow target=dom0"),
                    ]))
                    .call(dom0_sys_gui_policy.clone())
                    .and(
                        lib.clone()
                            .select_segments(["hasInfix"])
                            .call(NixValue::concat_text([
                                NixValue::from("admin.Events * sys-gui "),
                                name.clone().to_text(),
                                NixValue::from(" allow target=dom0"),
                            ]))
                            .call(dom0_sys_gui_policy.clone()),
                    )
            }))
            .call(
                lib.clone()
                    .select_segments(["filter"])
                    .call(NixValue::function(|name| {
                        lib.clone()
                            .select_segments(["hasPrefix"])
                            .call(NixValue::from("sys-"))
                            .call(name.clone())
                    }))
                    .call(
                        NixValue::builtin("attrNames").call(
                            dom0_cfg
                                .clone()
                                .select_segments(["airlock", "qubes", "topology", "domains"]),
                        ),
                    ),
            );

        let field_dom0_sys_gui_management_policy = lib
            .clone()
            .select_segments(["all"])
            .call(NixValue::function(|rule| {
                lib.clone()
                    .select_segments(["hasInfix"])
                    .call(rule.clone())
                    .call(dom0_sys_gui_policy.clone())
            }))
            .call(NixValue::list([
                NixValue::from("admin.vm.Create.AppVM * sys-gui dom0 allow"),
                NixValue::from("admin.vm.Create.StandaloneVM * sys-gui dom0 allow"),
                NixValue::from("admin.vm.property.Get * sys-gui @anyvm allow target=dom0"),
                NixValue::from("admin.vm.Start * sys-gui @anyvm allow target=dom0"),
                NixValue::from("admin.vm.Remove * sys-gui @anyvm allow target=dom0"),
                NixValue::from("* * sys-gui dom0 deny"),
                NixValue::from("* * sys-gui @anyvm deny"),
            ]))
            .and(
                !lib.clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::from("admin.*"))
                    .call(dom0_sys_gui_policy.clone()),
            );

        let field_dom0_sys_gui_session_present = lib
            .clone()
            .select_segments(["any"])
            .call(NixValue::function(|package| {
                NixValue::builtin("elem")
                    .call(NixValue::from("sys-gui"))
                    .call(
                        package
                            .clone()
                            .attr_or("providedSessions", NixValue::list([])),
                    )
            }))
            .call(dom0_cfg.clone().select_segments([
                "services",
                "displayManager",
                "sessionPackages",
            ]));

        let field_dom0_proxy_http = dom0_cfg
            .clone()
            .attr_or(
                "environment",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or(
                "sessionVariables",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("http_proxy", NixValue::null());

        let field_dom0_proxy_https = dom0_cfg
            .clone()
            .attr_or(
                "environment",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or(
                "sessionVariables",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("https_proxy", NixValue::null());

        let field_dom0_proxy_no_proxy = dom0_cfg
            .clone()
            .attr_or(
                "environment",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or(
                "sessionVariables",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("no_proxy", NixValue::null());

        let field_dom0_nix_daemon_proxy_http = dom0_cfg
            .clone()
            .attr_or("systemd", NixValue::record(Vec::<(&str, NixValue)>::new()))
            .attr_or("services", NixValue::record(Vec::<(&str, NixValue)>::new()))
            .attr_or(
                "nix-daemon",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or(
                "environment",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("http_proxy", NixValue::null());

        let field_release_dom0_proxy_http = release_dom0_cfg
            .clone()
            .attr_or(
                "environment",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or(
                "sessionVariables",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("http_proxy", NixValue::null());

        let field_non_dev_dom0_proxy_http = non_dev_dom0_cfg
            .clone()
            .attr_or(
                "environment",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or(
                "sessionVariables",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("http_proxy", NixValue::null());

        let field_codex_home_env = dev_cfg
            .clone()
            .attr_or(
                "environment",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or(
                "sessionVariables",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("CODEX_HOME", NixValue::null());

        let field_dev_forbidden_rpc_absent = lib
            .clone()
            .select_segments(["all"])
            .call(NixValue::function(|name| {
                !has.clone()
                    .call(name.clone())
                    .call(etc_names.clone().call(dev_cfg.clone()))
            }))
            .call(forbidden_config_rpc.clone());

        let field_dev_no_secret_env = lib
            .clone()
            .select_segments(["all"])
            .call(NixValue::function(|name| {
                !NixValue::builtin("hasAttr").call(name.clone()).call(
                    dev_cfg
                        .clone()
                        .select_segments(["environment", "sessionVariables"]),
                )
            }))
            .call(secret_env_names.clone());

        let field_dev_no_codex_auth_etc = lib
            .clone()
            .select_segments(["all"])
            .call(NixValue::function(|name| {
                (!lib
                    .clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::from("auth.json"))
                    .call(name.clone()))
                .and(
                    !lib.clone()
                        .select_segments(["hasInfix"])
                        .call(NixValue::from("OPENAI_API_KEY"))
                        .call(name.clone()),
                )
                .and(
                    !lib.clone()
                        .select_segments(["hasInfix"])
                        .call(NixValue::from(".codex"))
                        .call(name.clone()),
                )
            }))
            .call(etc_names.clone().call(dev_cfg.clone()));

        let field_base_forbidden_rpc_absent = lib
            .clone()
            .select_segments(["all"])
            .call(NixValue::function(|name| {
                !has.clone()
                    .call(name.clone())
                    .call(etc_names.clone().call(base_cfg.clone()))
            }))
            .call(forbidden_config_rpc.clone());

        NixValue::record([
            (
                "role",
                dev_cfg
                    .clone()
                    .select_segments(["airlock", "qubeGuest", "role"]),
            ),
            (
                "hostname",
                dev_cfg.clone().select_segments(["networking", "hostName"]),
            ),
            (
                "staged_dev_declared",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("usr-cmd"))
                    .call(staged_nix_native_domains.clone()),
            ),
            (
                "staged_sys_net_declared",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("sys-net"))
                    .call(staged_nix_native_domains.clone()),
            ),
            (
                "staged_sys_net_autostart",
                sys_net_domain_definition
                    .clone()
                    .select_segments(["prefs", "autostart"]),
            ),
            (
                "staged_sys_net_virt_mode",
                sys_net_domain_definition
                    .clone()
                    .select_segments(["prefs", "virt_mode"]),
            ),
            (
                "staged_sys_net_updates_proxy",
                sys_net_domain_definition
                    .clone()
                    .select_segments(["services", "qubes-updates-proxy"]),
            ),
            (
                "staged_sys_firewall_declared",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("sys-firewall"))
                    .call(staged_nix_native_domains.clone()),
            ),
            (
                "staged_sys_firewall_netvm",
                sys_firewall_domain_definition
                    .clone()
                    .select_segments(["netvm"]),
            ),
            (
                "staged_sys_firewall_autostart",
                sys_firewall_domain_definition
                    .clone()
                    .select_segments(["prefs", "autostart"]),
            ),
            (
                "staged_sys_firewall_provides_network",
                sys_firewall_domain_definition
                    .clone()
                    .select_segments(["prefs", "provides_network"]),
            ),
            (
                "staged_sys_firewall_servicevm",
                sys_firewall_domain_definition
                    .clone()
                    .select_segments(["features", "servicevm"]),
            ),
            (
                "staged_sys_firewall_updates_proxy",
                sys_firewall_domain_definition
                    .clone()
                    .select_segments(["services", "qubes-updates-proxy"]),
            ),
            (
                "staged_sys_gui_declared",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("sys-gui"))
                    .call(staged_nix_native_domains.clone()),
            ),
            (
                "staged_sys_gui_role",
                sys_gui_domain_definition
                    .clone()
                    .select_segments(["features", "airlock-role"]),
            ),
            (
                "staged_sys_gui_netvm",
                sys_gui_domain_definition.clone().select_segments(["netvm"]),
            ),
            (
                "staged_sys_gui_guivm",
                sys_gui_domain_definition
                    .clone()
                    .select_segments(["prefs", "guivm"]),
            ),
            (
                "staged_sys_gui_autostart",
                sys_gui_domain_definition
                    .clone()
                    .select_segments(["prefs", "autostart"]),
            ),
            (
                "staged_sys_gui_agent_feature",
                sys_gui_domain_definition
                    .clone()
                    .select_segments(["features", "gui"]),
            ),
            (
                "staged_sys_gui_service",
                sys_gui_domain_definition
                    .clone()
                    .select_segments(["services", "guivm"]),
            ),
            (
                "staged_default_guivm",
                default_runtime
                    .clone()
                    .select_segments(["prefs", "default_guivm"]),
            ),
            (
                "staged_default_netvm",
                default_runtime
                    .clone()
                    .select_segments(["prefs", "default_netvm"]),
            ),
            (
                "generated_dev_domain_netvm",
                generated_dev_domain.clone().select_segments(["netvm"]),
            ),
            (
                "generated_dev_domain_role_feature",
                generated_dev_domain
                    .clone()
                    .select_segments(["features", "airlock-role"]),
            ),
            (
                "generated_dev_domain_marker_feature",
                generated_dev_domain
                    .clone()
                    .select_segments(["features", "airlock-cmd"]),
            ),
            (
                "bootstrap_dev_domain_present",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("usr-cmd"))
                    .call(bootstrap_domains.clone()),
            ),
            (
                "bootstrap_sys_net_present",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("sys-net"))
                    .call(bootstrap_domains.clone()),
            ),
            (
                "bootstrap_sys_firewall_present",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("sys-firewall"))
                    .call(bootstrap_domains.clone()),
            ),
            (
                "bootstrap_sys_net_autostart",
                bootstrap_sys_net_domain
                    .clone()
                    .select_segments(["prefs", "autostart"]),
            ),
            (
                "bootstrap_sys_net_netvm",
                bootstrap_sys_net_domain.clone().select_segments(["netvm"]),
            ),
            (
                "bootstrap_sys_net_virt_mode",
                bootstrap_sys_net_domain
                    .clone()
                    .select_segments(["prefs", "virt_mode"]),
            ),
            (
                "bootstrap_sys_net_updates_proxy",
                bootstrap_sys_net_domain
                    .clone()
                    .select_segments(["services", "qubes-updates-proxy"]),
            ),
            (
                "bootstrap_sys_firewall_netvm",
                bootstrap_sys_firewall_domain
                    .clone()
                    .select_segments(["netvm"]),
            ),
            (
                "bootstrap_sys_firewall_autostart",
                bootstrap_sys_firewall_domain
                    .clone()
                    .select_segments(["prefs", "autostart"]),
            ),
            (
                "bootstrap_sys_firewall_provides_network",
                bootstrap_sys_firewall_domain
                    .clone()
                    .select_segments(["prefs", "provides_network"]),
            ),
            (
                "bootstrap_sys_firewall_servicevm",
                bootstrap_sys_firewall_domain
                    .clone()
                    .select_segments(["features", "servicevm"]),
            ),
            (
                "bootstrap_sys_firewall_updates_proxy",
                bootstrap_sys_firewall_domain
                    .clone()
                    .select_segments(["services", "qubes-updates-proxy"]),
            ),
            (
                "bootstrap_sys_gui_present",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("sys-gui"))
                    .call(bootstrap_domains.clone()),
            ),
            (
                "bootstrap_sys_gui_guivm",
                bootstrap_sys_gui_domain
                    .clone()
                    .select_segments(["prefs", "guivm"]),
            ),
            (
                "bootstrap_sys_gui_autostart",
                bootstrap_sys_gui_domain
                    .clone()
                    .select_segments(["prefs", "autostart"]),
            ),
            (
                "bootstrap_sys_gui_agent_feature",
                bootstrap_sys_gui_domain
                    .clone()
                    .select_segments(["features", "gui"]),
            ),
            (
                "bootstrap_default_guivm",
                dom0_cfg
                    .clone()
                    .select_segments(["airlock", "qubes", "prefs", "default_guivm"]),
            ),
            (
                "bootstrap_topology_default_guivm",
                dom0_cfg.clone().select_segments([
                    "airlock",
                    "qubes",
                    "topology",
                    "prefs",
                    "default_guivm",
                ]),
            ),
            (
                "sys_gui_role_enabled",
                sys_gui_cfg
                    .clone()
                    .select_segments(["airlock", "sysGuiGuest", "enable"]),
            ),
            (
                "sys_gui_agent_enabled",
                NixValue::builtin("elem")
                    .call(NixValue::from("multi-user.target"))
                    .call(sys_gui_cfg.clone().select_segments([
                        "systemd",
                        "services",
                        "qubes-gui-agent",
                        "wantedBy",
                    ])),
            ),
            (
                "sys_gui_agent_path",
                sys_gui_cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-gui-agent",
                    "environment",
                    "PATH",
                ]),
            ),
            (
                "sys_gui_pam",
                sys_gui_cfg.clone().select_segments([
                    "security",
                    "pam",
                    "services",
                    "qubes-gui-agent",
                    "text",
                ]),
            ),
            (
                "sys_gui_session_launcher",
                sys_gui_cfg.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes-rpc/qubes.GuiVMSession",
                    "source",
                ]),
            ),
            (
                "sys_gui_login_xserver",
                sys_gui_cfg.clone().select_segments([
                    "services",
                    "xserver",
                    "displayManager",
                    "xserverBin",
                ]),
            ),
            (
                "sys_gui_login_enabled",
                sys_gui_cfg.clone().select_segments([
                    "services",
                    "xserver",
                    "displayManager",
                    "lightdm",
                    "enable",
                ]),
            ),
            (
                "sys_gui_autologin",
                sys_gui_cfg.clone().select_segments([
                    "services",
                    "displayManager",
                    "autoLogin",
                    "enable",
                ]),
            ),
            (
                "sys_gui_login_wanted_by",
                sys_gui_cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "display-manager",
                    "wantedBy",
                ]),
            ),
            (
                "sys_gui_login_password",
                sys_gui_cfg
                    .clone()
                    .select_segments(["users", "users", "user", "initialPassword"]),
            ),
            (
                "dom0_greeter_enabled",
                dom0_cfg.clone().select_segments([
                    "services",
                    "xserver",
                    "displayManager",
                    "lightdm",
                    "greeter",
                    "enable",
                ]),
            ),
            (
                "dom0_autologin",
                dom0_cfg.clone().select_segments([
                    "services",
                    "displayManager",
                    "autoLogin",
                    "enable",
                ]),
            ),
            (
                "sys_gui_template_tools_removed",
                sys_gui_manager
                    .clone()
                    .attr_or("passthru", NixValue::record(Vec::<(&str, NixValue)>::new()))
                    .attr_or("airlockTemplateToolsRemoved", NixValue::from(false)),
            ),
            (
                "dom0_xfce_enabled",
                dom0_cfg.clone().select_segments([
                    "services",
                    "xserver",
                    "desktopManager",
                    "xfce",
                    "enable",
                ]),
            ),
            (
                "dom0_xterm_enabled",
                dom0_cfg.clone().select_segments([
                    "services",
                    "xserver",
                    "desktopManager",
                    "xterm",
                    "enable",
                ]),
            ),
            ("sys_gui_rpc_present", field_sys_gui_rpc_present),
            (
                "dom0_sys_gui_policy_present",
                has.clone()
                    .call(NixValue::from("50-airlock-sys-gui.policy"))
                    .call(dom0_qubes_packages.clone()),
            ),
            (
                "dom0_sys_gui_session_policy",
                field_dom0_sys_gui_session_policy,
            ),
            (
                "dom0_sys_gui_system_domain_visibility",
                field_dom0_sys_gui_system_domain_visibility,
            ),
            (
                "dom0_sys_gui_management_policy",
                field_dom0_sys_gui_management_policy,
            ),
            (
                "dom0_sys_gui_session_present",
                field_dom0_sys_gui_session_present,
            ),
            (
                "dom0_default_session",
                dom0_cfg
                    .clone()
                    .select_segments(["services", "displayManager", "defaultSession"]),
            ),
            (
                "dom0_display_manager_after",
                dom0_cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "display-manager",
                    "after",
                ]),
            ),
            (
                "dom0_bootstrap_status_device",
                dom0_cfg.clone().select_segments([
                    "airlock",
                    "qubes",
                    "setupConfigOnBoot",
                    "statusDevice",
                ]),
            ),
            (
                "dom0_bootstrap_status_environment",
                dom0_cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "airlock-setup-config",
                    "environment",
                    "AIRLOCK_SETUP_CONFIG_STATUS_DEVICE",
                ]),
            ),
            (
                "bootstrap_default_netvm",
                dom0_cfg
                    .clone()
                    .select_segments(["airlock", "qubes", "prefs", "default_netvm"]),
            ),
            (
                "bootstrap_topology_default_netvm",
                dom0_cfg.clone().select_segments([
                    "airlock",
                    "qubes",
                    "topology",
                    "prefs",
                    "default_netvm",
                ]),
            ),
            (
                "bootstrap_dev_topology_present",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("usr-cmd"))
                    .call(
                        dom0_cfg
                            .clone()
                            .select_segments(["airlock", "qubes", "topology", "domains"]),
                    ),
            ),
            (
                "bootstrap_sys_net_topology_autostart",
                bootstrap_topology_sys_net_domain
                    .clone()
                    .select_segments(["prefs", "autostart"]),
            ),
            (
                "bootstrap_sys_firewall_topology_netvm",
                bootstrap_topology_sys_firewall_domain
                    .clone()
                    .select_segments(["netvm"]),
            ),
            (
                "bootstrap_sys_firewall_topology_autostart",
                bootstrap_topology_sys_firewall_domain
                    .clone()
                    .select_segments(["prefs", "autostart"]),
            ),
            (
                "image_dev_mode_enabled",
                dom0_cfg
                    .clone()
                    .select_segments(["airlock", "image", "devMode", "enable"]),
            ),
            (
                "dom0_dhcp_enabled",
                dom0_cfg.clone().select_segments(["networking", "useDHCP"]),
            ),
            (
                "dom0_dhcpcd_no_gateway",
                lib.clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::from("nogateway"))
                    .call(dom0_cfg.clone().select_segments([
                        "networking",
                        "dhcpcd",
                        "extraConfig",
                    ])),
            ),
            (
                "dom0_proxy_enabled",
                dom0_cfg.clone().select_segments([
                    "airlock",
                    "dom0",
                    "devModeNetworkProxy",
                    "enable",
                ]),
            ),
            (
                "dom0_proxy_target",
                dom0_cfg.clone().select_segments([
                    "airlock",
                    "dom0",
                    "devModeNetworkProxy",
                    "target",
                ]),
            ),
            ("dom0_proxy_http", field_dom0_proxy_http),
            ("dom0_proxy_https", field_dom0_proxy_https),
            ("dom0_proxy_no_proxy", field_dom0_proxy_no_proxy),
            (
                "dom0_nix_daemon_proxy_http",
                field_dom0_nix_daemon_proxy_http,
            ),
            (
                "dom0_dev_mode_marker",
                dom0_cfg.clone().select_segments([
                    "environment",
                    "etc",
                    "airlock/dev-mode",
                    "text",
                ]),
            ),
            (
                "dom0_observability_enabled",
                dom0_cfg
                    .clone()
                    .select_segments(["airlock", "dom0", "observability", "enable"]),
            ),
            (
                "dom0_testing_tools_enabled",
                dom0_cfg
                    .clone()
                    .select_segments(["airlock", "dom0", "testingTools", "enable"]),
            ),
            (
                "release_image_dev_mode_enabled",
                release_dom0_cfg
                    .clone()
                    .select_segments(["airlock", "image", "devMode", "enable"]),
            ),
            (
                "release_dom0_dhcp_enabled",
                release_dom0_cfg
                    .clone()
                    .select_segments(["networking", "useDHCP"]),
            ),
            (
                "release_dom0_dhcpcd_no_gateway",
                lib.clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::from("nogateway"))
                    .call(release_dom0_cfg.clone().select_segments([
                        "networking",
                        "dhcpcd",
                        "extraConfig",
                    ])),
            ),
            (
                "release_dom0_proxy_enabled",
                release_dom0_cfg.clone().select_segments([
                    "airlock",
                    "dom0",
                    "devModeNetworkProxy",
                    "enable",
                ]),
            ),
            ("release_dom0_proxy_http", field_release_dom0_proxy_http),
            (
                "release_dom0_dev_marker_absent",
                !NixValue::builtin("hasAttr")
                    .call(NixValue::from("airlock/dev-mode"))
                    .call(
                        release_dom0_cfg
                            .clone()
                            .select_segments(["environment", "etc"]),
                    ),
            ),
            (
                "release_dom0_observability_enabled",
                release_dom0_cfg.clone().select_segments([
                    "airlock",
                    "dom0",
                    "observability",
                    "enable",
                ]),
            ),
            (
                "release_dom0_testing_tools_enabled",
                release_dom0_cfg.clone().select_segments([
                    "airlock",
                    "dom0",
                    "testingTools",
                    "enable",
                ]),
            ),
            (
                "release_bootstrap_dev_domain_present",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("usr-cmd"))
                    .call(release_bootstrap_domains.clone()),
            ),
            (
                "release_bootstrap_sys_net_present",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("sys-net"))
                    .call(release_bootstrap_domains.clone()),
            ),
            (
                "release_bootstrap_sys_net_netvm",
                release_bootstrap_domains
                    .clone()
                    .select_segments(["sys-net", "netvm"]),
            ),
            (
                "release_bootstrap_sys_net_autostart",
                release_bootstrap_domains.clone().select_segments([
                    "sys-net",
                    "prefs",
                    "autostart",
                ]),
            ),
            (
                "release_bootstrap_sys_net_virt_mode",
                release_bootstrap_domains.clone().select_segments([
                    "sys-net",
                    "prefs",
                    "virt_mode",
                ]),
            ),
            (
                "release_bootstrap_sys_firewall_present",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("sys-firewall"))
                    .call(release_bootstrap_domains.clone()),
            ),
            (
                "release_bootstrap_sys_firewall_netvm",
                release_bootstrap_domains
                    .clone()
                    .select_segments(["sys-firewall", "netvm"]),
            ),
            (
                "release_bootstrap_sys_firewall_autostart",
                release_bootstrap_domains.clone().select_segments([
                    "sys-firewall",
                    "prefs",
                    "autostart",
                ]),
            ),
            (
                "non_dev_image_dev_mode_enabled",
                non_dev_dom0_cfg
                    .clone()
                    .select_segments(["airlock", "image", "devMode", "enable"]),
            ),
            (
                "non_dev_dom0_dhcp_enabled",
                non_dev_dom0_cfg
                    .clone()
                    .select_segments(["networking", "useDHCP"]),
            ),
            (
                "non_dev_dom0_dhcpcd_no_gateway",
                lib.clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::from("nogateway"))
                    .call(non_dev_dom0_cfg.clone().select_segments([
                        "networking",
                        "dhcpcd",
                        "extraConfig",
                    ])),
            ),
            (
                "non_dev_dom0_proxy_enabled",
                non_dev_dom0_cfg.clone().select_segments([
                    "airlock",
                    "dom0",
                    "devModeNetworkProxy",
                    "enable",
                ]),
            ),
            ("non_dev_dom0_proxy_http", field_non_dev_dom0_proxy_http),
            (
                "non_dev_dom0_dev_marker_absent",
                !NixValue::builtin("hasAttr")
                    .call(NixValue::from("airlock/dev-mode"))
                    .call(
                        non_dev_dom0_cfg
                            .clone()
                            .select_segments(["environment", "etc"]),
                    ),
            ),
            (
                "non_dev_dom0_observability_enabled",
                non_dev_dom0_cfg.clone().select_segments([
                    "airlock",
                    "dom0",
                    "observability",
                    "enable",
                ]),
            ),
            (
                "non_dev_dom0_testing_tools_enabled",
                non_dev_dom0_cfg.clone().select_segments([
                    "airlock",
                    "dom0",
                    "testingTools",
                    "enable",
                ]),
            ),
            (
                "network_client_resolvconf_disabled",
                !dev_cfg
                    .clone()
                    .select_segments(["networking", "resolvconf", "enable"]),
            ),
            (
                "private_nix_enabled",
                NixValue::builtin("elem")
                    .call(dev_cfg.clone().select_segments(["nix", "package"]))
                    .call(
                        dev_cfg
                            .clone()
                            .select_segments(["environment", "systemPackages"]),
                    ),
            ),
            (
                "nix_features",
                dev_cfg
                    .clone()
                    .select_segments(["nix", "settings", "experimental-features"]),
            ),
            (
                "nix_allowed_users",
                dev_cfg
                    .clone()
                    .select_segments(["nix", "settings", "allowed-users"]),
            ),
            (
                "codex_home",
                dev_cfg
                    .clone()
                    .select_segments(["airlock", "qubeGuest", "codexHome"]),
            ),
            ("codex_home_env", field_codex_home_env),
            (
                "dev_persistent_paths",
                dev_cfg
                    .clone()
                    .select_segments(["airlock", "qubeGuest", "cmdPersistentPaths"]),
            ),
            (
                "dev_init_contains_dev_path",
                lib.clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::from("/home/user/dev"))
                    .call(dev_service_script.clone()),
            ),
            (
                "dev_init_contains_codex_home",
                lib.clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::from("/home/user/.codex"))
                    .call(dev_service_script.clone()),
            ),
            (
                "dev_init_contains_private_nix_root",
                lib.clone()
                    .select_segments(["hasInfix"])
                    .call(NixValue::from("/rw/home/user/airlock-config-nix-root"))
                    .call(dev_service_script.clone()),
            ),
            (
                "dev_qrexec_requires",
                dev_cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-qrexec-agent",
                    "requires",
                ]),
            ),
            (
                "dev_qrexec_after",
                dev_cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-qrexec-agent",
                    "after",
                ]),
            ),
            ("dev_forbidden_rpc_absent", field_dev_forbidden_rpc_absent),
            (
                "retired_config_policy_enabled",
                dom0_cfg.clone().select_segments([
                    "airlock",
                    "qubes",
                    "configAccessPolicy",
                    "enable",
                ]),
            ),
            ("dev_no_secret_env", field_dev_no_secret_env),
            ("dev_no_codex_auth_etc", field_dev_no_codex_auth_etc),
            (
                "base_role",
                base_cfg
                    .clone()
                    .select_segments(["airlock", "qubeGuest", "role"]),
            ),
            (
                "base_private_workspace_absent",
                // The ordinary NixOS CLI does not establish a private builder workspace.
                !base_cfg
                    .clone()
                    .select("systemd.services")
                    .has_attr("airlock-cmd-guest-init")
                    .or(base_cfg
                        .clone()
                        .select("systemd.services")
                        .has_attr("airlock-usr-assistant-workspace")),
            ),
            ("base_forbidden_rpc_absent", field_base_forbidden_rpc_absent),
        ])
    }
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_2(dom0_sys_gui_policy: NixValue, captures: Bindings) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_sys_firewall_domain = captures.get("bootstrap_sys_firewall_domain");
    let bootstrap_sys_gui_domain = captures.get("bootstrap_sys_gui_domain");
    let bootstrap_sys_net_domain = captures.get("bootstrap_sys_net_domain");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_cfg = captures.get("dev_cfg");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let etc_names = captures.get("etc_names");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let generated_dev_domain = captures.get("generated_dev_domain");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_gui_manager = captures.get("sys_gui_manager");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    dev_cfg
        .clone()
        .select_segments(["systemd", "services", "airlock-cmd-guest-init", "script"])
        .bind(|dev_service_script| {
            guest_roles_command_role_body(
                dev_service_script,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_sys_firewall_domain",
                        bootstrap_sys_firewall_domain.clone(),
                    ),
                    ("bootstrap_sys_gui_domain", bootstrap_sys_gui_domain.clone()),
                    ("bootstrap_sys_net_domain", bootstrap_sys_net_domain.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_cfg", dev_cfg.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                    ("dom0_sys_gui_policy", dom0_sys_gui_policy.clone()),
                    ("etc_names", etc_names.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("generated_dev_domain", generated_dev_domain.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    ("sys_gui_manager", sys_gui_manager.clone()),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_3(bootstrap_sys_gui_domain: NixValue, captures: Bindings) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_sys_firewall_domain = captures.get("bootstrap_sys_firewall_domain");
    let bootstrap_sys_net_domain = captures.get("bootstrap_sys_net_domain");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_cfg = captures.get("dev_cfg");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let dom0_sys_gui_policy_package = captures.get("dom0_sys_gui_policy_package");
    let etc_names = captures.get("etc_names");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let generated_dev_domain = captures.get("generated_dev_domain");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_gui_manager = captures.get("sys_gui_manager");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    dom0_sys_gui_policy_package
        .select("text")
        .bind(|dom0_sys_gui_policy| {
            guest_roles_command_role_2(
                dom0_sys_gui_policy,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_sys_firewall_domain",
                        bootstrap_sys_firewall_domain.clone(),
                    ),
                    ("bootstrap_sys_gui_domain", bootstrap_sys_gui_domain.clone()),
                    ("bootstrap_sys_net_domain", bootstrap_sys_net_domain.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_cfg", dev_cfg.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                    ("etc_names", etc_names.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("generated_dev_domain", generated_dev_domain.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    ("sys_gui_manager", sys_gui_manager.clone()),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_4(
    bootstrap_sys_firewall_domain: NixValue,
    captures: Bindings,
) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_sys_net_domain = captures.get("bootstrap_sys_net_domain");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_cfg = captures.get("dev_cfg");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let dom0_sys_gui_policy_package = captures.get("dom0_sys_gui_policy_package");
    let etc_names = captures.get("etc_names");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let generated_dev_domain = captures.get("generated_dev_domain");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_gui_manager = captures.get("sys_gui_manager");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    bootstrap_domains
        .clone()
        .select_segments(["sys-gui"])
        .bind(|bootstrap_sys_gui_domain| {
            guest_roles_command_role_3(
                bootstrap_sys_gui_domain,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_sys_firewall_domain",
                        bootstrap_sys_firewall_domain.clone(),
                    ),
                    ("bootstrap_sys_net_domain", bootstrap_sys_net_domain.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_cfg", dev_cfg.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                    (
                        "dom0_sys_gui_policy_package",
                        dom0_sys_gui_policy_package.clone(),
                    ),
                    ("etc_names", etc_names.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("generated_dev_domain", generated_dev_domain.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    ("sys_gui_manager", sys_gui_manager.clone()),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_5(bootstrap_sys_net_domain: NixValue, captures: Bindings) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_cfg = captures.get("dev_cfg");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let dom0_sys_gui_policy_package = captures.get("dom0_sys_gui_policy_package");
    let etc_names = captures.get("etc_names");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let generated_dev_domain = captures.get("generated_dev_domain");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_gui_manager = captures.get("sys_gui_manager");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    bootstrap_domains
        .clone()
        .select_segments(["sys-firewall"])
        .bind(|bootstrap_sys_firewall_domain| {
            guest_roles_command_role_4(
                bootstrap_sys_firewall_domain,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    ("bootstrap_sys_net_domain", bootstrap_sys_net_domain.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_cfg", dev_cfg.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                    (
                        "dom0_sys_gui_policy_package",
                        dom0_sys_gui_policy_package.clone(),
                    ),
                    ("etc_names", etc_names.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("generated_dev_domain", generated_dev_domain.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    ("sys_gui_manager", sys_gui_manager.clone()),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_6(generated_dev_domain: NixValue, captures: Bindings) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_cfg = captures.get("dev_cfg");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let dom0_sys_gui_policy_package = captures.get("dom0_sys_gui_policy_package");
    let etc_names = captures.get("etc_names");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_gui_manager = captures.get("sys_gui_manager");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    bootstrap_domains
        .clone()
        .select_segments(["sys-net"])
        .bind(|bootstrap_sys_net_domain| {
            guest_roles_command_role_5(
                bootstrap_sys_net_domain,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_cfg", dev_cfg.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                    (
                        "dom0_sys_gui_policy_package",
                        dom0_sys_gui_policy_package.clone(),
                    ),
                    ("etc_names", etc_names.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("generated_dev_domain", generated_dev_domain.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    ("sys_gui_manager", sys_gui_manager.clone()),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_7(dev_cfg: NixValue, captures: Bindings) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_entry = captures.get("dev_entry");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let dom0_sys_gui_policy_package = captures.get("dom0_sys_gui_policy_package");
    let etc_names = captures.get("etc_names");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_gui_manager = captures.get("sys_gui_manager");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    dev_entry
        .clone()
        .select_segments(["domain"])
        .bind(|generated_dev_domain| {
            guest_roles_command_role_6(
                generated_dev_domain,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_cfg", dev_cfg.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                    (
                        "dom0_sys_gui_policy_package",
                        dom0_sys_gui_policy_package.clone(),
                    ),
                    ("etc_names", etc_names.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    ("sys_gui_manager", sys_gui_manager.clone()),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_8(etc_names: NixValue, captures: Bindings) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_entry = captures.get("dev_entry");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let dom0_sys_gui_policy_package = captures.get("dom0_sys_gui_policy_package");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_gui_manager = captures.get("sys_gui_manager");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    dev_entry
        .clone()
        .select_segments(["configuration", "config"])
        .bind(|dev_cfg| {
            guest_roles_command_role_7(
                dev_cfg,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_entry", dev_entry.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                    (
                        "dom0_sys_gui_policy_package",
                        dom0_sys_gui_policy_package.clone(),
                    ),
                    ("etc_names", etc_names.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    ("sys_gui_manager", sys_gui_manager.clone()),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_9(sys_gui_manager: NixValue, captures: Bindings) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_entry = captures.get("dev_entry");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let dom0_sys_gui_policy_package = captures.get("dom0_sys_gui_policy_package");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    NixValue::function(|cfg| {
        lib.clone()
            .select_segments(["attrNames"])
            .call(cfg.clone().select_segments(["environment", "etc"]))
    })
    .bind(|etc_names| {
        guest_roles_command_role_8(
            etc_names,
            Bindings::new([
                ("base_cfg", base_cfg.clone()),
                ("bootstrap_domains", bootstrap_domains.clone()),
                (
                    "bootstrap_topology_sys_firewall_domain",
                    bootstrap_topology_sys_firewall_domain.clone(),
                ),
                (
                    "bootstrap_topology_sys_net_domain",
                    bootstrap_topology_sys_net_domain.clone(),
                ),
                ("default_runtime", default_runtime.clone()),
                ("dev_entry", dev_entry.clone()),
                ("dom0_cfg", dom0_cfg.clone()),
                ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                (
                    "dom0_sys_gui_policy_package",
                    dom0_sys_gui_policy_package.clone(),
                ),
                ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                ("has", has.clone()),
                ("lib", lib.clone()),
                ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                (
                    "release_bootstrap_domains",
                    release_bootstrap_domains.clone(),
                ),
                ("release_dom0_cfg", release_dom0_cfg.clone()),
                ("secret_env_names", secret_env_names.clone()),
                (
                    "staged_nix_native_domains",
                    staged_nix_native_domains.clone(),
                ),
                (
                    "sys_firewall_domain_definition",
                    sys_firewall_domain_definition.clone(),
                ),
                ("sys_gui_cfg", sys_gui_cfg.clone()),
                (
                    "sys_gui_domain_definition",
                    sys_gui_domain_definition.clone(),
                ),
                ("sys_gui_manager", sys_gui_manager.clone()),
                (
                    "sys_net_domain_definition",
                    sys_net_domain_definition.clone(),
                ),
            ]),
        )
    })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_10(
    dom0_sys_gui_policy_package: NixValue,
    captures: Bindings,
) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_entry = captures.get("dev_entry");
    let dom0_cfg = captures.get("dom0_cfg");
    let dom0_qubes_packages = captures.get("dom0_qubes_packages");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    {
        let receiver = lib
            .clone()
            .select_segments(["findFirst"])
            .call(NixValue::function(|package| {
                package_name
                    .clone()
                    .call(package.clone())
                    .equals(NixValue::from("qubes-manager"))
            }))
            .call(NixValue::builtin("throw").call(NixValue::from("missing sys-gui Qubes Manager")))
            .call(
                sys_gui_cfg
                    .clone()
                    .select_segments(["environment", "systemPackages"]),
            );

        receiver.bind(|sys_gui_manager| {
            guest_roles_command_role_9(
                sys_gui_manager,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_entry", dev_entry.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                    (
                        "dom0_sys_gui_policy_package",
                        dom0_sys_gui_policy_package.clone(),
                    ),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
    }
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_11(dom0_qubes_packages: NixValue, captures: Bindings) -> NixValue {
    let base_cfg = captures.get("base_cfg");
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_entry = captures.get("dev_entry");
    let dom0_cfg = captures.get("dom0_cfg");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    {
        let receiver = lib
            .clone()
            .select_segments(["findFirst"])
            .call(NixValue::function(|package| {
                package_name
                    .clone()
                    .call(package.clone())
                    .equals(NixValue::from("50-airlock-sys-gui.policy"))
            }))
            .call(
                NixValue::builtin("throw")
                    .call(NixValue::from("missing Airlock sys-gui policy package")),
            )
            .call(
                dom0_cfg
                    .clone()
                    .select_segments(["virtualisation", "qubes", "packages"]),
            );

        receiver.bind(|dom0_sys_gui_policy_package| {
            guest_roles_command_role_10(
                dom0_sys_gui_policy_package,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_entry", dev_entry.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("dom0_qubes_packages", dom0_qubes_packages.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    ("package_name", package_name.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
    }
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_12(base_cfg: NixValue, captures: Bindings) -> NixValue {
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_topology_sys_firewall_domain =
        captures.get("bootstrap_topology_sys_firewall_domain");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let default_runtime = captures.get("default_runtime");
    let dev_entry = captures.get("dev_entry");
    let dom0_cfg = captures.get("dom0_cfg");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    NixValue::builtin("map")
        .call(package_name.clone())
        .call(
            dom0_cfg
                .clone()
                .select_segments(["virtualisation", "qubes", "packages"]),
        )
        .bind(|dom0_qubes_packages| {
            guest_roles_command_role_11(
                dom0_qubes_packages,
                Bindings::new([
                    ("base_cfg", base_cfg.clone()),
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_entry", dev_entry.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    ("package_name", package_name.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_13(
    bootstrap_topology_sys_firewall_domain: NixValue,
    captures: Bindings,
) -> NixValue {
    let bootstrap_domains = captures.get("bootstrap_domains");
    let bootstrap_topology_sys_net_domain = captures.get("bootstrap_topology_sys_net_domain");
    let builders = captures.get("builders");
    let default_runtime = captures.get("default_runtime");
    let dev_entry = captures.get("dev_entry");
    let dom0_cfg = captures.get("dom0_cfg");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    builders
        .clone()
        .select_segments(["mkNixNativeDomain"])
        .call(NixValue::record([
            ("name", NixValue::from("minimal-base")),
            ("guestRole", NixValue::from("base")),
            ("modules", NixValue::list([])),
        ]))
        .select_segments(["configuration", "config"])
        .bind(|base_cfg| {
            guest_roles_command_role_12(
                base_cfg,
                Bindings::new([
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_topology_sys_firewall_domain",
                        bootstrap_topology_sys_firewall_domain.clone(),
                    ),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_entry", dev_entry.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    ("package_name", package_name.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_14(
    bootstrap_topology_sys_net_domain: NixValue,
    captures: Bindings,
) -> NixValue {
    let bootstrap_domains = captures.get("bootstrap_domains");
    let builders = captures.get("builders");
    let default_runtime = captures.get("default_runtime");
    let dev_entry = captures.get("dev_entry");
    let dom0_cfg = captures.get("dom0_cfg");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_bootstrap_domains = captures.get("release_bootstrap_domains");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    dom0_cfg
        .clone()
        .select_segments(["airlock", "qubes", "topology", "domains", "sys-firewall"])
        .bind(|bootstrap_topology_sys_firewall_domain| {
            guest_roles_command_role_13(
                bootstrap_topology_sys_firewall_domain,
                Bindings::new([
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    (
                        "bootstrap_topology_sys_net_domain",
                        bootstrap_topology_sys_net_domain.clone(),
                    ),
                    ("builders", builders.clone()),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_entry", dev_entry.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    ("package_name", package_name.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_15(
    release_bootstrap_domains: NixValue,
    captures: Bindings,
) -> NixValue {
    let bootstrap_domains = captures.get("bootstrap_domains");
    let builders = captures.get("builders");
    let default_runtime = captures.get("default_runtime");
    let dev_entry = captures.get("dev_entry");
    let dom0_cfg = captures.get("dom0_cfg");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    dom0_cfg
        .clone()
        .select_segments(["airlock", "qubes", "topology", "domains", "sys-net"])
        .bind(|bootstrap_topology_sys_net_domain| {
            guest_roles_command_role_14(
                bootstrap_topology_sys_net_domain,
                Bindings::new([
                    ("bootstrap_domains", bootstrap_domains.clone()),
                    ("builders", builders.clone()),
                    ("default_runtime", default_runtime.clone()),
                    ("dev_entry", dev_entry.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    ("package_name", package_name.clone()),
                    (
                        "release_bootstrap_domains",
                        release_bootstrap_domains.clone(),
                    ),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_16(dev_entry: NixValue, captures: Bindings) -> NixValue {
    let builders = captures.get("builders");
    let default_runtime = captures.get("default_runtime");
    let dom0_cfg = captures.get("dom0_cfg");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let staged_nix_native_domains = captures.get("staged_nix_native_domains");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    dom0_cfg
        .clone()
        .select_segments(["airlock", "qubes", "domains"])
        .bind(|bootstrap_domains| {
            release_dom0_cfg
                .clone()
                .select_segments(["airlock", "qubes", "domains"])
                .bind(|release_bootstrap_domains| {
                    guest_roles_command_role_15(
                        release_bootstrap_domains,
                        Bindings::new([
                            ("bootstrap_domains", bootstrap_domains.clone()),
                            ("builders", builders.clone()),
                            ("default_runtime", default_runtime.clone()),
                            ("dev_entry", dev_entry.clone()),
                            ("dom0_cfg", dom0_cfg.clone()),
                            ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                            ("has", has.clone()),
                            ("lib", lib.clone()),
                            ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                            ("package_name", package_name.clone()),
                            ("release_dom0_cfg", release_dom0_cfg.clone()),
                            ("secret_env_names", secret_env_names.clone()),
                            (
                                "staged_nix_native_domains",
                                staged_nix_native_domains.clone(),
                            ),
                            (
                                "sys_firewall_domain_definition",
                                sys_firewall_domain_definition.clone(),
                            ),
                            ("sys_gui_cfg", sys_gui_cfg.clone()),
                            (
                                "sys_gui_domain_definition",
                                sys_gui_domain_definition.clone(),
                            ),
                            (
                                "sys_net_domain_definition",
                                sys_net_domain_definition.clone(),
                            ),
                        ]),
                    )
                })
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_17(
    staged_nix_native_domains: NixValue,
    captures: Bindings,
) -> NixValue {
    let builders = captures.get("builders");
    let default_runtime = captures.get("default_runtime");
    let dom0_cfg = captures.get("dom0_cfg");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_gui_cfg = captures.get("sys_gui_cfg");
    let sys_gui_domain_definition = captures.get("sys_gui_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    {
        let receiver = builders
            .clone()
            .select_segments(["mkNixNativeDomain"])
            .call({
                let field_modules = NixValue::list([NixValue::function_attrs_open(
                    std::iter::empty::<&str>(),
                    |_arguments_1| {
                        (
                            Vec::<(&str, NixValue)>::new(),
                            field(["networking", "hostName"], NixValue::from("sample-cmd")),
                        )
                    },
                )]);

                NixValue::record([
                    ("name", NixValue::from("sample-cmd")),
                    ("guestRole", NixValue::from("cmd")),
                    ("modules", field_modules),
                    (
                        "features",
                        NixValue::record([
                            ("airlock-role", NixValue::from("cmd")),
                            ("airlock-cmd", NixValue::from("1")),
                        ]),
                    ),
                ])
            });

        receiver.bind(|dev_entry| {
            guest_roles_command_role_16(
                dev_entry,
                Bindings::new([
                    ("builders", builders.clone()),
                    ("default_runtime", default_runtime.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    ("package_name", package_name.clone()),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                    (
                        "staged_nix_native_domains",
                        staged_nix_native_domains.clone(),
                    ),
                    (
                        "sys_firewall_domain_definition",
                        sys_firewall_domain_definition.clone(),
                    ),
                    ("sys_gui_cfg", sys_gui_cfg.clone()),
                    (
                        "sys_gui_domain_definition",
                        sys_gui_domain_definition.clone(),
                    ),
                    (
                        "sys_net_domain_definition",
                        sys_net_domain_definition.clone(),
                    ),
                ]),
            )
        })
    }
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_18(
    sys_gui_domain_definition: NixValue,
    captures: Bindings,
) -> NixValue {
    let builders = captures.get("builders");
    let default_runtime = captures.get("default_runtime");
    let dom0_cfg = captures.get("dom0_cfg");
    let flake = captures.get("flake");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");
    let sys_firewall_domain_definition = captures.get("sys_firewall_domain_definition");
    let sys_net_domain_definition = captures.get("sys_net_domain_definition");

    flake
        .clone()
        .select_segments(["nixosConfigurations", "airlock-domain-sys-gui", "config"])
        .bind(|sys_gui_cfg| {
            default_runtime
                .clone()
                .select_segments(["domains"])
                .bind(|staged_nix_native_domains| {
                    guest_roles_command_role_17(
                        staged_nix_native_domains,
                        Bindings::new([
                            ("builders", builders.clone()),
                            ("default_runtime", default_runtime.clone()),
                            ("dom0_cfg", dom0_cfg.clone()),
                            ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                            ("has", has.clone()),
                            ("lib", lib.clone()),
                            ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                            ("package_name", package_name.clone()),
                            ("release_dom0_cfg", release_dom0_cfg.clone()),
                            ("secret_env_names", secret_env_names.clone()),
                            (
                                "sys_firewall_domain_definition",
                                sys_firewall_domain_definition.clone(),
                            ),
                            ("sys_gui_cfg", sys_gui_cfg.clone()),
                            (
                                "sys_gui_domain_definition",
                                sys_gui_domain_definition.clone(),
                            ),
                            (
                                "sys_net_domain_definition",
                                sys_net_domain_definition.clone(),
                            ),
                        ]),
                    )
                })
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_19(builders: NixValue, captures: Bindings) -> NixValue {
    let default_runtime = captures.get("default_runtime");
    let dom0_cfg = captures.get("dom0_cfg");
    let flake = captures.get("flake");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let lib = captures.get("lib");
    let non_dev_dom0_cfg = captures.get("non_dev_dom0_cfg");
    let package_name = captures.get("package_name");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");

    default_runtime
        .clone()
        .select_segments(["domains", "sys-net"])
        .bind(|sys_net_domain_definition| {
            default_runtime
                .clone()
                .select_segments(["domains", "sys-firewall"])
                .bind(|sys_firewall_domain_definition| {
                    default_runtime
                        .clone()
                        .select_segments(["domains", "sys-gui"])
                        .bind(|sys_gui_domain_definition| {
                            guest_roles_command_role_18(
                                sys_gui_domain_definition,
                                Bindings::new([
                                    ("builders", builders.clone()),
                                    ("default_runtime", default_runtime.clone()),
                                    ("dom0_cfg", dom0_cfg.clone()),
                                    ("flake", flake.clone()),
                                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                                    ("has", has.clone()),
                                    ("lib", lib.clone()),
                                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                                    ("package_name", package_name.clone()),
                                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                                    ("secret_env_names", secret_env_names.clone()),
                                    (
                                        "sys_firewall_domain_definition",
                                        sys_firewall_domain_definition.clone(),
                                    ),
                                    (
                                        "sys_net_domain_definition",
                                        sys_net_domain_definition.clone(),
                                    ),
                                ]),
                            )
                        })
                })
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_20(non_dev_dom0_cfg: NixValue, captures: Bindings) -> NixValue {
    let default_runtime = captures.get("default_runtime");
    let dom0_cfg = captures.get("dom0_cfg");
    let flake = captures.get("flake");
    let forbidden_config_rpc = captures.get("forbidden_config_rpc");
    let has = captures.get("has");
    let inputs = captures.get("inputs");
    let lib = captures.get("lib");
    let package_name = captures.get("package_name");
    let release_dom0_cfg = captures.get("release_dom0_cfg");
    let secret_env_names = captures.get("secret_env_names");

    {
        let receiver = NixValue::builtin("import")
            .call(NixValue::concat_text([
                inputs.clone().select("apps").to_text(),
                NixValue::from("/airlock-config/generated/vm-builders.nix"),
            ]))
            .select_segments(["builders"])
            .call(NixValue::record([
                (
                    "nixpkgs",
                    flake.clone().select_segments(["inputs", "nixpkgs"]),
                ),
                (
                    "nixpkgsLibvirt105Source",
                    NixValue::concat_text([flake
                        .clone()
                        .select_segments(["inputs", "nixpkgs-libvirt_10_5"])
                        .to_text()]),
                ),
                (
                    "nixosQubesSource",
                    NixValue::concat_text([flake
                        .clone()
                        .select_segments(["inputs", "nixos-qubes"])
                        .to_text()]),
                ),
                (
                    "airlockSupport",
                    NixValue::concat_text([inputs.clone().select("repo_root2").to_text()]),
                ),
                (
                    "airlockAppsSource",
                    NixValue::concat_text([inputs.clone().select("apps").to_text()]),
                ),
                ("system", NixValue::from("x86_64-linux")),
            ]));

        receiver.bind(|builders| {
            guest_roles_command_role_19(
                builders,
                Bindings::new([
                    ("default_runtime", default_runtime.clone()),
                    ("dom0_cfg", dom0_cfg.clone()),
                    ("flake", flake.clone()),
                    ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                    ("has", has.clone()),
                    ("lib", lib.clone()),
                    ("non_dev_dom0_cfg", non_dev_dom0_cfg.clone()),
                    ("package_name", package_name.clone()),
                    ("release_dom0_cfg", release_dom0_cfg.clone()),
                    ("secret_env_names", secret_env_names.clone()),
                ]),
            )
        })
    }
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_21(
    secret_env_names: NixValue,
    default_runtime: NixValue,
    flake: NixValue,
    forbidden_config_rpc: NixValue,
    has: NixValue,
    inputs: NixValue,
    package_name: NixValue,
) -> NixValue {
    flake
        .clone()
        .select_segments(["inputs", "nixpkgs", "lib"])
        .bind(|lib| {
            flake
                .clone()
                .select_segments(["nixosConfigurations", "vm-gui-config-dev", "config"])
                .bind(|dom0_cfg| {
                    flake
                        .clone()
                        .select_segments(["nixosConfigurations", "vm-gui-config-release", "config"])
                        .bind(|release_dom0_cfg| {
                            flake
                                .clone()
                                .select_segments(["nixosConfigurations", "test", "config"])
                                .bind(|non_dev_dom0_cfg| {
                                    guest_roles_command_role_20(
                                        non_dev_dom0_cfg,
                                        Bindings::new([
                                            ("default_runtime", default_runtime.clone()),
                                            ("dom0_cfg", dom0_cfg.clone()),
                                            ("flake", flake.clone()),
                                            ("forbidden_config_rpc", forbidden_config_rpc.clone()),
                                            ("has", has.clone()),
                                            ("inputs", inputs.clone()),
                                            ("lib", lib.clone()),
                                            ("package_name", package_name.clone()),
                                            ("release_dom0_cfg", release_dom0_cfg.clone()),
                                            ("secret_env_names", secret_env_names.clone()),
                                        ]),
                                    )
                                })
                        })
                })
        })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_22(
    has: NixValue,
    default_runtime: NixValue,
    flake: NixValue,
    inputs: NixValue,
    package_name: NixValue,
) -> NixValue {
    NixValue::list([
        NixValue::from("qubes-rpc/airlock.Config.ArchiveCandidate"),
        NixValue::from("qubes-rpc/airlock.Config.ArchiveDeployment"),
        NixValue::from("qubes-rpc/airlock.Config.CandidateExport"),
        NixValue::from("qubes-rpc/airlock.Config.InputsImport"),
        NixValue::from("qubes-rpc/airlock.Config.ValidateCandidate"),
        NixValue::from("qubes-rpc/airlock.PolicyProbe"),
    ])
    .bind(|forbidden_config_rpc| {
        NixValue::list([
            NixValue::from("CODEX_AUTH_TOKEN"),
            NixValue::from("OPENAI_API_KEY"),
            NixValue::from("OPENAI_API_KEY_FILE"),
        ])
        .bind(|secret_env_names| {
            guest_roles_command_role_21(
                secret_env_names,
                default_runtime.clone(),
                flake.clone(),
                forbidden_config_rpc.clone(),
                has.clone(),
                inputs.clone(),
                package_name.clone(),
            )
        })
    })
}

/// Build the VM roles check cmd role contribution using the caller supplied deferred values.
fn guest_roles_command_role_23(flake: NixValue, inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([
            NixValue::from("path:"),
            inputs.clone().select("candidate").to_text(),
        ]))
        .select_segments(["airlock", "topology"])
        .bind(|default_runtime| {
            NixValue::function(|package| {
                package
                    .clone()
                    .attr_or("pname", package.clone().attr_or("name", NixValue::from("")))
            })
            .bind(|package_name| {
                NixValue::function(|name| {
                    NixValue::function(|values| {
                        NixValue::builtin("elem")
                            .call(name.clone())
                            .call(values.clone())
                    })
                })
                .bind(|has| {
                    guest_roles_command_role_22(
                        has,
                        default_runtime.clone(),
                        flake.clone(),
                        inputs.clone(),
                        package_name.clone(),
                    )
                })
            })
        })
}
