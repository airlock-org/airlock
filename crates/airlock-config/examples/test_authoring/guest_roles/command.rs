//! Build the command-role inspection from named deferred inputs without forwarding captures.

use super::*;

mod checks;

/// A shared Nix binding's name and the expression built from preceding bindings.
type Definition = (&'static str, fn(&Bindings) -> NixValue);

/// Bind each expression once in Nix, retaining its original lazy evaluation order.
/// Each definition may reference only earlier names; the result checks see all names.
fn bind_inputs(mut bindings: Bindings, definitions: &[Definition]) -> NixValue {
    let Some(((name, expression), remaining)) = definitions.split_first() else {
        return checks::inspect(bindings);
    };

    expression(&bindings).bind(|value| {
        bindings.values.insert(name, value);
        bind_inputs(bindings, remaining)
    })
}

/// Inspect command-VM integration without evaluating any system during fixture generation.
pub(super) fn command_role(inputs: NixValue) -> NixValue {
    let definitions: &[Definition] = &[
        ("flake", |bindings| {
            let inputs = bindings.get("inputs");

            NixValue::builtin("getFlake").call(NixValue::concat_text([
                NixValue::from("path:"),
                inputs.clone().select("repo_root").to_text(),
            ]))
        }),
        ("default_runtime", |bindings| {
            let inputs = bindings.get("inputs");

            NixValue::builtin("getFlake")
                .call(NixValue::concat_text([
                    NixValue::from("path:"),
                    inputs.clone().select("candidate").to_text(),
                ]))
                .select_segments(["airlock", "topology"])
        }),
        ("package_name", |_bindings| {
            NixValue::function(|package| {
                package
                    .clone()
                    .attr_or("pname", package.clone().attr_or("name", NixValue::from("")))
            })
        }),
        ("has", |_bindings| {
            NixValue::function(|name| {
                NixValue::function(|values| {
                    NixValue::builtin("elem")
                        .call(name.clone())
                        .call(values.clone())
                })
            })
        }),
        ("forbidden_config_rpc", |_bindings| {
            NixValue::list([
                NixValue::from("qubes-rpc/airlock.Config.ArchiveCandidate"),
                NixValue::from("qubes-rpc/airlock.Config.ArchiveDeployment"),
                NixValue::from("qubes-rpc/airlock.Config.CandidateExport"),
                NixValue::from("qubes-rpc/airlock.Config.InputsImport"),
                NixValue::from("qubes-rpc/airlock.Config.ValidateCandidate"),
                NixValue::from("qubes-rpc/airlock.PolicyProbe"),
            ])
        }),
        ("secret_env_names", |_bindings| {
            NixValue::list([
                NixValue::from("CODEX_AUTH_TOKEN"),
                NixValue::from("OPENAI_API_KEY"),
                NixValue::from("OPENAI_API_KEY_FILE"),
            ])
        }),
        ("lib", |bindings| {
            let flake = bindings.get("flake");

            flake.clone().select_segments(["inputs", "nixpkgs", "lib"])
        }),
        ("dom0_cfg", |bindings| {
            let flake = bindings.get("flake");

            flake
                .clone()
                .select_segments(["nixosConfigurations", "vm-gui-config-dev", "config"])
        }),
        ("release_dom0_cfg", |bindings| {
            let flake = bindings.get("flake");

            flake.clone().select_segments([
                "nixosConfigurations",
                "vm-gui-config-release",
                "config",
            ])
        }),
        ("non_dev_dom0_cfg", |bindings| {
            let flake = bindings.get("flake");

            flake
                .clone()
                .select_segments(["nixosConfigurations", "test", "config"])
        }),
        ("builders", |bindings| {
            let inputs = bindings.get("inputs");
            let flake = bindings.get("flake");

            NixValue::builtin("import")
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
                ]))
        }),
        ("sys_net_domain_definition", |bindings| {
            let default_runtime = bindings.get("default_runtime");

            default_runtime
                .clone()
                .select_segments(["domains", "sys-net"])
        }),
        ("sys_firewall_domain_definition", |bindings| {
            let default_runtime = bindings.get("default_runtime");

            default_runtime
                .clone()
                .select_segments(["domains", "sys-firewall"])
        }),
        ("sys_gui_domain_definition", |bindings| {
            let default_runtime = bindings.get("default_runtime");

            default_runtime
                .clone()
                .select_segments(["domains", "sys-gui"])
        }),
        ("sys_gui_cfg", |bindings| {
            let flake = bindings.get("flake");

            flake.clone().select_segments([
                "nixosConfigurations",
                "airlock-domain-sys-gui",
                "config",
            ])
        }),
        ("staged_nix_native_domains", |bindings| {
            let default_runtime = bindings.get("default_runtime");

            default_runtime.clone().select_segments(["domains"])
        }),
        ("dev_entry", |bindings| {
            let builders = bindings.get("builders");

            builders
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
                })
        }),
        ("bootstrap_domains", |bindings| {
            let dom0_cfg = bindings.get("dom0_cfg");

            dom0_cfg
                .clone()
                .select_segments(["airlock", "qubes", "domains"])
        }),
        ("release_bootstrap_domains", |bindings| {
            let release_dom0_cfg = bindings.get("release_dom0_cfg");

            release_dom0_cfg
                .clone()
                .select_segments(["airlock", "qubes", "domains"])
        }),
        ("bootstrap_topology_sys_net_domain", |bindings| {
            let dom0_cfg = bindings.get("dom0_cfg");

            dom0_cfg
                .clone()
                .select_segments(["airlock", "qubes", "topology", "domains", "sys-net"])
        }),
        ("bootstrap_topology_sys_firewall_domain", |bindings| {
            let dom0_cfg = bindings.get("dom0_cfg");

            dom0_cfg.clone().select_segments([
                "airlock",
                "qubes",
                "topology",
                "domains",
                "sys-firewall",
            ])
        }),
        ("base_cfg", |bindings| {
            let builders = bindings.get("builders");

            builders
                .clone()
                .select_segments(["mkNixNativeDomain"])
                .call(NixValue::record([
                    ("name", NixValue::from("minimal-base")),
                    ("guestRole", NixValue::from("base")),
                    ("modules", NixValue::list([])),
                ]))
                .select_segments(["configuration", "config"])
        }),
        ("dom0_qubes_packages", |bindings| {
            let package_name = bindings.get("package_name");
            let dom0_cfg = bindings.get("dom0_cfg");

            NixValue::builtin("map").call(package_name.clone()).call(
                dom0_cfg
                    .clone()
                    .select_segments(["virtualisation", "qubes", "packages"]),
            )
        }),
        ("dom0_sys_gui_policy_package", |bindings| {
            let package_name = bindings.get("package_name");
            let lib = bindings.get("lib");
            let dom0_cfg = bindings.get("dom0_cfg");

            lib.clone()
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
                )
        }),
        ("sys_gui_manager", |bindings| {
            let package_name = bindings.get("package_name");
            let lib = bindings.get("lib");
            let sys_gui_cfg = bindings.get("sys_gui_cfg");

            lib.clone()
                .select_segments(["findFirst"])
                .call(NixValue::function(|package| {
                    package_name
                        .clone()
                        .call(package.clone())
                        .equals(NixValue::from("qubes-manager"))
                }))
                .call(
                    NixValue::builtin("throw")
                        .call(NixValue::from("missing sys-gui Qubes Manager")),
                )
                .call(
                    sys_gui_cfg
                        .clone()
                        .select_segments(["environment", "systemPackages"]),
                )
        }),
        ("etc_names", |bindings| {
            let lib = bindings.get("lib");

            NixValue::function(|cfg| {
                lib.clone()
                    .select_segments(["attrNames"])
                    .call(cfg.clone().select_segments(["environment", "etc"]))
            })
        }),
        ("dev_cfg", |bindings| {
            let dev_entry = bindings.get("dev_entry");

            dev_entry
                .clone()
                .select_segments(["configuration", "config"])
        }),
        ("generated_dev_domain", |bindings| {
            let dev_entry = bindings.get("dev_entry");

            dev_entry.clone().select_segments(["domain"])
        }),
        ("bootstrap_sys_net_domain", |bindings| {
            let bootstrap_domains = bindings.get("bootstrap_domains");

            bootstrap_domains.clone().select_segments(["sys-net"])
        }),
        ("bootstrap_sys_firewall_domain", |bindings| {
            let bootstrap_domains = bindings.get("bootstrap_domains");

            bootstrap_domains.clone().select_segments(["sys-firewall"])
        }),
        ("bootstrap_sys_gui_domain", |bindings| {
            let bootstrap_domains = bindings.get("bootstrap_domains");

            bootstrap_domains.clone().select_segments(["sys-gui"])
        }),
        ("dom0_sys_gui_policy", |bindings| {
            let dom0_sys_gui_policy_package = bindings.get("dom0_sys_gui_policy_package");

            dom0_sys_gui_policy_package.select("text")
        }),
        ("dev_service_script", |bindings| {
            let dev_cfg = bindings.get("dev_cfg");

            dev_cfg.clone().select_segments([
                "systemd",
                "services",
                "airlock-cmd-guest-init",
                "script",
            ])
        }),
    ];

    bind_inputs(Bindings::new([("inputs", inputs)]), definitions)
}
