//! Verify bootstrap export keeps extra arguments and source-owned modules.

use super::*;

/// Inspect exported custom modules through a recording builder.
pub(super) fn expression(inputs: NixValue) -> NixValue {
    {
        let receiver = {
            let receiver = {
                let receiver = {
                    let argument_1 = {
                        let field_lib = NixValue::builtin("import").call(
                            NixPath::from_source_text(
                                NixValue::concat_text([
                                    NixValue::from("/"),
                                    inputs.clone().select("nixpkgs").to_text(),
                                    NixValue::from("/lib"),
                                ])
                                .into_expr::<String>(),
                            )
                            .as_expression(),
                        );

                        let field_modules_directory = NixPath::from_source_text(
                            NixValue::concat_text([
                                NixValue::from("/"),
                                inputs.clone().select("output").to_text(),
                                NixValue::from("/generated/bootstrap"),
                            ])
                            .into_expr::<String>(),
                        )
                        .as_expression();

                        NixValue::record([
                            ("lib", field_lib),
                            (
                                "mkAirlockQubeBuilders",
                                NixValue::function(|_| {
                                    field(
                                        ["mkNixNativeDomain"],
                                        NixValue::function(|args| args.clone()),
                                    )
                                }),
                            ),
                            ("airlockQubesVmKernelName", NixValue::from("test-kernel")),
                            (
                                "airlockSupport",
                                NixPath::from_source_text(
                                    NixValue::concat_text([
                                        NixValue::from("/"),
                                        inputs.clone().select("machine").to_text(),
                                    ])
                                    .into_expr::<String>(),
                                )
                                .as_expression(),
                            ),
                            (
                                "airlockApps",
                                NixPath::from_source_text(
                                    NixValue::concat_text([
                                        NixValue::from("/"),
                                        inputs.clone().select("root").to_text(),
                                        NixValue::from("/crates"),
                                    ])
                                    .into_expr::<String>(),
                                )
                                .as_expression(),
                            ),
                            ("modulesDirectory", field_modules_directory),
                            (
                                "extraVmSpecialArgs",
                                field(["token"], NixValue::from("kept")),
                            ),
                        ])
                    };

                    NixValue::builtin("import")
                        .call(
                            NixPath::from_source_text(
                                NixValue::concat_text([
                                    NixValue::from("/"),
                                    inputs.clone().select("output").to_text(),
                                    NixValue::from("/generated/bootstrap/default.nix"),
                                ])
                                .into_expr::<String>(),
                            )
                            .as_expression(),
                        )
                        .select_segments(["builders"])
                        .call(argument_1)
                };

                receiver.select_segments(["sys-ctrl"])
            };

            receiver.call(NixValue::from("x86_64-linux"))
        };

        receiver.bind(|entry| {
            let field_module_exists = NixValue::builtin("pathExists").call(
                NixValue::builtin("elemAt")
                    .call(entry.clone().select_segments(["modules"]))
                    .call(
                        NixValue::builtin("sub")
                            .call(
                                NixValue::builtin("length")
                                    .call(entry.clone().select_segments(["modules"])),
                            )
                            .call(NixValue::from(1_i64)),
                    ),
            );

            NixValue::record([
                (
                    "token",
                    entry.clone().select_segments(["extraSpecialArgs", "token"]),
                ),
                ("moduleExists", field_module_exists),
            ])
        })
    }
}
