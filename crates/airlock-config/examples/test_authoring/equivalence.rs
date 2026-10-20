//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Describe the equivalence evaluation boundary using caller-supplied source trees.
pub(super) fn expression() -> NixValue {
    NixValue::function_attrs(
        ["generated", "machine", "nixpkgs", "overrides"],
        |arguments_1| {
            (
                vec![("overrides", NixValue::from("{}"))],
                NixValue::function_attrs(
                    [
                        "system",
                        "networkDebug",
                        "omitDebug",
                        "kernel",
                        "contextKernel",
                        "forceWantedBy",
                        "poisonForce",
                        "selection",
                    ],
                    |arguments_2| equivalence_expression_5(arguments_2, arguments_1.clone()),
                )
                .call(
                    NixValue::builtin("fromJSON")
                        .call(arguments_1.clone().select("overrides").clone()),
                ),
            )
        },
    )
}

/// Build the equivalence expression contribution using the caller supplied deferred values.
fn equivalence_expression_body(builders: NixValue, arguments_2: NixValue) -> NixValue {
    NixValue::record([
        (
            "sys-net",
            builders
                .clone()
                .select_segments(["sys-net"])
                .call(arguments_2.clone().select("system").clone()),
        ),
        (
            "sys-firewall",
            builders
                .clone()
                .select_segments(["sys-firewall"])
                .call(arguments_2.clone().select("system").clone()),
        ),
        (
            "usr-home",
            builders
                .clone()
                .select_segments(["usr-home"])
                .call(arguments_2.clone().select("system").clone()),
        ),
    ])
    .bind(|entries| {
        NixValue::if_else(
            arguments_2
                .clone()
                .select("selection")
                .clone()
                .equals(NixValue::from("driver-name")),
            entries.clone().select_segments(["sys-net", "name"]),
            NixValue::if_else(
                arguments_2
                    .clone()
                    .select("selection")
                    .clone()
                    .equals(NixValue::from("firewall-name")),
                entries.clone().select_segments(["sys-firewall", "name"]),
                NixValue::record([
                    ("entries", entries.clone()),
                    (
                        "kernelContext",
                        NixValue::builtin("getContext")
                            .call(entries.clone().select_segments(["usr-home", "kernelName"])),
                    ),
                ]),
            ),
        )
    })
}

/// Build the equivalence expression contribution using the caller supplied deferred values.
fn equivalence_expression_2(module: NixValue, lib: NixValue) -> NixValue {
    {
        let receiver = {
            let argument_1 = {
                let field_modules = {
                    let argument_1 = [
                        {
                            let field_options = {
                                let field_airlock = {
                                    let field_app_gui = field(
                                        ["scale"],
                                        lib.clone().select_segments(["mkOption"]).call(
                                            NixValue::record([
                                                (
                                                    "type",
                                                    lib.clone().select_segments([
                                                        "types", "ints", "positive",
                                                    ]),
                                                ),
                                                ("default", NixValue::from(1_i64)),
                                            ]),
                                        ),
                                    );

                                    let field_qube_guest = field(
                                        ["allowQrexecUserSelection"],
                                        lib.clone().select_segments(["mkOption"]).call(
                                            NixValue::record([
                                                (
                                                    "type",
                                                    lib.clone().select_segments(["types", "bool"]),
                                                ),
                                                ("default", NixValue::from(false)),
                                            ]),
                                        ),
                                    );

                                    NixValue::record([
                                        ("appGui", field_app_gui),
                                        ("qubeGuest", field_qube_guest),
                                    ])
                                };

                                let field_systemd = {
                                    let list_type = lib
                                        .clone()
                                        .select("types.listOf")
                                        .call(lib.clone().select("types.str"));
                                    let wanted_by =
                                        lib.clone().select("mkOption").call(NixValue::record([
                                            ("type", list_type),
                                            ("default", NixValue::list([])),
                                        ]));
                                    let service = field(["options", "wantedBy"], wanted_by);
                                    let service_type =
                                        lib.clone().select("types.submodule").call(service);
                                    let attributes =
                                        lib.clone().select("types.attrsOf").call(service_type);
                                    let field_services = lib
                                        .clone()
                                        .select("mkOption")
                                        .call(field(["type"], attributes));

                                    field(["services"], field_services)
                                };

                                NixValue::record([
                                    (
                                        "networking",
                                        field(
                                            ["hostName"],
                                            lib.clone().select_segments(["mkOption"]).call(field(
                                                ["type"],
                                                lib.clone().select_segments(["types", "str"]),
                                            )),
                                        ),
                                    ),
                                    ("airlock", field_airlock),
                                    ("systemd", field_systemd),
                                ])
                            };

                            field(["options"], field_options)
                        },
                        field(
                            ["systemd", "services", "NetworkManager", "wantedBy"],
                            NixValue::list([NixValue::from("multi-user.target")]),
                        ),
                        module.clone(),
                    ];

                    NixValue::list(argument_1)
                };

                field(["modules"], field_modules)
            };

            lib.clone()
                .select_segments(["evalModules"])
                .call(argument_1)
        };

        receiver.select_segments(["config"])
    }
}

/// Build the equivalence expression contribution using the caller supplied deferred values.
fn equivalence_expression_3(
    bindings_3: NixValue,
    arguments_1: NixValue,
    arguments_2: NixValue,
    caller_lib: NixValue,
    inline_config: NixValue,
    lib: NixValue,
) -> NixValue {
    {
        let field_record = NixValue::function(|builder_system| {
            NixValue::function(|arguments| {
                NixValue::builtin("removeAttrs")
                    .call(arguments.clone())
                    .call(NixValue::list([NixValue::from("modules")]))
                    .merge_attrs(NixValue::record([
                        ("builderSystem", builder_system.clone()),
                        (
                            "modules",
                            NixValue::list([NixValue::record([
                                ("kind", NixValue::from("inline")),
                                (
                                    "config",
                                    inline_config.clone().call(
                                        NixValue::builtin("head")
                                            .call(arguments.clone().select_segments(["modules"])),
                                    ),
                                ),
                            ])]),
                        ),
                    ]))
            })
        });

        let field_arguments = {
            let field_airlock_qubes_vm_kernel_name = NixValue::if_else(
                arguments_2.clone().select("contextKernel").clone(),
                NixValue::builtin("toFile")
                    .call(NixValue::from("qubes-kernel-name"))
                    .call(arguments_2.clone().select("kernel").clone()),
                arguments_2.clone().select("kernel").clone(),
            );

            NixValue::record([
                ("lib", caller_lib.clone()),
                (
                    "mkAirlockQubeBuilders",
                    NixValue::function(|builder_system| {
                        field(
                            ["mkNixNativeDomain"],
                            bindings_3
                                .clone()
                                .select("record")
                                .clone()
                                .call(builder_system.clone()),
                        )
                    }),
                ),
                (
                    "airlockQubesVmKernelName",
                    field_airlock_qubes_vm_kernel_name,
                ),
            ])
        }
        .merge_attrs(
            lib.clone()
                .select_segments(["optionalAttrs"])
                .call(!arguments_2.clone().select("omitDebug").clone())
                .call(field(
                    ["networkDebug"],
                    arguments_2.clone().select("networkDebug").clone(),
                )),
        );

        let field_candidate = NixValue::builtin("import")
            .call(arguments_1.clone().select("generated").clone())
            .select_segments(["builders"])
            .call(
                bindings_3
                    .clone()
                    .select("arguments")
                    .clone()
                    .merge_attrs(field(
                        ["airlockSupport"],
                        NixValue::if_else(
                            arguments_2
                                .clone()
                                .select("selection")
                                .clone()
                                .equals(NixValue::from("driver-name")),
                            NixValue::builtin("throw")
                                .call(NixValue::from("unused personal module source was forced")),
                            arguments_1.clone().select("machine").clone(),
                        ),
                    )),
            );

        NixValue::record([
            ("record", field_record),
            ("arguments", field_arguments),
            ("candidate", field_candidate),
        ])
    }
}

/// Build the equivalence expression contribution using the caller supplied deferred values.
fn equivalence_expression_4(
    inspect: NixValue,
    arguments_1: NixValue,
    arguments_2: NixValue,
    lib: NixValue,
) -> NixValue {
    {
        let receiver = lib.clone().merge_attrs({
            let field_mk_force = NixValue::if_else(
                arguments_2.clone().select("poisonForce").clone(),
                NixValue::builtin("throw").call(NixValue::from("excluded debug branch was forced")),
                NixValue::function(|value| {
                    lib.clone()
                        .select_segments(["mkForce"])
                        .call(NixValue::concat_lists([
                            arguments_2.clone().select("forceWantedBy").clone(),
                            value.clone(),
                        ]))
                }),
            );

            NixValue::record([
                (
                    "mkDefault",
                    NixValue::function(|_| {
                        NixValue::builtin("throw")
                            .call(NixValue::from("unexpected use of the outer mkDefault"))
                    }),
                ),
                ("mkForce", field_mk_force),
            ])
        });

        receiver.bind(|caller_lib| {
            NixValue::function(|module| equivalence_expression_2(module, lib.clone())).bind(
                |inline_config| {
                    recursive(|bindings_3| {
                        equivalence_expression_3(
                            bindings_3,
                            arguments_1.clone(),
                            arguments_2.clone(),
                            caller_lib.clone(),
                            inline_config.clone(),
                            lib.clone(),
                        )
                    })
                    .bind(|bindings_3| {
                        field(
                            ["candidate"],
                            inspect
                                .clone()
                                .call(bindings_3.clone().select("candidate").clone()),
                        )
                    })
                },
            )
        })
    }
}

/// Build the equivalence expression contribution using the caller supplied deferred values.
fn equivalence_expression_5(
    arguments_2: NixValue,
    arguments_1: NixValue,
) -> (Vec<(&'static str, NixValue)>, NixValue) {
    (
        vec![
            ("system", NixValue::from("x86_64-linux")),
            ("networkDebug", NixValue::from(false)),
            ("omitDebug", NixValue::from(false)),
            ("kernel", NixValue::from("6.12.64-1.fc41")),
            ("contextKernel", NixValue::from(false)),
            ("forceWantedBy", NixValue::list([])),
            ("poisonForce", NixValue::from(false)),
            ("selection", NixValue::from("all")),
        ],
        NixValue::builtin("import")
            .call(add(
                arguments_1.clone().select("nixpkgs").clone(),
                NixValue::from("/lib"),
            ))
            .bind(|lib| {
                NixValue::function(|builders| {
                    equivalence_expression_body(builders, arguments_2.clone())
                })
                .bind(|inspect| {
                    equivalence_expression_4(
                        inspect,
                        arguments_1.clone(),
                        arguments_2.clone(),
                        lib.clone(),
                    )
                })
            }),
    )
}
