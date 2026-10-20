//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for check system schema.
pub(super) fn live_activation(_inputs: NixValue) -> NixValue {
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
                        NixValue::from("/airlock/system-schema.nix"),
                    ))
                    .select("system")
                    .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                    .bind(system_schema_live_activation_body),
            )
        });

        field(["outputs"], field_outputs)
    }
}

/// Inspect evaluated configuration for check system schema.
pub(super) fn reboot_activation(_inputs: NixValue) -> NixValue {
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
                        NixValue::from("/airlock/system-schema.nix"),
                    ))
                    .select("system")
                    .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
                    .bind(system_schema_live_activation_2_body),
            )
        });

        field(["outputs"], field_outputs)
    }
}

/// Inspect evaluated configuration for check system schema.
pub(super) fn boot_activation(_inputs: NixValue) -> NixValue {
    field(
        ["outputs"],
        NixValue::function_attrs(["self"], |arguments_1| {
            system_schema_live_activation_3_body(arguments_1)
        }),
    )
}

/// Build the system schema check system schema contribution using the caller supplied deferred
/// values.
fn system_schema_live_activation_body(system_schema: NixValue) -> NixValue {
    {
        let field_airlock = {
            let field_system = system_schema.clone().select_segments(["mkSystem"]).call({
                let field_dom0 = {
                    let field_profile = NixValue::record([
                        (
                            "source",
                            field(
                                ["build"],
                                NixValue::from("packages.x86_64-linux.airlock-dom0-profile"),
                            ),
                        ),
                        (
                            "activationCommand",
                            NixValue::from("bin/airlock-dom0-profile-activate"),
                        ),
                        ("message", NixValue::from("hello")),
                    ]);

                    NixValue::record([
                        ("profile", field_profile),
                        (
                            "packages",
                            NixValue::list([field(
                                ["name"],
                                NixValue::from("airlock-dom0-profile"),
                            )]),
                        ),
                        (
                            "systemd",
                            field(
                                ["services", "airlock-example", "action"],
                                NixValue::from("restart"),
                            ),
                        ),
                    ])
                };

                field(["dom0"], field_dom0)
            });

            field(["system"], field_system)
        };

        field(["airlock"], field_airlock)
    }
}

/// Build the system schema check system schema 2 contribution using the caller supplied deferred
/// values.
fn system_schema_live_activation_2_body(system_schema: NixValue) -> NixValue {
    {
        let field_airlock = {
            let field_system = system_schema.clone().select_segments(["mkSystem"]).call({
                let field_dom0 = {
                    let field_nixos = {
                        let field_system = NixValue::record([
                            (
                                "source",
                                field(
                                    ["build"],
                                    NixValue::from(
                                        "nixosConfigurations.dom0.config.system.build.toplevel",
                                    ),
                                ),
                            ),
                            ("switchAction", NixValue::from("switch")),
                            ("reason", NixValue::from("new dom0 system generation")),
                        ]);

                        NixValue::record([
                            ("system", field_system),
                            ("kernel", field(["enable"], NixValue::from(true))),
                            ("xen", field(["enable"], NixValue::from(true))),
                            ("bootloader", field(["enable"], NixValue::from(true))),
                            ("qubes", field(["enable"], NixValue::from(true))),
                        ])
                    };

                    field(["nixos"], field_nixos)
                };

                field(["dom0"], field_dom0)
            });

            field(["system"], field_system)
        };

        field(["airlock"], field_airlock)
    }
}

/// Build the system schema check system schema 3 contribution using the caller supplied deferred
/// values.
fn system_schema_live_activation_3_body(
    arguments_1: NixValue,
) -> (Vec<(&'static str, NixValue)>, NixValue) {
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
                NixValue::from("/airlock/system-schema.nix"),
            ))
            .select("system")
            .call(arguments_1.clone().select("self.inputs.nixpkgs.lib"))
            .bind(|system_schema| {
                let field_airlock = {
                    let field_system = system_schema.clone().select_segments(["mkSystem"]).call(
                        field(
                            ["dom0", "nixos", "system"],
                            NixValue::record([
                                (
                                    "source",
                                    field(
                                        ["build"],
                                        NixValue::from(
                                            "nixosConfigurations.dom0.config.system.build.toplevel",
                                        ),
                                    ),
                                ),
                                ("switchAction", NixValue::from("boot")),
                                (
                                    "reason",
                                    NixValue::from("install next dom0 boot generation"),
                                ),
                            ]),
                        ),
                    );

                    field(["system"], field_system)
                };

                field(["airlock"], field_airlock)
            }),
    )
}
