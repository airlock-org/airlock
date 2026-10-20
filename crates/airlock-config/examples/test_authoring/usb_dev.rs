//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test usb dom0 display does not wait for VM password
/// mutation.
pub(super) fn independent_display_startup(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .select_segments(["nixosConfigurations"])
        .bind(|configs| {
            NixValue::function(|vm| {
                NixValue::record([
                    (
                        "password",
                        vm.clone()
                            .select_segments(["users", "users", "user", "initialPassword"]),
                    ),
                    (
                        "initialHash",
                        vm.clone().select_segments([
                            "users",
                            "users",
                            "user",
                            "initialHashedPassword",
                        ]),
                    ),
                    (
                        "mutable",
                        vm.clone().select_segments(["users", "mutableUsers"]),
                    ),
                ])
            })
            .bind(|login| usb_dev_independent_display_startup_3(login, configs.clone()))
        })
}

/// Build the usb dev test usb dom0 display does not wait for VM password mutation contribution
/// using the caller supplied deferred values.
fn usb_dev_independent_display_startup_body(recovery_config: NixValue) -> NixValue {
    {
        let field_setup = recovery_config
            .clone()
            .attr_or("airlock", NixValue::record(Vec::<(&str, NixValue)>::new()))
            .attr_or("qubes", NixValue::record(Vec::<(&str, NixValue)>::new()))
            .attr_or(
                "setupConfigOnBoot",
                NixValue::record(Vec::<(&str, NixValue)>::new()),
            )
            .attr_or("enable", NixValue::from(false));

        NixValue::record([
            (
                "role",
                recovery_config.clone().select_segments([
                    "environment",
                    "etc",
                    "airlock/boot-role",
                    "text",
                ]),
            ),
            (
                "xen",
                recovery_config
                    .clone()
                    .select_segments(["virtualisation", "xen", "enable"]),
            ),
            (
                "display",
                recovery_config
                    .clone()
                    .select_segments(["services", "xserver", "enable"]),
            ),
            ("setup", field_setup),
            (
                "networkManager",
                recovery_config
                    .clone()
                    .select_segments(["networking", "networkmanager", "enable"]),
            ),
            (
                "tailscale",
                recovery_config
                    .clone()
                    .select_segments(["services", "tailscale", "enable"]),
            ),
            (
                "ssh",
                recovery_config
                    .clone()
                    .select_segments(["services", "openssh", "enable"]),
            ),
            (
                "poolRecovery",
                recovery_config.clone().select_segments([
                    "environment",
                    "etc",
                    "airlock/pool-recovery",
                    "text",
                ]),
            ),
            (
                "store",
                recovery_config
                    .clone()
                    .select_segments(["fileSystems", "/nix/store", "device"]),
            ),
            (
                "passwordFile",
                recovery_config.clone().select_segments([
                    "users",
                    "users",
                    "user",
                    "hashedPasswordFile",
                ]),
            ),
        ])
    }
}

/// Build the usb dev test usb dom0 display does not wait for VM password mutation contribution
/// using the caller supplied deferred values.
fn usb_dev_independent_display_startup_2(
    config: NixValue,
    configs: NixValue,
    dom0_login: NixValue,
    login: NixValue,
) -> NixValue {
    {
        let field_dom0_logins = NixValue::concat_lists([
            NixValue::list([dom0_login.clone().call(config.clone())]),
            NixValue::builtin("map")
                .call(NixValue::function(|name| {
                    dom0_login.clone().call(
                        NixValue::builtin("getAttr")
                            .call(name.clone())
                            .call(config.clone().select_segments(["specialisation"]))
                            .select_segments(["configuration"]),
                    )
                }))
                .call(
                    NixValue::builtin("filter")
                        .call(NixValue::function(|name| {
                            !name.clone().equals(NixValue::from("usb-text-recovery"))
                        }))
                        .call(
                            NixValue::builtin("attrNames")
                                .call(config.clone().select_segments(["specialisation"])),
                        ),
                ),
        ]);

        let field_lab_has_password_service = NixValue::builtin("hasAttr")
            .call(NixValue::from("airlock-usb-gui-password"))
            .call(config.clone().select_segments([
                "specialisation",
                "usb-lab",
                "configuration",
                "systemd",
                "services",
            ]));

        let field_recovery = config
            .clone()
            .select_segments(["specialisation", "usb-text-recovery", "configuration"])
            .bind(usb_dev_independent_display_startup_body);

        NixValue::record([
            (
                "specialisations",
                NixValue::builtin("attrNames")
                    .call(config.clone().select_segments(["specialisation"])),
            ),
            (
                "baseSetup",
                config
                    .clone()
                    .select_segments(["airlock", "qubes", "setupConfigOnBoot", "enable"]),
            ),
            (
                "normalSetup",
                config.clone().select_segments([
                    "specialisation",
                    "usb-lab",
                    "configuration",
                    "airlock",
                    "qubes",
                    "setupConfigOnBoot",
                    "enable",
                ]),
            ),
            (
                "journal",
                config
                    .clone()
                    .select_segments(["services", "journald", "settings", "Journal"]),
            ),
            (
                "conciseBootMenu",
                config
                    .clone()
                    .select_segments(["airlock", "dom0", "bootMenu", "enable"]),
            ),
            (
                "imageMarker",
                config
                    .clone()
                    .select_segments(["environment", "etc", "airlock/usb-dev", "text"]),
            ),
            ("dom0Logins", field_dom0_logins),
            (
                "hasPasswordService",
                NixValue::builtin("hasAttr")
                    .call(NixValue::from("airlock-usb-gui-password"))
                    .call(config.clone().select_segments(["systemd", "services"])),
            ),
            (
                "displayRequires",
                config.clone().select_segments([
                    "systemd",
                    "services",
                    "display-manager",
                    "requires",
                ]),
            ),
            (
                "displayAfter",
                config
                    .clone()
                    .select_segments(["systemd", "services", "display-manager", "after"]),
            ),
            (
                "hybridLogin",
                login.clone().call(
                    configs
                        .clone()
                        .select_segments(["airlock-domain-sys-gui-usb", "config"]),
                ),
            ),
            (
                "labLogin",
                login.clone().call(
                    configs
                        .clone()
                        .select_segments(["airlock-domain-sys-gui-usb-gui-lab", "config"]),
                ),
            ),
            (
                "normalPassword",
                configs.clone().select_segments([
                    "airlock-domain-sys-gui",
                    "config",
                    "users",
                    "users",
                    "user",
                    "initialPassword",
                ]),
            ),
            ("labHasPasswordService", field_lab_has_password_service),
            ("recovery", field_recovery),
        ])
    }
}

/// Build the usb dev test usb dom0 display does not wait for VM password mutation contribution
/// using the caller supplied deferred values.
fn usb_dev_independent_display_startup_3(login: NixValue, configs: NixValue) -> NixValue {
    NixValue::function(|host| {
        NixValue::record([
            (
                "mutable",
                host.clone().select_segments(["users", "mutableUsers"]),
            ),
            (
                "passwordFile",
                host.clone()
                    .select_segments(["users", "users", "user", "hashedPasswordFile"]),
            ),
            (
                "account",
                host.clone().select_segments(["airlock", "dom0", "user"]),
            ),
            (
                "terminalAccount",
                host.clone()
                    .select_segments(["airlock", "devDom0Terminal", "user"]),
            ),
            (
                "uid",
                host.clone()
                    .select_segments(["users", "users", "user", "uid"]),
            ),
            (
                "home",
                host.clone()
                    .select_segments(["users", "users", "user", "home"]),
            ),
            (
                "rootHash",
                host.clone()
                    .select_segments(["users", "users", "root", "hashedPassword"]),
            ),
        ])
    })
    .bind(|dom0_login| {
        configs
            .clone()
            .select_segments(["usb-dev", "config"])
            .bind(|config| {
                usb_dev_independent_display_startup_2(
                    config,
                    configs.clone(),
                    dom0_login.clone(),
                    login.clone(),
                )
            })
    })
}
