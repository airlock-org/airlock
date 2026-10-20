#![cfg(feature = "evaluation")]

//! Generic library helpers preserve the caller's functions and deferred value semantics.
use rusix::interop::raw::NixRepresentation;
use rusix::interop::raw::{NixFunctionExt, NixpkgsExt};
use rusix::{
    Config, Expr,
    interop::{InputRef, NixLibrary, NixList, Nixpkgs, Package, raw::NixValue},
    nix_text,
};
use rusix::{NixSession, compile};
use std::path::Path;

#[rusix::args]
mod args {
    use rusix::interop::raw::NixValue;

    #[rusix(root)]
    struct Inputs {
        lib: NixValue,
        enabled: bool,
        number: i64,
        text: String,
    }
}

fn evaluate(value: NixValue) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", value)).unwrap())
        .unwrap()
        .value["result"]
        .clone()
}

#[test]
fn standard_helpers_handle_concrete_values_empty_lists_and_false_conditions() {
    let lib = Nixpkgs::new().library();
    let values = NixValue::list([1_i64.into(), 2_i64.into()]);

    assert_eq!(
        evaluate(NixValue::record([
            ("optional", lib.optional(true, 42_i64).into()),
            ("absent", lib.optional(false, 42_i64).into()),
            (
                "optionals",
                lib.optionals(true, NixList::<NixValue>::from_expression(values.clone()))
                    .into()
            ),
            (
                "noOptionals",
                lib.optionals(false, NixList::<NixValue>::from_expression(values.clone()))
                    .into()
            ),
            ("text", lib.optional_text(true, "exact\n text").into()),
            ("noText", lib.optional_text(false, "unused").into()),
            ("all", lib.all([true, true]).into()),
            ("notAll", lib.all([true, false]).into()),
            ("emptyAll", lib.all([] as [bool; 0]).into()),
            (
                "lists",
                lib.concat_lists(
                    [values, NixValue::list([3_i64.into()])]
                        .map(NixList::<NixValue>::from_expression)
                )
                .into()
            ),
            ("emptyLists", lib.concat_lists::<NixValue>([]).into()),
            (
                "nativeLists",
                NixValue::concat_lists([
                    NixValue::list([]),
                    NixValue::list([1_i64.into(), 2_i64.into()]),
                    NixValue::list([3_i64.into()]),
                ]),
            ),
            ("emptyNativeLists", NixValue::concat_lists([])),
            ("newerVersion", lib.version_at_least("10.10", "10.9").into()),
            ("equalVersion", lib.version_at_least("10.9", "10.9").into()),
            ("olderVersion", lib.version_at_least("10.9", "10.10").into()),
            ("versionOlder", lib.version_older("10.9", "10.10").into()),
            ("versionNotOlder", lib.version_older("10.10", "10.9").into()),
            (
                "versionEqualNotOlder",
                lib.version_older("10.9", "10.9").into()
            ),
            (
                "replacedText",
                lib.replace_text("ab a", [("ab", "a"), ("a", "b")]).into()
            ),
            ("not", !NixValue::from(true)),
            ("typedNot", (!Expr::boolean(false)).into()),
            (
                "curried",
                lib.as_expression()
                    .clone()
                    .select("concatStringsSep")
                    .apply(["/".into(), NixValue::list(["a".into(), "b".into()])])
            ),
        ])),
        serde_json::json!({
            "optional": [42], "absent": [], "optionals": [1, 2], "noOptionals": [],
            "text": "exact\n text", "noText": "", "all": true, "notAll": false,
            "emptyAll": true, "lists": [1, 2, 3], "emptyLists": [],
            "nativeLists": [1, 2, 3], "emptyNativeLists": [],
            "newerVersion": true, "equalVersion": true, "olderVersion": false,
            "versionOlder": true, "versionNotOlder": false, "versionEqualNotOlder": false,
            "replacedText": "a b",
            "not": false, "typedNot": true, "curried": "a/b",
        }),
    );
}

#[test]
fn library_helpers_accept_typed_symbolic_argument_values() {
    let factory = NixValue::function_attrs(["lib", "enabled", "number", "text"], |value| {
        let args = args::from_value(value);
        let lib = NixLibrary::from_expression(args.lib());
        let _: Expr<bool> = !args.enabled();
        let conjunction = args.enabled().and(args.enabled());
        let items = NixValue::list([args.number().into()]);

        (
            Vec::<(&str, NixValue)>::new(),
            NixValue::record([
                (
                    "optional",
                    lib.optional(args.enabled(), args.number()).into(),
                ),
                (
                    "optionals",
                    lib.optionals(
                        args.enabled(),
                        NixList::<NixValue>::from_expression(items.clone()),
                    )
                    .into(),
                ),
                (
                    "text",
                    lib.optional_text(args.enabled(), args.text()).into(),
                ),
                (
                    "validated",
                    lib.throw_if_not(args.enabled(), "disabled", args.number())
                        .into(),
                ),
                ("all", lib.all([args.enabled(), args.enabled()]).into()),
                (
                    "lists",
                    lib.concat_lists(
                        [items.clone(), items].map(NixList::<NixValue>::from_expression),
                    )
                    .into(),
                ),
                ("not", (!args.enabled()).into()),
                ("and", conjunction.into()),
            ]),
        )
    });

    assert_eq!(
        evaluate(factory.call(NixValue::record([
            ("lib", Nixpkgs::new().value("lib")),
            ("enabled", true.into()),
            ("number", 42_i64.into()),
            ("text", "deferred".into()),
        ]))),
        serde_json::json!({
            "optional": [42], "optionals": [42], "text": "deferred", "validated": 42, "all": true,
            "lists": [42, 42], "not": false, "and": true,
        }),
    );
}

#[test]
fn standard_conditionals_and_all_leave_excluded_or_short_circuited_values_lazy() {
    let lib = Nixpkgs::new().library();
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();

    assert_eq!(
        evaluate(NixValue::record([
            ("optional", lib.optional(false, failure.clone()).into()),
            (
                "optionals",
                lib.optionals(false, NixList::<NixValue>::from_expression(failure.clone()))
                    .into()
            ),
            (
                "text",
                lib.optional_text(false, failure.clone().into_expr::<String>())
                    .into()
            ),
            ("not", lib.optional(false, !failure.clone()).into()),
            (
                "typedNot",
                lib.optional(false, !failure.clone().into_expr::<bool>())
                    .into()
            ),
            (
                "all",
                lib.all([false.into(), failure.clone().into_expr::<bool>()])
                    .into()
            ),
            (
                "lists",
                lib.concat_lists([lib.optional(false, failure.clone())])
                    .into()
            ),
            (
                "headOnly",
                lib.as_expression()
                    .clone()
                    .select("length")
                    .apply([lib.optional(true, failure.clone()).into()])
            ),
            (
                "nativeHeadOnly",
                NixValue::builtin("length")
                    .call(NixValue::concat_lists([NixValue::list([failure.clone()])]))
            ),
        ])),
        serde_json::json!({
            "optional": [], "optionals": [], "text": "", "not": [], "typedNot": [], "all": false,
            "lists": [], "headOnly": 1, "nativeHeadOnly": 1,
        }),
    );

    // concatLists must not force individual elements merely to construct the list.
    assert_eq!(
        evaluate(
            lib.as_expression().clone().select("length").apply([lib
                .concat_lists([NixValue::list([failure])].map(NixList::<NixValue>::from_expression))
                .into()])
        ),
        1,
    );
}

fn replacement(name: &'static str) -> NixValue {
    NixValue::function(move |condition| {
        NixValue::function(move |value| {
            NixValue::record([
                ("function", name.into()),
                ("condition", condition),
                ("value", value),
            ])
        })
    })
}

#[test]
fn every_library_helper_uses_the_supplied_record_including_overridden_functions() {
    let library = NixValue::record([
        ("optional", replacement("optional")),
        ("optionals", replacement("optionals")),
        ("optionalString", replacement("optionalString")),
        (
            "all",
            NixValue::function(|predicate| {
                NixValue::function(move |conditions| {
                    NixValue::record([
                        ("function", "all".into()),
                        ("identityResult", predicate.call(true)),
                        ("conditions", conditions),
                    ])
                })
            }),
        ),
        (
            "concatLists",
            NixValue::function(|lists| {
                NixValue::record([("function", "concatLists".into()), ("lists", lists)])
            }),
        ),
        (
            "throwIfNot",
            NixValue::function(|condition| {
                NixValue::function(move |message| {
                    NixValue::function(move |value| {
                        NixValue::record([
                            ("condition", condition),
                            ("message", message),
                            ("value", value),
                        ])
                    })
                })
            }),
        ),
        ("custom", replacement("custom")),
        ("versionAtLeast", replacement("versionAtLeast")),
        ("versionOlder", replacement("versionOlder")),
        (
            "getDev",
            NixValue::function(|package| NixValue::record([("dev", package)])),
        ),
        (
            "getLib",
            NixValue::function(|package| NixValue::record([("lib", package)])),
        ),
        (
            "replaceStrings",
            NixValue::function(|from| {
                NixValue::function(|to| {
                    NixValue::function(|text| {
                        NixValue::record([("from", from), ("to", to), ("text", text)])
                    })
                })
            }),
        ),
    ]);
    let lib = NixLibrary::from_expression(library);
    let values = NixValue::list([42_i64.into()]);

    assert_eq!(
        evaluate(NixValue::record([
            ("optional", lib.optional(false, 42_i64).into()),
            (
                "optionals",
                lib.optionals(false, NixList::<NixValue>::from_expression(values.clone()))
                    .into()
            ),
            ("text", lib.optional_text(false, "caller text").into()),
            ("all", lib.all([false]).into()),
            (
                "lists",
                lib.concat_lists([values.clone()].map(NixList::<NixValue>::from_expression))
                    .into()
            ),
            ("nativeLists", NixValue::concat_lists([values])),
            ("nativeAnd", NixValue::from(true).and(false)),
            ("version", lib.version_at_least("10.9", "10.10").into()),
            ("older", lib.version_older("10.9", "10.10").into()),
            (
                "dev",
                lib.get_dev(Package::from_expression(NixValue::from(42_i64)))
                    .into()
            ),
            (
                "lib",
                lib.get_lib(Package::from_expression(NixValue::from(42_i64)))
                    .into()
            ),
            ("replaced", lib.replace_text("a.b", [(".", "_")]).into()),
            (
                "nativeReplacement",
                NixValue::from("a.b").replace_text([(".", "_")])
            ),
            (
                "validated",
                lib.throw_if_not(false, "caller message", 7_i64).into()
            ),
            (
                "custom",
                lib.as_expression()
                    .clone()
                    .select("custom")
                    .apply([true.into(), 7_i64.into()])
            ),
        ])),
        serde_json::json!({
            "optional": {"function":"optional", "condition":false, "value":42},
            "optionals": {"function":"optionals", "condition":false, "value":[42]},
            "text": {"function":"optionalString", "condition":false, "value":"caller text"},
            "all": {"function":"all", "identityResult":true, "conditions":[false]},
            "lists": {"function":"concatLists", "lists":[[42]]},
            "nativeLists": [42], "nativeAnd": false,
            "version": {"function":"versionAtLeast", "condition":"10.9", "value":"10.10"},
            "older": {"function":"versionOlder", "condition":"10.9", "value":"10.10"},
            "dev": {"dev":42}, "lib": {"lib":42},
            "replaced": {"from":["."], "to":["_"], "text":"a.b"},
            "nativeReplacement":"a_b",
            "validated": {"condition":false, "message":"caller message", "value":7},
            "custom": {"function":"custom", "condition":true, "value":7},
        }),
    );
}

#[test]
fn helper_failures_capture_the_public_call_site_and_keep_the_nix_trace() {
    let lib = NixLibrary::from_expression(NixValue::record([] as [(&str, NixValue); 0]));
    let lookup_line = line!() + 1;
    let missing = lib.as_expression().clone().select("missing").call(true);
    let supplied = Package::from_expression(NixValue::from(42_i64));
    let empty = NixList::<NixValue>::new([]);
    let cases = [
        (missing, lookup_line),
        (lib.optional(true, "value").into(), line!()),
        (lib.optionals(true, empty).into(), line!()),
        (lib.optional_text(true, "text").into(), line!()),
        (lib.all([true]).into(), line!()),
        (lib.concat_lists::<NixValue>([]).into(), line!()),
        (lib.version_at_least("10.9", "10.10").into(), line!()),
        (lib.version_older("10.9", "10.10").into(), line!()),
        (lib.get_dev(supplied.clone()).into(), line!()),
        (lib.get_lib(supplied).into(), line!()),
        (lib.replace_text("a", [("a", "b")]).into(), line!()),
        (lib.throw_if_not(true, "unused", 42_i64).into(), line!()),
    ];

    for (value, line) in cases {
        let diagnostic = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(Config::new().set_dynamic("result", value)).unwrap())
            .unwrap_err();
        let origin = diagnostic.primary.as_ref().unwrap();
        assert_eq!(origin.line, line);
        assert_eq!(origin.file, file!());
        assert!(diagnostic.reason.contains("missing"));
        assert!(!diagnostic.raw_nix.is_empty());
    }
}

#[test]
fn child_expression_failures_remain_more_precise_than_helper_boundaries() {
    let lib = Nixpkgs::new().library();
    let line = line!() + 1;
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let values = [
        lib.optional(true, failure.clone()).into(),
        lib.optionals(true, NixList::<NixValue>::from_expression(failure.clone()))
            .into(),
        lib.optional_text(true, failure.clone().into_expr::<String>())
            .into(),
        lib.all([failure.clone().into_expr::<bool>()]).into(),
        lib.concat_lists(
            [NixValue::list([failure.clone()])].map(NixList::<NixValue>::from_expression),
        )
        .into(),
        NixValue::concat_lists([NixValue::list([failure.clone()])]),
        lib.version_at_least(failure.clone().into_expr::<String>(), "10.10")
            .into(),
        lib.version_at_least("10.9", failure.clone().into_expr::<String>())
            .into(),
        lib.version_older(failure.clone().into_expr::<String>(), "10.10")
            .into(),
        lib.version_older("10.9", failure.clone().into_expr::<String>())
            .into(),
        lib.get_dev(Package::from_expression(failure.clone()))
            .into(),
        lib.get_lib(Package::from_expression(failure.clone()))
            .into(),
        lib.replace_text(failure.clone().into_expr::<String>(), [("a", "b")])
            .into(),
        NixValue::from(true).and(failure.clone()),
        NixValue::from(false).or(failure.clone()),
        NixValue::from(true).implies(failure.clone()),
        lib.throw_if_not(true, "unused", failure.clone()),
        lib.throw_if_not(failure.clone().into_expr::<bool>(), "unused", 42_i64)
            .into(),
        !failure.clone(),
        (!failure.into_expr::<bool>()).into(),
    ];

    for value in values {
        let diagnostic = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(Config::new().set_dynamic("result", value)).unwrap())
            .unwrap_err();
        assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
        assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
        assert_eq!(diagnostic.reason, "division by zero");
        assert!(!diagnostic.raw_nix.is_empty());
    }
}

#[test]
fn throw_if_not_preserves_lazy_branches_and_reports_the_validation_call() {
    let lib = Nixpkgs::new().library();
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();

    // A successful check does not evaluate the failure message.
    assert_eq!(
        evaluate(
            lib.throw_if_not(true, failure.clone().into_expr::<String>(), 42_i64)
                .into()
        ),
        42
    );
    let line = line!() + 1;
    let rejected = lib.throw_if_not(false, "feature combination rejected", failure);

    // Constructing a rejected expression does not force it if Nix never uses it.
    assert_eq!(
        evaluate(
            NixValue::record([("good", 42_i64.into()), ("bad", rejected.clone())]).select("good")
        ),
        42,
    );

    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", rejected)).unwrap())
        .unwrap_err();
    assert!(diagnostic.reason.contains("feature combination rejected"));
    assert!(!diagnostic.reason.contains("division by zero"));
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
    assert!(!diagnostic.raw_nix.is_empty());
}

// A test-only reproduction of Git's three-rule composition, not a public helper.
fn ordered_guards(
    lib: &NixLibrary,
    checks: [(NixValue, NixValue); 3],
    value: NixValue,
) -> NixValue {
    checks
        .into_iter()
        .rev()
        .fold(value, |body, (condition, message)| {
            lib.throw_if_not(
                condition.into_expr::<bool>(),
                message.into_expr::<String>(),
                body,
            )
        })
}

#[test]
fn ordered_guards_stop_at_the_first_failure_without_forcing_later_values() {
    let lib = Nixpkgs::new().library();
    let session = NixSession::new().unwrap();
    let unused: NixValue = Expr::int(1).divide(Expr::int(0)).into();

    for rejected in 0..3 {
        let checks = std::array::from_fn(|index| {
            if index < rejected {
                (true.into(), unused.clone())
            } else if index == rejected {
                (false.into(), format!("rule {index} rejected").into())
            } else {
                (unused.clone(), unused.clone())
            }
        });
        let guarded = ordered_guards(&lib, checks, unused.clone());
        let diagnostic = session
            .evaluate_interop(
                &compile(Config::new().set_dynamic("result", guarded.clone())).unwrap(),
            )
            .unwrap_err();
        assert!(
            diagnostic
                .reason
                .contains(&format!("rule {rejected} rejected"))
        );
        assert!(!diagnostic.reason.contains("division by zero"));
        assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
        assert!(!diagnostic.raw_nix.is_empty());

        assert_eq!(
            evaluate(
                NixValue::record([("good", 42_i64.into()), ("unused", guarded)]).select("good")
            ),
            42,
        );
    }

    // Passing guards return the result, leaving unused messages and fields lazy.
    let result = NixValue::record([("good", 42_i64.into()), ("unused", unused.clone())]);
    let guarded = ordered_guards(
        &lib,
        std::array::from_fn(|_| (true.into(), unused.clone())),
        result,
    );
    assert_eq!(evaluate(guarded.select("good")), 42);
}

#[test]
fn ordered_guards_retain_failing_condition_and_message_origins() {
    let lib = Nixpkgs::new().library();
    let session = NixSession::new().unwrap();

    for failing_message in [false, true] {
        let line = line!() + 1;
        let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
        let rule = if failing_message {
            (false.into(), failure.clone())
        } else {
            (failure.clone(), "unused message".into())
        };
        let guarded = ordered_guards(
            &lib,
            [
                (true.into(), failure.clone()),
                rule,
                (false.into(), failure.clone()),
            ],
            failure,
        );
        let diagnostic = session
            .evaluate_interop(&compile(Config::new().set_dynamic("result", guarded)).unwrap())
            .unwrap_err();
        assert_eq!(diagnostic.reason, "division by zero");
        assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
        assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
        assert!(!diagnostic.raw_nix.is_empty());
    }
}

#[test]
fn ordered_guards_use_the_supplied_throw_function_at_every_level() {
    let lib = NixLibrary::from_expression(NixValue::record([(
        "throwIfNot",
        NixValue::function(|_| {
            NixValue::function(|message| {
                NixValue::function(move |value| NixValue::list([message, value]))
            })
        }),
    )]));
    let guarded = ordered_guards(
        &lib,
        ["first", "second", "third"].map(|message| (false.into(), message.into())),
        42_i64.into(),
    );

    // This supplied function deliberately ignores conditions instead of throwing.
    assert_eq!(
        evaluate(guarded),
        serde_json::json!(["first", ["second", ["third", 42]]]),
    );
}

#[test]
fn symbolic_negation_reports_its_caller_for_invalid_backend_boolean_types() {
    let line = line!() + 1;
    let value = !NixValue::from("not a boolean");
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", value)).unwrap())
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert!(diagnostic.reason.to_lowercase().contains("boolean"));
}

#[test]
fn boolean_conjunction_short_circuits_and_reports_operand_and_call_origins() {
    let failure_line = line!() + 1;
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let unused = Expr::boolean(false).and(failure.clone().into_expr());

    assert_eq!(evaluate(unused.into()), false);
    assert_eq!(evaluate(NixValue::from(false).and(failure.clone())), false);
    for left in [false, true] {
        for right in [false, true] {
            assert_eq!(
                evaluate(Expr::boolean(left).and(Expr::boolean(right)).into()),
                left && right,
            );
            assert_eq!(evaluate(NixValue::from(left).and(right)), left && right);
        }
    }

    let failure = Expr::boolean(true).and(failure.into_expr());
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", failure)).unwrap())
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, failure_line);
    assert_eq!(diagnostic.reason, "division by zero");

    // A declared bool expectation is not proof of the backend value's type.
    // Even the right operand is checked as a boolean, matching Nix &&.
    for left in [false, true] {
        let right = NixValue::from("not a boolean");
        let typed_line = line!() + 1;
        let typed = Expr::boolean(left).and(right.clone().into_expr()).into();
        let opaque_line = line!() + 1;
        let opaque = NixValue::from(left).and(right);
        let cases = [(typed, typed_line), (opaque, opaque_line)];
        for (invalid, call_line) in cases {
            let artifact = compile(Config::new().set_dynamic("result", invalid)).unwrap();
            let result = NixSession::new().unwrap().evaluate_interop(&artifact);

            if left {
                let diagnostic = result.unwrap_err();
                assert_eq!(diagnostic.primary.as_ref().unwrap().line, call_line);
                assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
                assert!(diagnostic.reason.to_lowercase().contains("boolean"));
                assert!(!diagnostic.raw_nix.is_empty());
            } else {
                assert_eq!(result.unwrap().value["result"], false);
            }
        }
    }
}

#[test]
fn output_helpers_keep_fallback_and_explicit_output_semantics_lazy() {
    let lib = Nixpkgs::new().library();
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let split = NixValue::record([
        ("dev", "dev-output".into()),
        ("lib", "lib-output".into()),
        ("out", failure.clone()),
    ]);
    let fallback = NixValue::record([
        ("out", "default-output".into()),
        ("unused", failure.clone()),
    ]);
    let unsplit = NixValue::record([("tag", "whole-package".into()), ("unused", failure.clone())]);
    let specified = NixValue::record([
        ("outputSpecified", true.into()),
        ("tag", "explicit-output".into()),
        ("dev", failure.clone()),
        ("lib", failure),
    ]);

    assert_eq!(
        evaluate(NixValue::record([
            (
                "dev",
                lib.get_dev(Package::from_expression(split.clone())).into()
            ),
            ("lib", lib.get_lib(Package::from_expression(split)).into()),
            (
                "fallbackDev",
                lib.get_dev(Package::from_expression(fallback.clone()))
                    .into()
            ),
            (
                "fallbackLib",
                lib.get_lib(Package::from_expression(fallback)).into()
            ),
            (
                "wholeDev",
                lib.get_dev(Package::from_expression(unsplit.clone()))
                    .field::<NixValue>("tag")
            ),
            (
                "wholeLib",
                lib.get_lib(Package::from_expression(unsplit))
                    .field::<NixValue>("tag")
            ),
            (
                "explicitDev",
                lib.get_dev(Package::from_expression(specified.clone()))
                    .field::<NixValue>("tag")
            ),
            (
                "explicitLib",
                lib.get_lib(Package::from_expression(specified))
                    .field::<NixValue>("tag")
            ),
        ])),
        serde_json::json!({
            "dev":"dev-output", "lib":"lib-output",
            "fallbackDev":"default-output", "fallbackLib":"default-output",
            "wholeDev":"whole-package", "wholeLib":"whole-package",
            "explicitDev":"explicit-output", "explicitLib":"explicit-output",
        })
    );
}

#[test]
fn boolean_or_and_implication_check_demanded_operands_and_short_circuit() {
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    assert_eq!(
        evaluate(NixValue::record([
            (
                "typedOr",
                Expr::boolean(true).or(failure.clone().into_expr()).into()
            ),
            ("or", NixValue::from(true).or(failure.clone())),
            (
                "typedImplies",
                Expr::boolean(false)
                    .implies(failure.clone().into_expr())
                    .into()
            ),
            ("implies", NixValue::from(false).implies(failure)),
        ])),
        serde_json::json!({"typedOr":true, "or":true, "typedImplies":true, "implies":true})
    );

    for left in [false, true] {
        for right in [false, true] {
            assert_eq!(
                evaluate(NixValue::record([
                    (
                        "typedOr",
                        Expr::boolean(left).or(Expr::boolean(right)).into()
                    ),
                    ("or", NixValue::from(left).or(right)),
                    (
                        "typedImplies",
                        Expr::boolean(left).implies(Expr::boolean(right)).into()
                    ),
                    ("implies", NixValue::from(left).implies(right)),
                ])),
                serde_json::json!({
                    "typedOr":left || right, "or":left || right,
                    "typedImplies":!left || right, "implies":!left || right,
                })
            );
        }

        for implication in [false, true] {
            let right = NixValue::from("not a boolean");
            let typed_right = right.clone().into_expr::<bool>();
            let (typed, typed_line): (NixValue, _) = if implication {
                let call_line = line!() + 1;
                let value = Expr::boolean(left).implies(typed_right);
                (value.into(), call_line)
            } else {
                let call_line = line!() + 1;
                let value = Expr::boolean(left).or(typed_right);
                (value.into(), call_line)
            };
            let (opaque, opaque_line) = if implication {
                let call_line = line!() + 1;
                let value = NixValue::from(left).implies(right);
                (value, call_line)
            } else {
                let call_line = line!() + 1;
                let value = NixValue::from(left).or(right);
                (value, call_line)
            };
            for (value, line) in [(typed, typed_line), (opaque, opaque_line)] {
                let result = NixSession::new().unwrap().evaluate_interop(
                    &compile(Config::new().set_dynamic("result", value)).unwrap(),
                );
                if left == implication {
                    let error = result.unwrap_err();
                    assert!(error.reason.to_lowercase().contains("boolean"));
                    assert_eq!(error.primary.as_ref().unwrap().file, file!());
                    assert_eq!(error.primary.as_ref().unwrap().line, line);
                    assert!(!error.raw_nix.is_empty());
                } else {
                    assert_eq!(result.unwrap().value["result"], true);
                }
            }
        }
    }
}

#[test]
fn native_helpers_report_type_errors_at_the_public_call_site() {
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let boolean_line = line!() + 1;
    let invalid_boolean = NixValue::from("not a boolean").and(failure.clone());
    let or_line = line!() + 1;
    let invalid_or = NixValue::from("not a boolean").or(failure.clone());
    let implies_line = line!() + 1;
    let invalid_implies = NixValue::from("not a boolean").implies(failure);
    let cases = [
        (invalid_boolean, "boolean", boolean_line),
        (invalid_or, "boolean", or_line),
        (invalid_implies, "boolean", implies_line),
        (NixValue::concat_lists([42_i64.into()]), "list", line!()),
    ];

    for (value, expected_type, line) in cases {
        let diagnostic = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(Config::new().set_dynamic("result", value)).unwrap())
            .unwrap_err();
        assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
        assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
        assert!(diagnostic.reason.to_lowercase().contains(expected_type));
        assert!(!diagnostic.reason.contains("division by zero"));
        assert!(!diagnostic.raw_nix.is_empty());
    }
}

#[test]
fn optional_text_keeps_exact_bytes_and_store_dependency_context() {
    let lib = Nixpkgs::new().library();
    let dependency = Nixpkgs::new().get("hello");
    let text = nix_text!("prefix {dependency}\n suffix", dependency = dependency);
    let context = InputRef::local(
        "context",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/structured-interop.nix"),
    )
    .function("getContext");
    let selected = lib.optional_text(Expr::boolean(true), text.clone());
    let excluded = lib.optional_text(Expr::boolean(false), text.clone());

    assert_eq!(
        evaluate(NixValue::record([
            (
                "bytesEqual",
                NixValue::from(selected.clone()).equals(text.clone())
            ),
            (
                "nativeReplacementContextEqual",
                context
                    .call(text.clone().replace_text([("prefix", "changed")]))
                    .equals(context.call(text.clone())),
            ),
            (
                "libraryReplacementContextEqual",
                context
                    .call(lib.replace_text(text.clone(), [("prefix", "changed")]))
                    .equals(context.call(text.clone())),
            ),
            (
                "validatedContextEqual",
                context
                    .call(lib.throw_if_not(true, "unused", selected.clone()))
                    .equals(context.call(selected.clone()))
            ),
            (
                "contextsEqual",
                context.call(selected.clone()).equals(context.call(text))
            ),
            (
                "contextNotEmpty",
                !context
                    .call(selected)
                    .equals(NixValue::record([] as [(&str, NixValue); 0]))
            ),
            ("excluded", excluded.clone().into()),
            ("excludedContext", context.call(excluded)),
        ])),
        serde_json::json!({
            "bytesEqual":true,"validatedContextEqual":true,"contextsEqual":true,"contextNotEmpty":true,
            "nativeReplacementContextEqual":true,"libraryReplacementContextEqual":true,
            "excluded":"","excludedContext":{},
        }),
    );
}
