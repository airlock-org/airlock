//! Schema examples and rejection cases authored as Rusix expressions.

use super::*;

/// Exercise valid template, domain, device and RPC normalization.
fn valid() -> NixValue {
    {
        let field_outputs = NixValue::function_attrs(["self"], |arguments_1| {
            (
                Vec::<(&str, NixValue)>::new(),
                NixValue::builtin("import")
                    .call(add(
                        NixPath::from_source_text(
                            arguments_1
                                .clone()
                                .select("self")
                                .clone()
                                .select("outPath")
                                .into_expr::<String>(),
                        )
                        .as_expression(),
                        NixValue::from("/airlock/topology.nix"),
                    ))
                    .select("topology")
                    .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                    .bind(|topology| topology_schema_valid_body(topology, arguments_1.clone())),
            )
        });

        NixValue::record([
            (
                "description",
                NixValue::from("Airlock topology schema valid fixture"),
            ),
            ("outputs", field_outputs),
        ])
    }
}

/// Construct malformed inputs for distinct closed-schema failure modes.
fn invalid(case: &str) -> Result<NixValue, String> {
    {
        let argument_1 = match case {
            "missing-appvm-template" => {
                let field_outputs = NixValue::function_attrs(["self"], |arguments_1| {
                    (Vec::<(&str, NixValue)>::new(), {
                        let field_airlock = {
                            let field_topology = NixValue::builtin("import")
                                .call(add(
                                    NixPath::from_source_text(
                                        arguments_1
                                            .clone()
                                            .select("self")
                                            .clone()
                                            .select("outPath")
                                            .into_expr::<String>(),
                                    )
                                    .as_expression(),
                                    NixValue::from("/airlock/topology.nix"),
                                ))
                                .select("topology")
                                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                                .select_segments(["mkTopology"])
                                .call(field(["domains", "bad", "label"], NixValue::from("green")));

                            field(["topology"], field_topology)
                        };

                        field(["airlock"], field_airlock)
                    })
                });

                field(["outputs"], field_outputs)
            }
            "invalid-label" => {
                let field_outputs = NixValue::function_attrs(["self"], |arguments_1| {
                    (Vec::<(&str, NixValue)>::new(), {
                        let field_airlock = {
                            let field_topology = NixValue::builtin("import")
                                .call(add(
                                    NixPath::from_source_text(
                                        arguments_1
                                            .clone()
                                            .select("self")
                                            .clone()
                                            .select("outPath")
                                            .into_expr::<String>(),
                                    )
                                    .as_expression(),
                                    NixValue::from("/airlock/topology.nix"),
                                ))
                                .select("topology")
                                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                                .select_segments(["mkTopology"])
                                .call(field(
                                    ["domains", "bad"],
                                    NixValue::record([
                                        ("template", NixValue::from("airlock-template")),
                                        ("label", NixValue::from("clear")),
                                    ]),
                                ));

                            field(["topology"], field_topology)
                        };

                        field(["airlock"], field_airlock)
                    })
                });

                field(["outputs"], field_outputs)
            }
            "unknown-option" => {
                let field_outputs = NixValue::function_attrs(["self"], |arguments_1| {
                    (Vec::<(&str, NixValue)>::new(), {
                        let field_airlock = {
                            let field_topology = NixValue::builtin("import")
                                .call(add(
                                    NixPath::from_source_text(
                                        arguments_1
                                            .clone()
                                            .select("self")
                                            .clone()
                                            .select("outPath")
                                            .into_expr::<String>(),
                                    )
                                    .as_expression(),
                                    NixValue::from("/airlock/topology.nix"),
                                ))
                                .select("topology")
                                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                                .select_segments(["mkTopology"])
                                .call(field(
                                    ["domains", "bad"],
                                    NixValue::record([
                                        ("template", NixValue::from("airlock-template")),
                                        (
                                            "servies",
                                            NixValue::record(Vec::<(&str, NixValue)>::new()),
                                        ),
                                    ]),
                                ));

                            field(["topology"], field_topology)
                        };

                        field(["airlock"], field_airlock)
                    })
                });

                field(["outputs"], field_outputs)
            }
            "invalid-pci-device-id" => {
                let field_outputs = NixValue::function_attrs(["self"], |arguments_1| {
                    (Vec::<(&str, NixValue)>::new(), {
                        let field_airlock = {
                            let field_topology = NixValue::builtin("import")
                                .call(add(
                                    NixPath::from_source_text(
                                        arguments_1
                                            .clone()
                                            .select("self")
                                            .clone()
                                            .select("outPath")
                                            .into_expr::<String>(),
                                    )
                                    .as_expression(),
                                    NixValue::from("/airlock/topology.nix"),
                                ))
                                .select("topology")
                                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                                .select_segments(["mkTopology"])
                                .call(field(
                                    ["domains", "bad"],
                                    NixValue::record([
                                        ("template", NixValue::from("airlock-template")),
                                        (
                                            "devices",
                                            field(
                                                ["pci"],
                                                NixValue::list([field(
                                                    ["id"],
                                                    NixValue::from("../bad"),
                                                )]),
                                            ),
                                        ),
                                    ]),
                                ));

                            field(["topology"], field_topology)
                        };

                        field(["airlock"], field_airlock)
                    })
                });

                field(["outputs"], field_outputs)
            }
            "invalid-pci-assignment-mode" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_4(arguments_1)
                }),
            ),
            "template-import-without-source" => {
                let field_outputs = NixValue::function_attrs(["self"], |arguments_1| {
                    (Vec::<(&str, NixValue)>::new(), {
                        let field_airlock = {
                            let field_topology = NixValue::builtin("import")
                                .call(add(
                                    NixPath::from_source_text(
                                        arguments_1
                                            .clone()
                                            .select("self")
                                            .clone()
                                            .select("outPath")
                                            .into_expr::<String>(),
                                    )
                                    .as_expression(),
                                    NixValue::from("/airlock/topology.nix"),
                                ))
                                .select("topology")
                                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                                .select_segments(["mkTopology"])
                                .call(field(
                                    ["templates", "airlock-template", "import"],
                                    NixValue::from(true),
                                ));

                            field(["topology"], field_topology)
                        };

                        field(["airlock"], field_airlock)
                    })
                });

                field(["outputs"], field_outputs)
            }
            "template-source-and-source-ref" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_6(arguments_1)
                }),
            ),
            "template-invalid-build-ref" => {
                let field_outputs = NixValue::function_attrs(["self"], |arguments_1| {
                    (Vec::<(&str, NixValue)>::new(), {
                        let field_airlock = {
                            let field_topology = NixValue::builtin("import")
                                .call(add(
                                    NixPath::from_source_text(
                                        arguments_1
                                            .clone()
                                            .select("self")
                                            .clone()
                                            .select("outPath")
                                            .into_expr::<String>(),
                                    )
                                    .as_expression(),
                                    NixValue::from("/airlock/topology.nix"),
                                ))
                                .select("topology")
                                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                                .select_segments(["mkTopology"])
                                .call(field(
                                    ["templates", "airlock-template"],
                                    NixValue::record([
                                        ("import", NixValue::from(true)),
                                        ("source", field(["build"], NixValue::from("../bad"))),
                                    ]),
                                ));

                            field(["topology"], field_topology)
                        };

                        field(["airlock"], field_airlock)
                    })
                });

                field(["outputs"], field_outputs)
            }
            "non-airlock-qrexec-service" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_body(arguments_1)
                }),
            ),
            "reserved-qrexec-service" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_5(arguments_1)
                }),
            ),
            "qrexec-allow-any-source" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_3(arguments_1)
                }),
            ),
            "qrexec-target-dom0" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_2(arguments_1)
                }),
            ),
            "nix-closure-with-template" => {
                let field_outputs = NixValue::function_attrs(["self"], |arguments_1| {
                    (Vec::<(&str, NixValue)>::new(), {
                        let field_airlock = {
                            let field_topology = NixValue::builtin("import")
                                .call(add(
                                    NixPath::from_source_text(
                                        arguments_1
                                            .clone()
                                            .select("self")
                                            .clone()
                                            .select("outPath")
                                            .into_expr::<String>(),
                                    )
                                    .as_expression(),
                                    NixValue::from("/airlock/topology.nix"),
                                ))
                                .select("topology")
                                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                                .select_segments(["mkTopology"])
                                .call(field(
                                    ["domains", "bad"],
                                    NixValue::record([
                                        ("template", NixValue::from("airlock-template")),
                                        ("backend", field(["type"], NixValue::from("nix-closure"))),
                                    ]),
                                ));

                            field(["topology"], field_topology)
                        };

                        field(["airlock"], field_airlock)
                    })
                });

                field(["outputs"], field_outputs)
            }
            "nix-closure-missing-system" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_7(arguments_1)
                }),
            ),
            "nix-closure-writable-store" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_10(arguments_1)
                }),
            ),
            "nix-closure-unsupported-store-device" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_11(arguments_1)
                }),
            ),
            "nix-closure-missing-root" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_8(arguments_1)
                }),
            ),
            "nix-closure-readonly-root" => field(
                ["outputs"],
                NixValue::function_attrs(["self"], |arguments_1| {
                    topology_schema_invalid_9(arguments_1)
                }),
            ),
            _ => return Err(format!("unknown invalid topology case {case}")),
        };

        Ok(argument_1)
    }
}

/// Emit a valid schema example.
pub(super) fn expression() -> NixValue {
    valid()
}

/// Emit a malformed schema example selected by the integration harness.
pub(super) fn invalid_expression(case: &str) -> Result<NixValue, String> {
    invalid(case)
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_body(
    arguments_1: NixValue,
) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = NixValue::builtin("import")
                .call(add(
                    NixPath::from_source_text(
                        arguments_1
                            .clone()
                            .select("self")
                            .clone()
                            .select("outPath")
                            .into_expr::<String>(),
                    )
                    .as_expression(),
                    NixValue::from("/airlock/topology.nix"),
                ))
                .select("topology")
                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                .select_segments(["mkTopology"])
                .call(field(
                    ["qrexecPolicy", "rules"],
                    NixValue::list([NixValue::record([
                        ("service", NixValue::from("qubes.Filecopy")),
                        ("source", NixValue::from("dom0")),
                        ("target", NixValue::from("vault")),
                        ("action", NixValue::from("allow")),
                    ])]),
                ));

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_2(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = NixValue::builtin("import")
                .call(add(
                    NixPath::from_source_text(
                        arguments_1
                            .clone()
                            .select("self")
                            .clone()
                            .select("outPath")
                            .into_expr::<String>(),
                    )
                    .as_expression(),
                    NixValue::from("/airlock/topology.nix"),
                ))
                .select("topology")
                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                .select_segments(["mkTopology"])
                .call(field(
                    ["qrexecPolicy", "rules"],
                    NixValue::list([NixValue::record([
                        ("service", NixValue::from("airlock.PolicyProbe")),
                        ("source", NixValue::from("sys-config-vault")),
                        ("target", NixValue::from("dom0")),
                        ("action", NixValue::from("allow")),
                    ])]),
                ));

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_3(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = NixValue::builtin("import")
                .call(add(
                    NixPath::from_source_text(
                        arguments_1
                            .clone()
                            .select("self")
                            .clone()
                            .select("outPath")
                            .into_expr::<String>(),
                    )
                    .as_expression(),
                    NixValue::from("/airlock/topology.nix"),
                ))
                .select("topology")
                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                .select_segments(["mkTopology"])
                .call(field(
                    ["qrexecPolicy", "rules"],
                    NixValue::list([NixValue::record([
                        ("service", NixValue::from("airlock.PolicyProbe")),
                        ("source", NixValue::from("@anyvm")),
                        ("target", NixValue::from("sys-config-vault")),
                        ("action", NixValue::from("allow")),
                    ])]),
                ));

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_4(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = NixValue::builtin("import")
                .call(add(
                    NixPath::from_source_text(
                        arguments_1
                            .clone()
                            .select("self")
                            .clone()
                            .select("outPath")
                            .into_expr::<String>(),
                    )
                    .as_expression(),
                    NixValue::from("/airlock/topology.nix"),
                ))
                .select("topology")
                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                .select_segments(["mkTopology"])
                .call(field(
                    ["domains", "bad"],
                    NixValue::record([
                        ("template", NixValue::from("airlock-template")),
                        (
                            "devices",
                            field(
                                ["pci"],
                                NixValue::list([NixValue::record([
                                    ("id", NixValue::from("00_14.0")),
                                    ("mode", NixValue::from("manual")),
                                ])]),
                            ),
                        ),
                    ]),
                ));

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_5(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = NixValue::builtin("import")
                .call(add(
                    NixPath::from_source_text(
                        arguments_1
                            .clone()
                            .select("self")
                            .clone()
                            .select("outPath")
                            .into_expr::<String>(),
                    )
                    .as_expression(),
                    NixValue::from("/airlock/topology.nix"),
                ))
                .select("topology")
                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                .select_segments(["mkTopology"])
                .call(field(
                    ["qrexecPolicy", "rules"],
                    NixValue::list([NixValue::record([
                        ("service", NixValue::from("airlock.Config.ArchiveCandidate")),
                        ("source", NixValue::from("dom0")),
                        ("target", NixValue::from("sys-config-vault")),
                        ("action", NixValue::from("allow")),
                    ])]),
                ));

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_6(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = NixValue::builtin("import")
                .call(add(
                    NixPath::from_source_text(
                        arguments_1
                            .clone()
                            .select("self")
                            .clone()
                            .select("outPath")
                            .into_expr::<String>(),
                    )
                    .as_expression(),
                    NixValue::from("/airlock/topology.nix"),
                ))
                .select("topology")
                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                .select_segments(["mkTopology"])
                .call({
                    let field_templates = {
                        let field_airlock_template = NixValue::record([
                            ("import", NixValue::from(true)),
                            (
                                "source",
                                add(
                                    NixPath::from_source_text(
                                        arguments_1
                                            .clone()
                                            .select("self")
                                            .clone()
                                            .select("outPath")
                                            .into_expr::<String>(),
                                    )
                                    .as_expression(),
                                    NixValue::from("/template-source"),
                                ),
                            ),
                            (
                                "sourceRef",
                                field(["build"], NixValue::from("packages.x86_64-linux.template")),
                            ),
                        ]);

                        field(["airlock-template"], field_airlock_template)
                    };

                    field(["templates"], field_templates)
                });

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_7(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = NixValue::builtin("import")
                .call(add(
                    NixPath::from_source_text(
                        arguments_1
                            .clone()
                            .select("self")
                            .clone()
                            .select("outPath")
                            .into_expr::<String>(),
                    )
                    .as_expression(),
                    NixValue::from("/airlock/topology.nix"),
                ))
                .select("topology")
                .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                .select_segments(["mkTopology"])
                .call({
                    let field_domains = {
                        let field_bad = {
                            let field_backend = NixValue::record([
                                ("type", NixValue::from("nix-closure")),
                                ("kernel", field(["name"], NixValue::from("6.12.64-1.fc41"))),
                                (
                                    "root",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from("packages.x86_64-linux.bad-root"),
                                    ),
                                ),
                                (
                                    "store",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from("packages.x86_64-linux.bad-store"),
                                    ),
                                ),
                            ]);

                            field(["backend"], field_backend)
                        };

                        field(["bad"], field_bad)
                    };

                    field(["domains"], field_domains)
                });

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_8(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock =
            {
                let field_topology = NixValue::builtin("import")
                .call(add(
                    NixPath::from_source_text(
                        arguments_1
                            .clone()
                            .select("self")
                            .clone()
                            .select("outPath")
                            .into_expr::<String>(),
                    )
                    .as_expression(),
                    NixValue::from("/airlock/topology.nix"),
                )).select("topology").call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                .select_segments(["mkTopology"])
                .call({
                    let field_domains = {
                        let field_bad = {
                            let field_backend = NixValue::record([
                                ("type", NixValue::from("nix-closure")),
                                (
                                    "system",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from(
                                            "nixosConfigurations.bad.config.system.build.toplevel",
                                        ),
                                    ),
                                ),
                                ("kernel", field(["name"], NixValue::from("6.12.64-1.fc41"))),
                                (
                                    "store",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from("packages.x86_64-linux.bad-store"),
                                    ),
                                ),
                            ]);

                            field(["backend"], field_backend)
                        };

                        field(["bad"], field_bad)
                    };

                    field(["domains"], field_domains)
                });

                field(["topology"], field_topology)
            };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_9(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = {
                let argument_1 = {
                    let field_domains = {
                        let field_bad = {
                            let field_backend = NixValue::record([
                                ("type", NixValue::from("nix-closure")),
                                (
                                    "system",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from(
                                            "nixosConfigurations.bad.config.system.build.toplevel",
                                        ),
                                    ),
                                ),
                                ("kernel", field(["name"], NixValue::from("6.12.64-1.fc41"))),
                                (
                                    "root",
                                    NixValue::record([
                                        (
                                            "sourceRef",
                                            field(
                                                ["build"],
                                                NixValue::from("packages.x86_64-linux.bad-root"),
                                            ),
                                        ),
                                        ("readOnly", NixValue::from(true)),
                                    ]),
                                ),
                                (
                                    "store",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from("packages.x86_64-linux.bad-store"),
                                    ),
                                ),
                            ]);

                            field(["backend"], field_backend)
                        };

                        field(["bad"], field_bad)
                    };

                    field(["domains"], field_domains)
                };

                NixValue::builtin("import")
                    .call(add(
                        NixPath::from_source_text(
                            arguments_1
                                .clone()
                                .select("self")
                                .clone()
                                .select("outPath")
                                .into_expr::<String>(),
                        )
                        .as_expression(),
                        NixValue::from("/airlock/topology.nix"),
                    ))
                    .select("topology")
                    .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                    .select_segments(["mkTopology"])
                    .call(argument_1)
            };

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_10(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = {
                let argument_1 = {
                    let field_domains = {
                        let field_bad = {
                            let field_backend = NixValue::record([
                                ("type", NixValue::from("nix-closure")),
                                (
                                    "system",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from(
                                            "nixosConfigurations.bad.config.system.build.toplevel",
                                        ),
                                    ),
                                ),
                                ("kernel", field(["name"], NixValue::from("6.12.64-1.fc41"))),
                                (
                                    "root",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from("packages.x86_64-linux.bad-root"),
                                    ),
                                ),
                                (
                                    "store",
                                    NixValue::record([
                                        (
                                            "sourceRef",
                                            field(
                                                ["build"],
                                                NixValue::from("packages.x86_64-linux.bad-store"),
                                            ),
                                        ),
                                        ("readOnly", NixValue::from(false)),
                                    ]),
                                ),
                            ]);

                            field(["backend"], field_backend)
                        };

                        field(["bad"], field_bad)
                    };

                    field(["domains"], field_domains)
                };

                NixValue::builtin("import")
                    .call(add(
                        NixPath::from_source_text(
                            arguments_1
                                .clone()
                                .select("self")
                                .clone()
                                .select("outPath")
                                .into_expr::<String>(),
                        )
                        .as_expression(),
                        NixValue::from("/airlock/topology.nix"),
                    ))
                    .select("topology")
                    .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                    .select_segments(["mkTopology"])
                    .call(argument_1)
            };

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema invalid contribution using the caller supplied deferred values.
fn topology_schema_invalid_11(arguments_1: NixValue) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (Vec::<(&str, NixValue)>::new(), {
        let field_airlock = {
            let field_topology = {
                let argument_1 = {
                    let field_domains = {
                        let field_bad = {
                            let field_backend = NixValue::record([
                                ("type", NixValue::from("nix-closure")),
                                (
                                    "system",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from(
                                            "nixosConfigurations.bad.config.system.build.toplevel",
                                        ),
                                    ),
                                ),
                                ("kernel", field(["name"], NixValue::from("6.12.64-1.fc41"))),
                                (
                                    "root",
                                    field(
                                        ["sourceRef", "build"],
                                        NixValue::from("packages.x86_64-linux.bad-root"),
                                    ),
                                ),
                                (
                                    "store",
                                    NixValue::record([
                                        (
                                            "sourceRef",
                                            field(
                                                ["build"],
                                                NixValue::from("packages.x86_64-linux.bad-store"),
                                            ),
                                        ),
                                        ("device", NixValue::from("/dev/xvdf")),
                                    ]),
                                ),
                            ]);

                            field(["backend"], field_backend)
                        };

                        field(["bad"], field_bad)
                    };

                    field(["domains"], field_domains)
                };

                NixValue::builtin("import")
                    .call(add(
                        NixPath::from_source_text(
                            arguments_1
                                .clone()
                                .select("self")
                                .clone()
                                .select("outPath")
                                .into_expr::<String>(),
                        )
                        .as_expression(),
                        NixValue::from("/airlock/topology.nix"),
                    ))
                    .select("topology")
                    .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                    .select_segments(["mkTopology"])
                    .call(argument_1)
            };

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    })
}

/// Build the topology schema valid contribution using the caller supplied deferred values.
fn topology_schema_valid_body(topology: NixValue, arguments_1: NixValue) -> NixValue {
    {
        let field_airlock = {
            let field_topology = {
                let argument_1 = {
                    let field_templates = {
                        let field_airlock_template = NixValue::record([
                            ("import", NixValue::from(true)),
                            (
                                "source",
                                add(
                                    NixPath::from_source_text(
                                        arguments_1
                                            .clone()
                                            .select("self")
                                            .clone()
                                            .select("outPath")
                                            .into_expr::<String>(),
                                    )
                                    .as_expression(),
                                    NixValue::from("/template-source"),
                                ),
                            ),
                        ]);

                        let field_airlock_built_template = NixValue::record([
                            ("import", NixValue::from(true)),
                            (
                                "source",
                                NixValue::record([
                                    (
                                        "build",
                                        NixValue::from(
                                            "packages.x86_64-linux.airlock-built-template",
                                        ),
                                    ),
                                    ("subpath", NixValue::from("airlock-built-template")),
                                ]),
                            ),
                        ]);

                        NixValue::record([
                            ("airlock-template", field_airlock_template),
                            ("airlock-built-template", field_airlock_built_template),
                        ])
                    };

                    let field_kernels = field(
                        ["6.12.64-1.fc41", "source"],
                        add(
                            NixPath::from_source_text(
                                arguments_1
                                    .clone()
                                    .select("self")
                                    .clone()
                                    .select("outPath")
                                    .into_expr::<String>(),
                            )
                            .as_expression(),
                            NixValue::from("/kernel-source"),
                        ),
                    );

                    let field_domains = {
                        let field_test_qube = NixValue::record([
                            ("template", NixValue::from("airlock-template")),
                            ("label", NixValue::from("green")),
                            ("prefs", field(["memory"], NixValue::from(1024_i64))),
                            ("features", field(["airlock-role"], NixValue::from("test"))),
                            (
                                "services",
                                field(["network-manager"], NixValue::from(false)),
                            ),
                            (
                                "devices",
                                field(
                                    ["pci"],
                                    NixValue::list([NixValue::record([
                                        ("id", NixValue::from("00_14.0")),
                                        (
                                            "options",
                                            field(["no-strict-reset"], NixValue::from(true)),
                                        ),
                                    ])]),
                                ),
                            ),
                        ]);

                        let field_legacy_fedora = NixValue::record([
                            (
                                "backend",
                                NixValue::record([
                                    ("type", NixValue::from("qubes-template")),
                                    ("template", NixValue::from("airlock-template")),
                                ]),
                            ),
                            ("label", NixValue::from("yellow")),
                            ("netvm", NixValue::null()),
                        ]);

                        let field_nix_work = {
                            let field_backend = {
                                let field_system = field(
                                    ["source"],
                                    add(
                                        NixPath::from_source_text(
                                            arguments_1
                                                .clone()
                                                .select("self")
                                                .clone()
                                                .select("outPath")
                                                .into_expr::<String>(),
                                        )
                                        .as_expression(),
                                        NixValue::from("/system-source"),
                                    ),
                                );

                                NixValue::record([
                                    ("type", NixValue::from("nix-closure")),
                                    ("system", field_system),
                                    ("kernel", field(["name"], NixValue::from("6.12.64-1.fc41"))),
                                    (
                                        "root",
                                        field(
                                            ["sourceRef"],
                                            NixValue::record([
                                                (
                                                    "build",
                                                    NixValue::from(
                                                        "packages.x86_64-linux.nix-work-root-image",
                                                    ),
                                                ),
                                                ("subpath", NixValue::from("root.img")),
                                            ]),
                                        ),
                                    ),
                                    (
                                        "store",
                                        field(
                                            ["sourceRef", "build"],
                                            NixValue::from(
                                                "packages.x86_64-linux.nix-work-store-closure",
                                            ),
                                        ),
                                    ),
                                ])
                            };

                            NixValue::record([
                                ("backend", field_backend),
                                ("label", NixValue::from("blue")),
                                ("prefs", field(["memory"], NixValue::from(2048_i64))),
                            ])
                        };

                        NixValue::record([
                            ("test-qube", field_test_qube),
                            ("legacy-fedora", field_legacy_fedora),
                            ("nix-work", field_nix_work),
                        ])
                    };

                    let field_qrexec_policy = field(
                        ["rules"],
                        NixValue::list([NixValue::record([
                            ("service", NixValue::from("airlock.PolicyProbe")),
                            ("source", NixValue::from("dom0")),
                            ("target", NixValue::from("test-qube")),
                            ("action", NixValue::from("allow")),
                        ])]),
                    );

                    NixValue::record([
                        ("templates", field_templates),
                        ("kernels", field_kernels),
                        ("prefs", field(["default_kernel"], NixValue::from(""))),
                        ("domains", field_domains),
                        ("qrexecPolicy", field_qrexec_policy),
                    ])
                };

                topology
                    .clone()
                    .select_segments(["mkTopology"])
                    .call(argument_1)
            };

            field(["topology"], field_topology)
        };

        field(["airlock"], field_airlock)
    }
}
