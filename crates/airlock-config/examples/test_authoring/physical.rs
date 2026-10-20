//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Describe the physical evaluation boundary using caller-supplied source trees.
pub(super) fn expression() -> NixValue {
    NixValue::function_attrs(["lib", "outputs"], |arguments_1| {
        (
            Vec::<(&str, NixValue)>::new(),
            arguments_1
                .clone()
                .select("outputs")
                .clone()
                .select_segments(["lib", "machineGuestEntries"])
                .call(NixValue::from("x86_64-linux"))
                .bind(|entries| {
                    arguments_1
                        .clone()
                        .select("outputs")
                        .clone()
                        .select_segments([
                            "nixosConfigurations",
                            "machine",
                            "config",
                            "specialisation",
                            "usb-gui-lab",
                            "configuration",
                        ])
                        .bind(|standalone| {
                            physical_expression_3(standalone, arguments_1.clone(), entries.clone())
                        })
                }),
        )
    })
}

/// Build the physical expression contribution using the caller supplied deferred values.
fn physical_expression_body(
    recovery: NixValue,
    arguments_1: NixValue,
    image: NixValue,
    normal: NixValue,
    prepared: NixValue,
    standalone: NixValue,
    worker: NixValue,
) -> NixValue {
    {
        let field_normal = {
            let field_stores = arguments_1
                .clone()
                .select("lib")
                .clone()
                .select_segments(["mapAttrs"])
                .call(NixValue::function(|_| {
                    NixValue::function(|domain| {
                        domain
                            .clone()
                            .select_segments(["backend", "store", "source"])
                    })
                }))
                .call(
                    normal
                        .clone()
                        .select_segments(["airlock", "qubes", "domains"]),
                );

            NixValue::record([
                (
                    "profile",
                    normal
                        .clone()
                        .select_segments(["airlock", "machineDeployment", "profile"]),
                ),
                (
                    "source",
                    normal.clone().select_segments([
                        "airlock",
                        "machineDeployment",
                        "initialSource",
                    ]),
                ),
                (
                    "bootSource",
                    normal
                        .clone()
                        .select_segments(["airlock", "dom0", "bootMenu", "source"]),
                ),
                (
                    "device",
                    normal
                        .clone()
                        .select_segments(["airlock", "guiGpu", "device"]),
                ),
                (
                    "assignment",
                    normal.clone().select_segments([
                        "airlock", "qubes", "domains", "sys-gui", "devices", "pci",
                    ]),
                ),
                ("stores", field_stores),
                ("prepared", prepared.clone()),
                (
                    "seed",
                    worker.clone().select_segments([
                        "systemd",
                        "services",
                        "airlock-machine-source-seed",
                        "script",
                    ]),
                ),
                (
                    "capturePolicy",
                    normal.clone().select_segments([
                        "environment",
                        "etc",
                        "qubes/policy.d/45-airlock-screen-capture.policy",
                        "text",
                    ]),
                ),
                (
                    "release",
                    normal.clone().select_segments([
                        "system",
                        "build",
                        "airlockMachineRelease",
                        "drvPath",
                    ]),
                ),
                (
                    "transport",
                    normal.clone().select_segments([
                        "system",
                        "build",
                        "airlockMachineTransport",
                        "drvPath",
                    ]),
                ),
                (
                    "releaseTopology",
                    normal.clone().select_segments([
                        "system",
                        "build",
                        "airlockMachineRelease",
                        "topology",
                    ]),
                ),
                (
                    "daemonStore",
                    normal.clone().select_segments([
                        "systemd",
                        "services",
                        "nix-daemon",
                        "environment",
                        "NIX_CONFIG",
                    ]),
                ),
                (
                    "hostOrdering",
                    normal.clone().select_segments([
                        "boot",
                        "initrd",
                        "systemd",
                        "services",
                        "initrd-find-nixos-closure",
                        "after",
                    ]),
                ),
            ])
        };

        let field_recovery = {
            let argument_1 = [
                (
                    "role",
                    recovery.clone().select_segments([
                        "environment",
                        "etc",
                        "airlock/boot-role",
                        "text",
                    ]),
                ),
                (
                    "source",
                    NixValue::builtin("fromJSON").call(recovery.clone().select_segments([
                        "environment",
                        "etc",
                        "airlock/boot-source.json",
                        "text",
                    ])),
                ),
                (
                    "nixEnabled",
                    recovery.clone().select_segments(["nix", "enable"]),
                ),
                (
                    "mounts",
                    NixValue::builtin("attrNames")
                        .call(recovery.clone().select_segments(["fileSystems"])),
                ),
                (
                    "store",
                    recovery
                        .clone()
                        .select_segments(["fileSystems", "/nix/store"]),
                ),
                (
                    "hasTopology",
                    recovery.clone().has_attr("airlock").and(
                        recovery
                            .clone()
                            .attr_or("airlock", NixValue::record(Vec::<(&str, NixValue)>::new()))
                            .has_attr("qubes"),
                    ),
                ),
                (
                    "hasHostStore",
                    recovery
                        .clone()
                        .select_segments(["boot", "initrd", "systemd", "services"])
                        .has_attr("airlock-host-store"),
                ),
                (
                    "sameKernel",
                    recovery
                        .clone()
                        .select_segments(["boot", "kernelPackages", "kernel", "drvPath"])
                        .equals(normal.clone().select_segments([
                            "boot",
                            "kernelPackages",
                            "kernel",
                            "drvPath",
                        ])),
                ),
                (
                    "sameXen",
                    recovery
                        .clone()
                        .select_segments(["virtualisation", "xen", "package", "drvPath"])
                        .equals(normal.clone().select_segments([
                            "virtualisation",
                            "xen",
                            "package",
                            "drvPath",
                        ])),
                ),
                (
                    "system",
                    recovery
                        .clone()
                        .select_segments(["system", "build", "toplevel", "drvPath"]),
                ),
            ];

            NixValue::record(argument_1)
        };

        let field_standalone = NixValue::record([
            (
                "domains",
                NixValue::builtin("attrNames").call(
                    standalone
                        .clone()
                        .select_segments(["airlock", "qubes", "domains"]),
                ),
            ),
            (
                "profile",
                standalone
                    .clone()
                    .select_segments(["airlock", "machineDeployment", "profile"]),
            ),
            (
                "assignment",
                standalone
                    .clone()
                    .select_segments(["airlock", "qubes", "domains", "sys-gui", "devices", "pci"]),
            ),
            (
                "system",
                standalone
                    .clone()
                    .select_segments(["system", "build", "toplevel", "drvPath"]),
            ),
        ]);

        NixValue::record([
            (
                "carrierSetup",
                image.clone().select_segments([
                    "config",
                    "airlock",
                    "qubes",
                    "setupConfigOnBoot",
                    "enable",
                ]),
            ),
            (
                "specialisations",
                NixValue::builtin("attrNames")
                    .call(image.clone().select_segments(["config", "specialisation"])),
            ),
            ("normal", field_normal),
            ("recovery", field_recovery),
            ("standalone", field_standalone),
        ])
    }
}

/// Build the physical expression contribution using the caller supplied deferred values.
fn physical_expression_2(
    prepared: NixValue,
    arguments_1: NixValue,
    standalone: NixValue,
    worker: NixValue,
) -> NixValue {
    {
        let receiver = arguments_1
            .clone()
            .select("outputs")
            .clone()
            .select_segments(["lib", "mkMachineConfiguration"])
            .call({
                let field_hardware_modules = NixValue::list([field(
                    ["airlock", "guiGpu", "device"],
                    NixValue::record([
                        ("bdf", NixValue::from("0000:09:03.0")),
                        ("port", NixValue::from("00_03.1-00_00.0")),
                        ("vendor", NixValue::from("8086")),
                        ("device", NixValue::from("1234")),
                        ("driver", NixValue::from("i915")),
                    ]),
                )]);

                NixValue::record([
                    ("includeAlternativeGuiProfiles", NixValue::from(false)),
                    ("storeSources", prepared.clone()),
                    (
                        "extraGuestSpecialArgs",
                        field(
                            ["airlockMachineSeedArchive"],
                            NixValue::from("/seed.tar.gz"),
                        ),
                    ),
                    ("hardwareModules", field_hardware_modules),
                ])
            });

        receiver.bind(|image| {
            image
                .clone()
                .select_segments(["config", "specialisation", "usb-lab", "configuration"])
                .bind(|normal| {
                    image
                        .clone()
                        .select_segments([
                            "config",
                            "specialisation",
                            "usb-text-recovery",
                            "configuration",
                        ])
                        .bind(|recovery| {
                            physical_expression_body(
                                recovery,
                                arguments_1.clone(),
                                image.clone(),
                                normal.clone(),
                                prepared.clone(),
                                standalone.clone(),
                                worker.clone(),
                            )
                        })
                })
        })
    }
}

/// Build the physical expression contribution using the caller supplied deferred values.
fn physical_expression_3(
    standalone: NixValue,
    arguments_1: NixValue,
    entries: NixValue,
) -> NixValue {
    arguments_1
        .clone()
        .select("outputs")
        .clone()
        .select_segments(["lib", "mkMachineGuestEntries"])
        .call(NixValue::from("x86_64-linux"))
        .call(field(
            ["airlockMachineSeedArchive"],
            NixValue::from("/seed.tar.gz"),
        ))
        .select_segments(["sys-ctrl", "configuration", "config"])
        .bind(|worker| {
            arguments_1
                .clone()
                .select("lib")
                .clone()
                .select_segments(["mapAttrs"])
                .call(NixValue::function(|name| {
                    NixValue::function(|_| {
                        NixValue::concat_text([
                            NixValue::from("/prepared/"),
                            name.clone().to_text(),
                        ])
                    })
                }))
                .call(
                    NixValue::builtin("removeAttrs")
                        .call(entries.clone())
                        .call(NixValue::list([
                            NixValue::from("sys-gui-hybrid"),
                            NixValue::from("sys-gui-standalone"),
                        ])),
                )
                .bind(|prepared| {
                    physical_expression_2(
                        prepared,
                        arguments_1.clone(),
                        standalone.clone(),
                        worker.clone(),
                    )
                })
        })
}
