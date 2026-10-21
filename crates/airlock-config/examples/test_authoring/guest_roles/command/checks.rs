//! Check command-VM isolation, desktop integration and network contracts together.

use super::*;

/// Inspect the shared command-role scenario after its deferred inputs have been bound.
pub(super) fn inspect(captures: Bindings) -> NixValue {
    let dev_service_script = captures.get("dev_service_script");
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
