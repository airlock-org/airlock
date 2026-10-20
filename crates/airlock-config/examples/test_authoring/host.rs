//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Describe the host evaluation boundary using caller-supplied source trees.
pub(super) fn expression() -> NixValue {
    NixValue::function_attrs(
        [
            "lib",
            "nixpkgs",
            "nixpkgsSource",
            "qubesSource",
            "libvirtSource",
            "machine",
            "apps",
            "generated",
            "variant",
            "revision",
            "entries",
            "policyOnly",
            "desktopOnly",
            "desktopDetails",
            "vmEntriesOnly",
            "physicalOnly",
        ],
        host_expression_5,
    )
}

/// Build the host expression contribution using the caller supplied deferred values.
fn host_expression_body(desktop: NixValue) -> NixValue {
    NixValue::record([
        (
            "session",
            desktop.clone().select_segments([
                "config",
                "services",
                "displayManager",
                "defaultSession",
            ]),
        ),
        (
            "defaultSession",
            desktop.clone().select_segments([
                "config",
                "services",
                "displayManager",
                "defaultSession",
            ]),
        ),
        (
            "gnome",
            desktop.clone().select_segments([
                "config",
                "services",
                "desktopManager",
                "gnome",
                "enable",
            ]),
        ),
        (
            "capture",
            desktop
                .clone()
                .select_segments(["config", "environment", "etc"])
                .has_attr("airlock/capture-sender.json"),
        ),
        (
            "captureOptions",
            desktop
                .clone()
                .select_segments(["options", "airlock"])
                .has_attr("captureTransfer"),
        ),
        (
            "desktopOptions",
            desktop
                .clone()
                .select_segments(["options", "airlock"])
                .has_attr("sysGuiDesktop"),
        ),
    ])
}

/// Build the host expression contribution using the caller supplied deferred values.
fn host_expression_2(bindings_2: NixValue, arguments_1: NixValue) -> NixValue {
    {
        let when_false = {
            let when_true = NixValue::builtin("removeAttrs")
                .call(
                    bindings_2
                        .clone()
                        .select("outputs")
                        .clone()
                        .select_segments(["lib", "machineGuestEntries"])
                        .call(NixValue::from("x86_64-linux")),
                )
                .call(NixValue::list([
                    NixValue::from("sys-gui-hybrid"),
                    NixValue::from("sys-gui-standalone"),
                ]));

            let when_false = {
                let when_true = arguments_1
                    .clone()
                    .select("lib")
                    .clone()
                    .select_segments(["mapAttrs"])
                    .call(NixValue::function(|_| {
                        NixValue::function(|configuration| {
                            desktop::expression().call(NixValue::record([
                                ("lib", arguments_1.clone().select("lib").clone()),
                                ("configuration", configuration.clone()),
                            ]))
                        })
                    }))
                    .call(NixValue::record([
                        (
                            "connected",
                            bindings_2
                                .clone()
                                .select("outputs")
                                .clone()
                                .select_segments([
                                    "nixosConfigurations",
                                    "airlock-domain-sys-gui-usb-lab",
                                ]),
                        ),
                        (
                            "standalone",
                            bindings_2
                                .clone()
                                .select("outputs")
                                .clone()
                                .select_segments([
                                    "nixosConfigurations",
                                    "airlock-domain-sys-gui-usb-gui-lab",
                                ]),
                        ),
                    ]));

                let when_false = {
                    let when_false = {
                        let when_true = {
                            let field_gui = arguments_1
                                .clone()
                                .select("lib")
                                .clone()
                                .select_segments(["findFirst"])
                                .call(NixValue::function(|p| {
                                    p.clone().attr_or("destination", NixValue::from("")).equals(
                                        NixValue::from(
                                            "/etc/qubes/policy.d/50-airlock-sys-gui.policy",
                                        ),
                                    )
                                }))
                                .call(
                                    NixValue::builtin("throw")
                                        .call(NixValue::from("missing GUI policy")),
                                )
                                .call(bindings_2.clone().select("cfg").clone().select_segments([
                                    "virtualisation",
                                    "qubes",
                                    "packages",
                                ]))
                                .select_segments(["text"]);

                            NixValue::record([
                                (
                                    "capture",
                                    bindings_2.clone().select("cfg").clone().select_segments([
                                        "environment",
                                        "etc",
                                        "qubes/policy.d/45-airlock-screen-capture.policy",
                                        "text",
                                    ]),
                                ),
                                ("gui", field_gui),
                            ])
                        };

                        let when_false = {
                            let field_host = {
                                let field_system = NixValue::record([
                                    (
                                        "drv",
                                        bindings_2.clone().select("cfg").clone().select_segments([
                                            "system", "build", "toplevel", "drvPath",
                                        ]),
                                    ),
                                    (
                                        "out",
                                        NixValue::builtin("toString").call(
                                            bindings_2
                                                .clone()
                                                .select("cfg")
                                                .clone()
                                                .select_segments(["system", "build", "toplevel"]),
                                        ),
                                    ),
                                ]);

                                let field_release = NixValue::record([
                                    (
                                        "drv",
                                        bindings_2.clone().select("cfg").clone().select_segments([
                                            "system",
                                            "build",
                                            "airlockMachineRelease",
                                            "drvPath",
                                        ]),
                                    ),
                                    (
                                        "out",
                                        NixValue::builtin("toString").call(
                                            bindings_2
                                                .clone()
                                                .select("cfg")
                                                .clone()
                                                .select_segments([
                                                    "system",
                                                    "build",
                                                    "airlockMachineRelease",
                                                ]),
                                        ),
                                    ),
                                ]);

                                let field_admission = NixValue::record([
                                    (
                                        "enable",
                                        bindings_2.clone().select("cfg").clone().select_segments([
                                            "airlock",
                                            "poolAdmission",
                                            "enable",
                                        ]),
                                    ),
                                    (
                                        "caller",
                                        bindings_2.clone().select("cfg").clone().select_segments([
                                            "airlock",
                                            "poolAdmission",
                                            "caller",
                                        ]),
                                    ),
                                    (
                                        "targets",
                                        bindings_2.clone().select("cfg").clone().select_segments([
                                            "airlock",
                                            "poolAdmission",
                                            "targets",
                                        ]),
                                    ),
                                ]);

                                NixValue::record([
                                    ("system", field_system),
                                    ("release", field_release),
                                    (
                                        "profile",
                                        bindings_2.clone().select("cfg").clone().select_segments([
                                            "airlock",
                                            "machineDeployment",
                                            "profile",
                                        ]),
                                    ),
                                    ("admission", field_admission),
                                ])
                            };

                            NixValue::record([
                                (
                                    "topology",
                                    bindings_2
                                        .clone()
                                        .select("cfg")
                                        .clone()
                                        .select_segments(["airlock", "qubes", "topology"]),
                                ),
                                ("host", field_host),
                                (
                                    "manifest",
                                    bindings_2.clone().select("cfg").clone().select_segments([
                                        "system",
                                        "build",
                                        "airlockMachineRelease",
                                        "manifest",
                                    ]),
                                ),
                            ])
                        };

                        NixValue::if_else(
                            arguments_1.clone().select("policyOnly").clone(),
                            when_true,
                            when_false,
                        )
                    };

                    NixValue::if_else(
                        arguments_1.clone().select("desktopOnly").clone(),
                        bindings_2
                            .clone()
                            .select("outputs")
                            .clone()
                            .select_segments([
                                "nixosConfigurations",
                                "airlock-domain-sys-gui-usb-lab",
                            ])
                            .bind(host_expression_body),
                        when_false,
                    )
                };

                NixValue::if_else(
                    arguments_1.clone().select("desktopDetails").clone(),
                    when_true,
                    when_false,
                )
            };

            NixValue::if_else(
                arguments_1.clone().select("vmEntriesOnly").clone(),
                when_true,
                when_false,
            )
        };

        NixValue::if_else(
            arguments_1.clone().select("physicalOnly").clone(),
            physical::expression().call(NixValue::record([
                ("lib", arguments_1.clone().select("lib").clone()),
                ("outputs", bindings_2.clone().select("outputs").clone()),
            ])),
            when_false,
        )
    }
}

/// Build the host expression contribution using the caller supplied deferred values.
fn host_expression_3(
    bindings_2: NixValue,
    arguments_1: NixValue,
    materialize: NixValue,
    upstream: NixValue,
) -> NixValue {
    {
        let field_outputs = NixValue::builtin("import")
            .call(add(
                arguments_1.clone().select("machine").clone(),
                NixValue::from("/flake.nix"),
            ))
            .select_segments(["outputs"])
            .call({
                let field_field_self =
                    bindings_2
                        .clone()
                        .select("outputs")
                        .clone()
                        .merge_attrs(NixValue::record([
                            ("outPath", arguments_1.clone().select("machine").clone()),
                            ("rev", arguments_1.clone().select("revision").clone()),
                            ("lastModified", NixValue::from(0_i64)),
                        ]));

                NixValue::record([
                    ("self", field_field_self),
                    ("nixpkgs", arguments_1.clone().select("nixpkgs").clone()),
                    (
                        "nixpkgs-libvirt_10_5",
                        field(
                            ["outPath"],
                            arguments_1.clone().select("libvirtSource").clone(),
                        ),
                    ),
                    ("nixos-qubes", upstream.clone()),
                    (
                        "airlock-apps",
                        field(["outPath"], arguments_1.clone().select("apps").clone()),
                    ),
                ])
            });

        let field_kernel = NixValue::record([
            ("name", NixValue::from("6.12.64-1.fc41")),
            (
                "package",
                bindings_2
                    .clone()
                    .select("outputs")
                    .clone()
                    .select_segments(["packages", "x86_64-linux", "airlock-qubes-vm-kernel"]),
            ),
        ]);

        let field_special_args = {
            let field_airlock_kernels = NixValue::record(Vec::<(&str, NixValue)>::new())
                .merge_attrs(
                    NixValue::builtin("listToAttrs").call(NixValue::list([NixValue::record([
                        (
                            "name",
                            bindings_2
                                .clone()
                                .select("kernel")
                                .clone()
                                .select_segments(["name"]),
                        ),
                        (
                            "value",
                            field(
                                ["source"],
                                NixValue::builtin("toString").call(
                                    bindings_2
                                        .clone()
                                        .select("kernel")
                                        .clone()
                                        .select_segments(["package"]),
                                ),
                            ),
                        ),
                    ])])),
                );

            NixValue::record([
                (
                    "airlockAppsSource",
                    arguments_1.clone().select("apps").clone(),
                ),
                (
                    "airlockConfigDomains",
                    arguments_1
                        .clone()
                        .select("lib")
                        .clone()
                        .select_segments(["mapAttrs"])
                        .call(NixValue::function(|_| materialize.clone()))
                        .call(arguments_1.clone().select("entries").clone()),
                ),
                (
                    "airlockQubeKernel",
                    bindings_2.clone().select("kernel").clone(),
                ),
                ("airlockKernels", field_airlock_kernels),
                (
                    "airlockSupport",
                    add(
                        source_path("/"),
                        arguments_1.clone().select("machine").clone(),
                    ),
                ),
                (
                    "airlockMachineGuestEntries",
                    arguments_1.clone().select("entries").clone(),
                ),
                (
                    "airlockSourceRevision",
                    arguments_1.clone().select("revision").clone(),
                ),
                ("airlockSourceTimestamp", NixValue::from(0_i64)),
                (
                    "airlockSourceLockHash",
                    NixValue::builtin("hashFile")
                        .call(NixValue::from("sha256"))
                        .call(add(
                            arguments_1.clone().select("machine").clone(),
                            NixValue::from("/flake.lock"),
                        )),
                ),
            ])
        };

        let field_bootstrap_configuration = {
            let argument_1 = {
                let field_modules = {
                    let argument_1 = [
                        NixValue::list([
                            bindings_2
                                .clone()
                                .select("outputs")
                                .clone()
                                .select_segments(["nixosModules", "dom0"]),
                            NixValue::builtin("import")
                                .call(add(
                                    arguments_1.clone().select("apps").clone(),
                                    NixValue::from("/airlock-config/generated/displays.nix"),
                                ))
                                .select_segments(["release"])
                                .call({
                                    let field_metadata = NixValue::record([
                                        (
                                            "revision",
                                            arguments_1.clone().select("revision").clone(),
                                        ),
                                        ("timestamp", NixValue::from(0_i64)),
                                        (
                                            "lockHash",
                                            NixValue::builtin("hashFile")
                                                .call(NixValue::from("sha256"))
                                                .call(add(
                                                    arguments_1.clone().select("machine").clone(),
                                                    NixValue::from("/flake.lock"),
                                                )),
                                        ),
                                    ]);

                                    NixValue::record([
                                        (
                                            "source",
                                            add(
                                                source_path("/"),
                                                arguments_1.clone().select("machine").clone(),
                                            ),
                                        ),
                                        ("apps", arguments_1.clone().select("apps").clone()),
                                        ("entries", arguments_1.clone().select("entries").clone()),
                                        ("metadata", field_metadata),
                                    ])
                                }),
                        ]),
                        {
                            let when_true = NixValue::list([
                                NixValue::builtin("import")
                                    .call(add(
                                        arguments_1.clone().select("apps").clone(),
                                        NixValue::from("/airlock-config/generated/modules.nix"),
                                    ))
                                    .select_segments(["modules"])
                                    .call(arguments_1.clone().select("machine").clone())
                                    .select_segments(["profiles"])
                                    .call(arguments_1.clone().select("apps").clone())
                                    .select_segments(["vmGui"]),
                                NixValue::builtin("import")
                                    .call(add(
                                        arguments_1.clone().select("apps").clone(),
                                        NixValue::from("/airlock-config/generated/modules.nix"),
                                    ))
                                    .select_segments(["modules"])
                                    .call(arguments_1.clone().select("machine").clone())
                                    .select_segments(["profiles"])
                                    .call(arguments_1.clone().select("apps").clone())
                                    .select_segments(["configSystem"]),
                            ]);

                            let when_false = NixValue::builtin("import")
                                .call(add(
                                    arguments_1.clone().select("generated").clone(),
                                    NixValue::concat_text([
                                        NixValue::from("/"),
                                        arguments_1.clone().select("variant").clone().to_text(),
                                        NixValue::from("/default.nix"),
                                    ]),
                                ))
                                .select_segments(["hostModules"])
                                .call(NixValue::record([
                                    ("lib", arguments_1.clone().select("lib").clone()),
                                    ("airlockApps", arguments_1.clone().select("apps").clone()),
                                    (
                                        "airlockConfigDomains",
                                        bindings_2
                                            .clone()
                                            .select("specialArgs")
                                            .clone()
                                            .select_segments(["airlockConfigDomains"]),
                                    ),
                                    (
                                        "airlockKernels",
                                        bindings_2
                                            .clone()
                                            .select("specialArgs")
                                            .clone()
                                            .select_segments(["airlockKernels"]),
                                    ),
                                    (
                                        "airlockSupport",
                                        bindings_2
                                            .clone()
                                            .select("specialArgs")
                                            .clone()
                                            .select_segments(["airlockSupport"]),
                                    ),
                                ]));

                            NixValue::if_else(
                                arguments_1
                                    .clone()
                                    .select("variant")
                                    .clone()
                                    .equals(NixValue::from("original")),
                                when_true,
                                when_false,
                            )
                        },
                    ];

                    NixValue::concat_lists(argument_1)
                };

                NixValue::record([
                    ("system", NixValue::from("x86_64-linux")),
                    (
                        "specialArgs",
                        bindings_2.clone().select("specialArgs").clone(),
                    ),
                    ("modules", field_modules),
                ])
            };

            arguments_1
                .clone()
                .select("lib")
                .clone()
                .select_segments(["nixosSystem"])
                .call(argument_1)
        };

        let field_physical = bindings_2
            .clone()
            .select("outputs")
            .clone()
            .select_segments(["lib", "mkMachineConfiguration"])
            .call(NixValue::record([
                ("includeAlternativeGuiProfiles", NixValue::from(false)),
                (
                    "machineEntries",
                    arguments_1.clone().select("entries").clone(),
                ),
                (
                    "extraSpecialArgs",
                    NixValue::record([
                        (
                            "airlockSourceRevision",
                            arguments_1.clone().select("revision").clone(),
                        ),
                        ("airlockSourceTimestamp", NixValue::from(0_i64)),
                    ]),
                ),
            ]));

        let field_configuration = NixValue::if_else(
            NixValue::builtin("pathExists").call(add(
                arguments_1.clone().select("generated").clone(),
                NixValue::concat_text([
                    NixValue::from("/"),
                    arguments_1.clone().select("variant").clone().to_text(),
                    NixValue::from("/hardware.nix"),
                ]),
            )),
            bindings_2.clone().select("physical").clone(),
            bindings_2.clone().select("bootstrapConfiguration").clone(),
        );

        let field_cfg = NixValue::if_else(
            NixValue::builtin("pathExists").call(add(
                arguments_1.clone().select("generated").clone(),
                NixValue::concat_text([
                    NixValue::from("/"),
                    arguments_1.clone().select("variant").clone().to_text(),
                    NixValue::from("/hardware.nix"),
                ]),
            )),
            bindings_2
                .clone()
                .select("physical")
                .clone()
                .select_segments(["config", "specialisation", "usb-lab", "configuration"]),
            bindings_2
                .clone()
                .select("configuration")
                .clone()
                .select_segments(["config"]),
        );

        NixValue::record([
            ("outputs", field_outputs),
            ("kernel", field_kernel),
            ("specialArgs", field_special_args),
            ("bootstrapConfiguration", field_bootstrap_configuration),
            ("physical", field_physical),
            ("configuration", field_configuration),
            ("cfg", field_cfg),
        ])
    }
}

/// Build the host expression contribution using the caller supplied deferred values.
fn host_expression_4(
    materialize: NixValue,
    arguments_1: NixValue,
    qubes_overlay: NixValue,
) -> NixValue {
    {
        let field_nixos_modules = field(
            ["qubes"],
            NixValue::record([
                (
                    "nixpkgs",
                    field(["overlays"], NixValue::list([qubes_overlay.clone()])),
                ),
                (
                    "imports",
                    NixValue::list([add(
                        arguments_1.clone().select("qubesSource").clone(),
                        NixValue::from("/nixos/modules/virtualisation/qubes"),
                    )]),
                ),
            ]),
        );

        NixValue::record([
            ("outPath", arguments_1.clone().select("qubesSource").clone()),
            ("overlays", field(["default"], qubes_overlay.clone())),
            ("nixosModules", field_nixos_modules),
        ])
    }
    .bind(|upstream| {
        recursive(|bindings_2| {
            host_expression_3(
                bindings_2,
                arguments_1.clone(),
                materialize.clone(),
                upstream.clone(),
            )
        })
        .bind(|bindings_2| host_expression_2(bindings_2, arguments_1.clone()))
    })
}

/// Build the host expression contribution using the caller supplied deferred values.
fn host_expression_5(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (
        vec![
            ("policyOnly", NixValue::from(false)),
            ("desktopOnly", NixValue::from(false)),
            ("desktopDetails", NixValue::from(false)),
            ("vmEntriesOnly", NixValue::from(false)),
            ("physicalOnly", NixValue::from(false)),
        ],
        {
            let receiver = {
                let argument_1 = {
                    let argument_1 = [
                        NixValue::builtin("import").call(add(
                            arguments_1.clone().select("qubesSource").clone(),
                            NixValue::from("/pkgs/top-level/overlay.nix"),
                        )),
                        NixValue::builtin("import")
                            .call(add(
                                arguments_1.clone().select("nixpkgsSource").clone(),
                                NixValue::from("/pkgs/top-level/by-name-overlay.nix"),
                            ))
                            .call(add(
                                arguments_1.clone().select("qubesSource").clone(),
                                NixValue::from("/pkgs/by-name"),
                            )),
                        NixValue::function(|r#final| {
                            NixValue::function(|_| {
                                let field_libvirt_10_5 = r#final
                                    .clone()
                                    .select_segments(["callPackage"])
                                    .call(add(
                                        arguments_1.clone().select("libvirtSource").clone(),
                                        NixValue::from("/pkgs/development/libraries/libvirt"),
                                    ))
                                    .call({
                                        let field_substitute_all = NixValue::function(|args| {
                                            r#final
                                                .clone()
                                                .select_segments(["replaceVars"])
                                                .call(args.clone().select_segments(["src"]))
                                                .call(
                                                    NixValue::builtin("removeAttrs")
                                                        .call(args.clone())
                                                        .call(NixValue::list([NixValue::from(
                                                            "src",
                                                        )])),
                                                )
                                        });

                                        NixValue::record([
                                            ("substituteAll", field_substitute_all),
                                            ("xhtml1", NixValue::null()),
                                            ("AppKit", NixValue::null()),
                                            ("Carbon", NixValue::null()),
                                        ])
                                    });

                                field(["libvirt_10_5"], field_libvirt_10_5)
                            })
                        }),
                        NixValue::builtin("import").call(add(
                            arguments_1.clone().select("qubesSource").clone(),
                            NixValue::from("/nix/overlay.nix"),
                        )),
                    ];

                    NixValue::list(argument_1)
                };

                arguments_1
                    .clone()
                    .select("lib")
                    .clone()
                    .select_segments(["composeManyExtensions"])
                    .call(argument_1)
            };

            receiver.bind(|qubes_overlay| {
                NixValue::builtin("import")
                    .call(add(
                        arguments_1.clone().select("apps").clone(),
                        NixValue::from("/airlock-config/generated/displays.nix"),
                    ))
                    .select_segments(["materialize"])
                    .bind(|materialize| {
                        host_expression_4(materialize, arguments_1.clone(), qubes_overlay.clone())
                    })
            })
        },
    )
}
