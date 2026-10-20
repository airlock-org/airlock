//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for terminal config.
pub(super) fn terminal_configuration(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(|f| {
            f.clone()
                .select_segments(["nixosConfigurations", "machine"])
                .bind(|usb| {
                    f.clone()
                        .select_segments([
                            "nixosConfigurations",
                            "airlock-domain-sys-gui-usb-gui-lab",
                            "config",
                        ])
                        .bind(|vm| {
                            dev_dom0_terminal_terminal_configuration_3(vm, f.clone(), usb.clone())
                        })
                })
        })
}

/// Inspect evaluated configuration for terminal config.
pub(super) fn terminal_client(inputs: NixValue) -> NixValue {
    native_contract(NixValue::concat_text([inputs
        .clone()
        .select("client_path")
        .to_text()]))
}

/// Build the dev dom0 terminal terminal config contribution using the caller supplied deferred
/// values.
fn dev_dom0_terminal_terminal_configuration_body(policy: NixValue, captures: Bindings) -> NixValue {
    let f = captures.get("f");
    let forbidden = captures.get("forbidden");
    let vm = captures.get("vm");
    let host = captures.get("host");
    let lab = captures.get("lab");
    let launcher = captures.get("launcher");
    let normal = captures.get("normal");
    let normal_guest = captures.get("normal_guest");

    {
        let field_rejected = NixValue::builtin("map")
            .call(NixValue::function(|a| {
                a.clone().select_segments(["message"])
            }))
            .call(
                NixValue::builtin("filter")
                    .call(NixValue::function(|a| {
                        !a.clone().select_segments(["assertion"])
                    }))
                    .call(forbidden.clone().select_segments(["assertions"])),
            );

        NixValue::record([
            (
                "handler",
                native_contract(host.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes-rpc/airlock.DevDom0Shell",
                    "source",
                ])),
            ),
            (
                "policy",
                NixValue::builtin("readFile").call(NixValue::concat_text([
                    policy.clone().to_text(),
                    NixValue::from("/etc/qubes/policy.d/40-airlock-dev-dom0-terminal.policy"),
                ])),
            ),
            (
                "launcher",
                NixValue::builtin("readFile").call(NixValue::concat_text([
                    launcher.clone().to_text(),
                    NixValue::from("/bin/airlock-dom0-terminal"),
                ])),
            ),
            (
                "labHost",
                lab.clone()
                    .select_segments(["airlock", "devDom0Terminal", "host", "enable"]),
            ),
            (
                "guest",
                vm.clone()
                    .select_segments(["airlock", "devDom0Terminal", "guest", "enable"]),
            ),
            (
                "hybridGuest",
                f.clone().select_segments([
                    "nixosConfigurations",
                    "airlock-domain-sys-gui-usb",
                    "config",
                    "airlock",
                    "devDom0Terminal",
                    "guest",
                    "enable",
                ]),
            ),
            (
                "normalHost",
                normal
                    .clone()
                    .select_segments(["airlock", "devDom0Terminal", "host", "enable"]),
            ),
            (
                "normalHandler",
                normal
                    .clone()
                    .select_segments(["environment", "etc"])
                    .has_attr("qubes-rpc/airlock.DevDom0Shell"),
            ),
            (
                "normalGuest",
                normal_guest.clone().select_segments([
                    "airlock",
                    "devDom0Terminal",
                    "guest",
                    "enable",
                ]),
            ),
            (
                "sudoPassword",
                lab.clone()
                    .select_segments(["security", "sudo", "wheelNeedsPassword"]),
            ),
            ("rejected", field_rejected),
        ])
    }
}

/// Build the dev dom0 terminal terminal config contribution using the caller supplied deferred
/// values.
fn dev_dom0_terminal_terminal_configuration_2(
    launcher: NixValue,
    f: NixValue,
    vm: NixValue,
    host: NixValue,
    normal: NixValue,
    normal_guest: NixValue,
    usb: NixValue,
) -> NixValue {
    {
        let receiver = usb
            .clone()
            .select_segments(["extendModules"])
            .call({
                let field_modules =
                    NixValue::list([NixValue::function_attrs_open(["lib"], |arguments_1| {
                        (
                            Vec::<(&str, NixValue)>::new(),
                            field(
                                ["airlock", "image", "devMode", "enable"],
                                arguments_1
                                    .clone()
                                    .select("lib")
                                    .clone()
                                    .select_segments(["mkForce"])
                                    .call(NixValue::from(false)),
                            ),
                        )
                    })]);

                field(["modules"], field_modules)
            })
            .select_segments(["config"]);

        receiver.bind(|forbidden| {
            host.clone()
                .select_segments(["specialisation", "usb-gui-lab", "configuration"])
                .bind(|lab| {
                    NixValue::builtin("head")
                        .call(
                            NixValue::builtin("filter")
                                .call(NixValue::function(|p| {
                                    p.clone()
                                        .attr_or("name", NixValue::from(""))
                                        .equals(NixValue::from("airlock-dev-dom0-terminal-policy"))
                                }))
                                .call(host.clone().select_segments([
                                    "virtualisation",
                                    "qubes",
                                    "packages",
                                ])),
                        )
                        .bind(|policy| {
                            dev_dom0_terminal_terminal_configuration_body(
                                policy,
                                Bindings::new([
                                    ("f", f.clone()),
                                    ("forbidden", forbidden.clone()),
                                    ("vm", vm.clone()),
                                    ("host", host.clone()),
                                    ("lab", lab.clone()),
                                    ("launcher", launcher.clone()),
                                    ("normal", normal.clone()),
                                    ("normal_guest", normal_guest.clone()),
                                ]),
                            )
                        })
                })
        })
    }
}

/// Build the dev dom0 terminal terminal config contribution using the caller supplied deferred
/// values.
fn dev_dom0_terminal_terminal_configuration_3(
    vm: NixValue,
    f: NixValue,
    usb: NixValue,
) -> NixValue {
    f.clone()
        .select_segments(["nixosConfigurations", "vm-gui-minimal", "config"])
        .bind(|normal| {
            f.clone()
                .select_segments([
                    "nixosConfigurations",
                    "airlock-domain-sys-gui-minimal",
                    "config",
                ])
                .bind(|normal_guest| {
                    usb.clone().select_segments(["config"]).bind(|host| {
                        NixValue::builtin("head")
                            .call(
                                NixValue::builtin("filter")
                                    .call(NixValue::function(|p| {
                                        p.clone()
                                            .attr_or("name", NixValue::from(""))
                                            .equals(NixValue::from("airlock-dom0-terminal"))
                                    }))
                                    .call(
                                        vm.clone()
                                            .select_segments(["environment", "systemPackages"]),
                                    ),
                            )
                            .bind(|launcher| {
                                dev_dom0_terminal_terminal_configuration_2(
                                    launcher,
                                    f.clone(),
                                    vm.clone(),
                                    host.clone(),
                                    normal.clone(),
                                    normal_guest.clone(),
                                    usb.clone(),
                                )
                            })
                    })
                })
        })
}
