//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test pool admission uses narrow root receiver.
pub(super) fn admission_receiver(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(|flake| {
            flake
                .clone()
                .select_segments(["nixosConfigurations", "vm-pool-admission-test", "config"])
                .bind(|image| {
                    recursive(|bindings_1| {
                        pool_admission_module_admission_receiver_body(bindings_1, image.clone())
                    })
                    .bind(|bindings_1| {
                        pool_admission_module_admission_receiver_2(bindings_1, image.clone())
                    })
                })
        })
}

/// Build the pool admission module test pool admission uses narrow root receiver contribution
/// using the caller supplied deferred values.
fn pool_admission_module_admission_receiver_body(
    bindings_1: NixValue,
    image: NixValue,
) -> NixValue {
    {
        let field_rules = NixValue::builtin("filter")
            .call(NixValue::function(|rule| {
                NixValue::builtin("any")
                    .call(NixValue::function(|entry| {
                        !NixValue::builtin("match")
                            .call(NixValue::from(".*airlock-pool-admit-root.*"))
                            .call(entry.clone().select_segments(["command"]))
                            .equals(NixValue::null())
                    }))
                    .call(rule.clone().select_segments(["commands"]))
            }))
            .call(
                image
                    .clone()
                    .select_segments(["security", "sudo", "extraRules"]),
            );

        NixValue::record([
            ("rules", field_rules),
            (
                "rule",
                NixValue::builtin("head").call(bindings_1.clone().select("rules").clone()),
            ),
            (
                "command",
                NixValue::builtin("head")
                    .call(
                        bindings_1
                            .clone()
                            .select("rule")
                            .clone()
                            .select_segments(["commands"]),
                    )
                    .select_segments(["command"]),
            ),
        ])
    }
}

/// Build the pool admission module test pool admission uses narrow root receiver contribution
/// using the caller supplied deferred values.
fn pool_admission_module_admission_receiver_2(bindings_1: NixValue, image: NixValue) -> NixValue {
    {
        let field_receivers = NixValue::builtin("map")
            .call(NixValue::function(|entry| {
                native_contract(entry.clone().select_segments(["command"]))
            }))
            .call(
                bindings_1
                    .clone()
                    .select("rule")
                    .clone()
                    .select_segments(["commands"]),
            );

        let field_options = NixValue::builtin("map")
            .call(NixValue::function(|entry| {
                entry.clone().select_segments(["options"])
            }))
            .call(
                bindings_1
                    .clone()
                    .select("rule")
                    .clone()
                    .select_segments(["commands"]),
            );

        let field_policy = NixValue::builtin("readFile").call(NixValue::concat_text([
            NixValue::builtin("head")
                .call(
                    NixValue::builtin("filter")
                        .call(NixValue::function(|p| {
                            NixValue::builtin("pathExists").call(NixValue::concat_text([
                                p.clone().to_text(),
                                NixValue::from(
                                    "/etc/qubes/policy.d/42-airlock-pool-admission.policy",
                                ),
                            ]))
                        }))
                        .call(image.clone().select_segments([
                            "virtualisation",
                            "qubes",
                            "packages",
                        ])),
                )
                .to_text(),
            NixValue::from("/etc/qubes/policy.d/42-airlock-pool-admission.policy"),
        ]));

        NixValue::record([
            (
                "handler",
                native_contract(image.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes-rpc/airlock.Pool.Admit",
                    "source",
                ])),
            ),
            (
                "rootReceiver",
                native_contract(bindings_1.clone().select("command").clone()),
            ),
            (
                "incrementalHandler",
                native_contract(image.clone().select_segments([
                    "environment",
                    "etc",
                    "qubes-rpc/airlock.Pool.AdmitIncremental",
                    "source",
                ])),
            ),
            ("receivers", field_receivers),
            ("options", field_options),
            ("policy", field_policy),
            (
                "sudoUsers",
                bindings_1
                    .clone()
                    .select("rule")
                    .clone()
                    .select_segments(["users"]),
            ),
            (
                "sudoOptions",
                NixValue::builtin("head")
                    .call(
                        bindings_1
                            .clone()
                            .select("rule")
                            .clone()
                            .select_segments(["commands"]),
                    )
                    .select_segments(["options"]),
            ),
            (
                "matchingRuleCount",
                NixValue::builtin("length").call(bindings_1.clone().select("rules").clone()),
            ),
            (
                "targets",
                image
                    .clone()
                    .select_segments(["airlock", "poolAdmission", "targets"]),
            ),
        ])
    }
}
