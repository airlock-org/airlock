//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test usr home integrations do not start another desktop or
/// installer.
pub(super) fn personal_integrations(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(|f| {
            f.clone()
                .select_segments(["nixosConfigurations", "airlock-domain-usr-home", "config"])
                .bind(usr_home_gnome_personal_integrations_body)
        })
}

/// Inspect evaluated configuration for test personal domains explicitly select the gui vm.
pub(super) fn personal_gui_selection(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(|f| {
            NixValue::if_else(
                inputs.clone().select("profile").equals("usb-lab"),
                f.clone().select_segments([
                    "nixosConfigurations",
                    "machine",
                    "config",
                    "specialisation",
                    "usb-lab",
                    "configuration",
                ]),
                f.select_segments(["nixosConfigurations", "vm-gui-config-dev", "config"]),
            )
            .select_segments(["airlock", "qubes", "topology"])
            .bind(|topology| {
                NixValue::builtin("getFlake")
                    .call(NixValue::concat_text([
                        NixValue::from("path:"),
                        inputs.clone().select("candidate").to_text(),
                    ]))
                    .select_segments(["airlock", "topology"])
                    .bind(|runtime| {
                        usr_home_gnome_personal_gui_selection_body(runtime, topology.clone())
                    })
            })
        })
}

/// Inspect evaluated configuration for test gnome desktop work applications are routed to usr home.
pub(super) fn application_routing(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(usr_home_gnome_application_routing_3)
}

/// Build the usr home gnome test personal domains explicitly select the gui vm contribution using
/// the caller supplied deferred values.
fn usr_home_gnome_personal_gui_selection_body(runtime: NixValue, topology: NixValue) -> NixValue {
    {
        let field_personal_gui_v_ms = NixValue::builtin("listToAttrs").call(
            NixValue::builtin("map")
                .call(NixValue::function(|name| {
                    NixValue::record([
                        ("name", name.clone()),
                        (
                            "value",
                            NixValue::builtin("getAttr")
                                .call(name.clone())
                                .call(topology.clone().attr_or(
                                    "domains",
                                    NixValue::record(Vec::<(&str, NixValue)>::new()),
                                ))
                                .attr_or("prefs", NixValue::record(Vec::<(&str, NixValue)>::new()))
                                .attr_or("guivm", NixValue::null()),
                        ),
                    ])
                }))
                .call(NixValue::list([NixValue::from("usr-home")])),
        );

        let field_runtime_gui_v_ms = NixValue::builtin("listToAttrs").call(
            NixValue::builtin("map")
                .call(NixValue::function(|name| {
                    NixValue::record([
                        ("name", name.clone()),
                        (
                            "value",
                            NixValue::builtin("getAttr")
                                .call(name.clone())
                                .call(runtime.clone().attr_or(
                                    "domains",
                                    NixValue::record(Vec::<(&str, NixValue)>::new()),
                                ))
                                .attr_or("prefs", NixValue::record(Vec::<(&str, NixValue)>::new()))
                                .attr_or("guivm", NixValue::null()),
                        ),
                    ])
                }))
                .call(NixValue::list([NixValue::from("usr-home")])),
        );

        NixValue::record([
            (
                "defaultGuiVM",
                topology.clone().select_segments(["prefs", "default_guivm"]),
            ),
            ("personalGuiVMs", field_personal_gui_v_ms),
            ("runtimeGuiVMs", field_runtime_gui_v_ms),
        ])
    }
}

/// Build the usr home gnome test usr home integrations do not start another desktop or installer
/// contribution using the caller supplied deferred values.
fn usr_home_gnome_personal_integrations_body(home: NixValue) -> NixValue {
    {
        let field_installation = NixValue::record([
            ("nix", home.clone().select_segments(["nix", "enable"])),
            (
                "channels",
                home.clone().select_segments(["nix", "channel", "enable"]),
            ),
            (
                "flatpak",
                home.clone()
                    .select_segments(["services", "flatpak", "enable"]),
            ),
            (
                "installerTools",
                !home
                    .clone()
                    .select_segments(["system", "disableInstallerTools"]),
            ),
        ]);

        let field_desktop = NixValue::record([
            (
                "gnome",
                home.clone()
                    .select_segments(["services", "desktopManager", "gnome", "enable"]),
            ),
            (
                "shell",
                home.clone()
                    .select_segments(["services", "gnome", "core-shell", "enable"]),
            ),
            (
                "gdm",
                home.clone()
                    .select_segments(["services", "displayManager", "gdm", "enable"]),
            ),
        ]);

        let field_integrations = NixValue::record([
            (
                "mime",
                home.clone().select_segments(["xdg", "mime", "enable"]),
            ),
            (
                "icons",
                home.clone().select_segments(["xdg", "icons", "enable"]),
            ),
            (
                "autostart",
                home.clone().select_segments(["xdg", "autostart", "enable"]),
            ),
            (
                "portal",
                home.clone().select_segments(["xdg", "portal", "enable"]),
            ),
            (
                "keyring",
                home.clone()
                    .select_segments(["services", "gnome", "gnome-keyring", "enable"]),
            ),
            (
                "calendar",
                home.clone().select_segments([
                    "services",
                    "gnome",
                    "evolution-data-server",
                    "enable",
                ]),
            ),
            (
                "gvfs",
                home.clone().select_segments(["services", "gvfs", "enable"]),
            ),
            (
                "gui",
                home.clone()
                    .select_segments(["systemd", "services", "qubes-gui-agent", "enable"]),
            ),
            (
                "graphics",
                home.clone()
                    .select_segments(["hardware", "graphics", "enable"]),
            ),
        ]);

        NixValue::record([
            ("installation", field_installation),
            ("desktop", field_desktop),
            ("integrations", field_integrations),
            (
                "portals",
                home.clone()
                    .select_segments(["xdg", "portal", "config", "common"]),
            ),
        ])
    }
}

/// Build the usr home gnome test gnome desktop work applications are routed to usr home
/// contribution using the caller supplied deferred values.
fn usr_home_gnome_application_routing_body(cfg: NixValue) -> NixValue {
    {
        let argument_1 = [
            (
                "enabled",
                cfg.clone()
                    .select_segments(["systemd", "services", "qubes-gui-agent", "enable"]),
            ),
            (
                "envPath",
                cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-gui-agent",
                    "environment",
                    "ENV_PATH",
                ]),
            ),
            (
                "restart",
                cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-gui-agent",
                    "serviceConfig",
                    "Restart",
                ]),
            ),
            (
                "restartSec",
                cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-gui-agent",
                    "serviceConfig",
                    "RestartSec",
                ]),
            ),
            (
                "startLimit",
                cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-gui-agent",
                    "unitConfig",
                    "StartLimitIntervalSec",
                ]),
            ),
            (
                "monitorLayout",
                native_contract(cfg.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes-rpc/qubes.SetMonitorLayout",
                    "source",
                ])),
            ),
            (
                "sessionReady",
                native_contract(cfg.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes-rpc/airlock.AppSessionReady",
                    "source",
                ])),
            ),
            (
                "sessionReadyConfig",
                cfg.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes/rpc-config/airlock.AppSessionReady",
                    "text",
                ]),
            ),
            (
                "forkAutostart",
                cfg.clone().select_segments([
                    "environment",
                    "etc",
                    "xdg/autostart/qubes-qrexec-fork-server.desktop",
                    "text",
                ]),
            ),
            (
                "wantedBy",
                cfg.clone()
                    .select_segments(["systemd", "services", "qubes-gui-agent", "wantedBy"]),
            ),
            (
                "pam",
                cfg.clone().select_segments([
                    "security",
                    "pam",
                    "services",
                    "qubes-gui-agent",
                    "startSession",
                ]),
            ),
            (
                "session",
                native_contract(cfg.clone().select_segments([
                    "environment",
                    "etc",
                    "X11/xinit/xinitrc",
                    "source",
                ])),
            ),
            (
                "xorg",
                cfg.clone().select_segments([
                    "environment",
                    "etc",
                    "X11/xorg-qubes.conf.template",
                    "source",
                ]),
            ),
            (
                "networkBoot",
                cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-network-uplink",
                    "wantedBy",
                ]),
            ),
            (
                "networkReconcile",
                cfg.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-network-uplink",
                    "script",
                ]),
            ),
        ];

        NixValue::record(argument_1)
    }
}

/// Build the usr home gnome test gnome desktop work applications are routed to usr home
/// contribution using the caller supplied deferred values.
fn usr_home_gnome_application_routing_2(
    application_vm: NixValue,
    minimal: NixValue,
    workspace: NixValue,
) -> NixValue {
    {
        let field_app_guest_uplink = NixValue::record([
            (
                "requires",
                application_vm.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-network-uplink@",
                    "requires",
                ]),
            ),
            (
                "after",
                application_vm.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-network-uplink@",
                    "after",
                ]),
            ),
            (
                "bindsTo",
                application_vm.clone().select_segments([
                    "systemd",
                    "services",
                    "qubes-network-uplink@",
                    "bindsTo",
                ]),
            ),
        ]);

        let field_app_gui = NixValue::builtin("map")
            .call(NixValue::function(|cfg| {
                usr_home_gnome_application_routing_body(cfg)
            }))
            .call(NixValue::list([application_vm.clone()]));

        NixValue::record([
            (
                "discovery",
                workspace.clone().select_segments([
                    "airlock",
                    "sysGuiGuest",
                    "applicationMenus",
                    "enable",
                ]),
            ),
            (
                "workspaceCoreApps",
                workspace
                    .clone()
                    .select_segments(["services", "gnome", "core-apps", "enable"]),
            ),
            (
                "minimalCoreApps",
                minimal
                    .clone()
                    .select_segments(["services", "gnome", "core-apps", "enable"]),
            ),
            (
                "workspaceEtc",
                NixValue::builtin("attrNames")
                    .call(workspace.clone().select_segments(["environment", "etc"])),
            ),
            (
                "workspaceStartApp",
                workspace.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes-rpc/qubes.StartApp",
                    "source",
                ]),
            ),
            (
                "workspaceStartAppConfig",
                workspace.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes/rpc-config/qubes.StartApp",
                    "text",
                ]),
            ),
            (
                "workspaceGSettings",
                workspace.clone().select_segments([
                    "services",
                    "desktopManager",
                    "gnome",
                    "extraGSettingsOverrides",
                ]),
            ),
            (
                "guiLogRules",
                workspace
                    .clone()
                    .select_segments(["systemd", "tmpfiles", "rules"]),
            ),
            (
                "mutterFlags",
                workspace
                    .clone()
                    .select_segments(["nixpkgs", "pkgs", "mutter", "mesonFlags"]),
            ),
            (
                "appGuestDhcp",
                application_vm
                    .clone()
                    .select_segments(["networking", "useDHCP"]),
            ),
            ("appGuestUplink", field_app_guest_uplink),
            (
                "appGuestForwarding",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("qubes-network"))
                    .call(
                        application_vm
                            .clone()
                            .select_segments(["systemd", "services"]),
                    ),
            ),
            (
                "appGuestUdev",
                application_vm
                    .clone()
                    .select_segments(["services", "udev", "extraRules"]),
            ),
            (
                "appGuestVifLink",
                NixValue::builtin("toString").call(application_vm.clone().select_segments([
                    "environment",
                    "etc",
                    "systemd/network/80-qubes-vif.link",
                    "source",
                ])),
            ),
            ("appGui", field_app_gui),
        ])
    }
}

/// Build the usr home gnome test gnome desktop work applications are routed to usr home
/// contribution using the caller supplied deferred values.
fn usr_home_gnome_application_routing_3(f: NixValue) -> NixValue {
    {
        let receiver = f
            .clone()
            .select_segments([
                "nixosConfigurations",
                "airlock-domain-sys-gui-usb-gui-lab",
                "extendModules",
            ])
            .call({
                let field_modules =
                    NixValue::list([NixValue::function_attrs_open(["lib"], |arguments_1| {
                        (
                            Vec::<(&str, NixValue)>::new(),
                            field(
                                ["airlock", "sysGuiGuest", "defaultApplicationVM"],
                                arguments_1
                                    .clone()
                                    .select("lib")
                                    .clone()
                                    .select_segments(["mkForce"])
                                    .call(NixValue::from("usr-home")),
                            ),
                        )
                    })]);

                field(["modules"], field_modules)
            })
            .select_segments(["config"]);

        receiver.bind(|workspace| {
            f.clone()
                .select_segments([
                    "nixosConfigurations",
                    "airlock-domain-sys-gui-usb-gui-lab",
                    "config",
                ])
                .bind(|minimal| {
                    f.clone()
                        .select_segments([
                            "nixosConfigurations",
                            "airlock-domain-usr-home",
                            "config",
                        ])
                        .bind(|application_vm| {
                            usr_home_gnome_application_routing_2(
                                application_vm,
                                minimal.clone(),
                                workspace.clone(),
                            )
                        })
                })
        })
    }
}
