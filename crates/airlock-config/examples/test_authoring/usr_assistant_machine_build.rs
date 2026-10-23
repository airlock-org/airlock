//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for build script.
pub(super) fn build_script(inputs: NixValue) -> NixValue {
    NixValue::builtin("listToAttrs")
        .call(
            NixValue::builtin("map")
                .call(NixValue::function(|name| {
                    NixValue::record([("name", name.clone()), ("value", NixValue::from("/tools"))])
                }))
                .call(NixValue::list([
                    NixValue::from("coreutils"),
                    NixValue::from("gitMinimal"),
                    NixValue::from("jq"),
                    NixValue::from("gawk"),
                    NixValue::from("gnugrep"),
                ])),
        )
        .bind(|tools| usr_assistant_machine_build_build_script_2(tools, inputs.clone()))
}

/// Build the usr assistant machine build build script contribution using the caller supplied
/// deferred values.
fn usr_assistant_machine_build_build_script_body(apps: NixValue, tools: NixValue) -> NixValue {
    {
        let receiver = NixValue::builtin("import")
            .call(add(
                apps.clone(),
                NixValue::from("/airlock-config/generated/modules.nix"),
            ))
            .select_segments(["modules"])
            .call(NixValue::null())
            .select_segments(["assistantBuild"])
            .call({
                let field_config = field(
                    ["airlock", "qubeGuest"],
                    NixValue::record([
                        ("user", NixValue::from("user")),
                        ("configNixStoreRoot", NixValue::from("/unused")),
                        (
                            "configRepoPath",
                            NixValue::from("/home/user/airlockos-machine"),
                        ),
                    ]),
                );

                NixValue::record([
                    ("config", field_config),
                    (
                        "lib",
                        field(["escapeShellArg"], NixValue::function(|x| x.clone())),
                    ),
                    (
                        "pkgs",
                        tools.clone().merge_attrs(field(
                            ["writeShellScriptBin"],
                            NixValue::function(|_name| NixValue::function(|text| text.clone())),
                        )),
                    ),
                ])
            });

        receiver.bind(|module| {
            NixValue::builtin("elemAt")
                .call(
                    module
                        .clone()
                        .select_segments(["config", "environment", "systemPackages"]),
                )
                .call(NixValue::from(1_i64))
        })
    }
}

/// Build the usr assistant machine build build script contribution using the caller supplied
/// deferred values.
fn usr_assistant_machine_build_build_script_2(tools: NixValue, inputs: NixValue) -> NixValue {
    {
        let condition = NixValue::builtin("pathExists").call(
            NixPath::from_source_text(
                NixValue::concat_text([
                    NixValue::from("/"),
                    inputs.clone().select("root_parent").to_text(),
                    NixValue::from("/crates/airlock-config/generated/modules.nix"),
                ])
                .into_expr::<String>(),
            )
            .as_expression(),
        );

        let when_true = NixPath::from_source_text(
            NixValue::concat_text([
                NixValue::from("/"),
                inputs.clone().select("root_parent2").to_text(),
                NixValue::from("/crates"),
            ])
            .into_expr::<String>(),
        )
        .as_expression();

        NixValue::if_else(
            condition,
            when_true,
            NixValue::builtin("getFlake")
                .call(NixValue::concat_text([
                    NixValue::from("path:"),
                    inputs.clone().select("root").to_text(),
                ]))
                .select_segments(["inputs", "airlock-apps"]),
        )
    }
    .bind(|apps| usr_assistant_machine_build_build_script_body(apps, tools.clone()))
}
