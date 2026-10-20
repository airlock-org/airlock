#![cfg(feature = "evaluation")]

//! Native Nix operations remain lazy and preserve source locations without library substitution.
use rusix::{Config, Expr, interop::raw::NixValue};
use rusix::{NixSession, compile};

fn evaluate(value: NixValue) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", value)).unwrap())
        .unwrap()
        .value["result"]
        .clone()
}

#[test]
fn builtin_functions_accept_concrete_and_scoped_symbolic_values() {
    let replace = NixValue::builtin("replaceStrings");
    let factory = NixValue::function_attrs(["text"], |args| {
        (
            vec![("text", "8.11.0".into())],
            replace.apply([
                NixValue::list([".".into()]),
                NixValue::list(["_".into()]),
                args.select("text"),
            ]),
        )
    });

    assert_eq!(
        evaluate(
            factory
                .clone()
                .call(NixValue::record([] as [(&str, NixValue); 0]))
        ),
        "8_11_0"
    );
    assert_eq!(
        evaluate(factory.call(NixValue::record([("text", "overridden.release".into())]))),
        "overridden_release",
    );
    assert_eq!(
        evaluate(NixValue::builtin("lessThan").apply([1_i64.into(), 2_i64.into()])),
        true,
    );
}

#[test]
fn replacement_pairs_preserve_order_and_accept_deferred_text_and_patterns() {
    let factory = NixValue::function_attrs(["text", "from", "to"], |args| {
        (
            Vec::<(&str, NixValue)>::new(),
            args.clone()
                .select("text")
                .replace_text([(args.clone().select("from"), args.select("to"))]),
        )
    });
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    assert_eq!(
        evaluate(NixValue::record([
            (
                "symbolic",
                factory.call(NixValue::record([
                    ("text", "8.11.0".into()),
                    ("from", ".".into()),
                    ("to", "_".into()),
                ]))
            ),
            (
                "nonRecursive",
                NixValue::from("ab a").replace_text([("ab", "a"), ("a", "b")])
            ),
            (
                "firstMatch",
                NixValue::from("ab").replace_text([("a", "_"), ("ab", "long")])
            ),
            (
                "empty",
                NixValue::from("unchanged").replace_text([] as [(&str, &str); 0])
            ),
            (
                "unusedReplacement",
                NixValue::from("a")
                    .replace_text([("a", NixValue::from("matched")), ("b", failure)])
            ),
        ])),
        serde_json::json!({
            "symbolic":"8_11_0", "nonRecursive":"a b", "firstMatch":"_b",
            "empty":"unchanged", "unusedReplacement":"matched",
        })
    );
}

#[test]
fn attribute_helpers_keep_names_literal_and_unselected_values_lazy() {
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let unusual = "a.\"${builtins.abort \"not code\"}\n";
    let attrs = NixValue::record([
        ("a.b", 42_i64.into()),
        (unusual, "literal data".into()),
        ("null", NixValue::null()),
        ("bad", failure.clone()),
        ("", "empty name".into()),
    ]);
    let factory = NixValue::function(|name| attrs.clone().attr_or(name, failure.clone()));

    assert_eq!(
        evaluate(NixValue::record([
            ("hasBad", attrs.clone().has_attr("bad")),
            ("hasDotted", attrs.clone().has_attr("a.b")),
            ("hasPrefix", attrs.clone().has_attr("a")),
            ("dotted", attrs.clone().attr_or("a.b", failure.clone())),
            ("dynamic", factory.call(unusual)),
            ("absent", attrs.clone().attr_or("absent", 7_i64)),
            ("null", attrs.clone().attr_or("null", failure.clone())),
            ("emptyName", attrs.clone().attr_or("", failure.clone())),
            (
                "selectedSubtree",
                attrs
                    .attr_or(
                        "missing",
                        NixValue::record([("good", 7_i64.into()), ("bad", failure),])
                    )
                    .select("good")
            ),
        ])),
        serde_json::json!({
            "hasBad":true,"hasDotted":true,"hasPrefix":false,"dotted":42,
            "dynamic":"literal data","absent":7,"null":null,"emptyName":"empty name",
            "selectedSubtree":7,
        })
    );
}

#[test]
fn native_assertion_leaves_its_value_lazy_and_retains_its_call_site() {
    let unused: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let line = line!() + 1;
    let rejected = NixValue::assert(false, unused.clone());
    let generated = compile(Config::new().set_dynamic("result", rejected.clone())).unwrap();
    assert!(generated.source.contains("assert false;"));
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(error.reason.contains("assertion"), "{}", error.reason);
    assert_eq!(error.primary.as_ref().unwrap().file, file!());
    assert_eq!(error.primary.as_ref().unwrap().line, line);
    assert!(!error.raw_nix.is_empty());
    assert!(!error.reason.contains("division by zero"));

    assert_eq!(evaluate(NixValue::assert(true, 42_i64)), 42);
    assert_eq!(
        evaluate(NixValue::record([("good", 42_i64.into()), ("unused", rejected),]).select("good")),
        42
    );
    assert_eq!(
        evaluate(
            NixValue::assert(
                true,
                NixValue::record([("good", 42_i64.into()), ("unused", unused),])
            )
            .select("good")
        ),
        42
    );
}

#[test]
fn native_union_is_shallow_right_biased_and_keeps_displaced_values_lazy() {
    let bad: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let left = NixValue::record([
        ("unchanged", 7_i64.into()),
        ("replaced", bad.clone()),
        ("nested", NixValue::record([("left", true.into())])),
    ]);
    let right = NixValue::record([
        ("replaced", 42_i64.into()),
        ("nested", NixValue::record([("right", true.into())])),
        ("unused", bad),
    ]);
    let merged = left.merge_attrs(right);
    assert_eq!(evaluate(merged.clone().select("replaced")), 42);
    assert_eq!(evaluate(merged.clone().select("unchanged")), 7);
    assert_eq!(
        evaluate(merged.select("nested")),
        serde_json::json!({"right":true})
    );

    let factory =
        NixValue::function(|args| args.merge_attrs(NixValue::record([("version", 1_i64.into())])));
    assert_eq!(
        evaluate(factory.call(NixValue::record([("version", 2_i64.into())]))),
        serde_json::json!({"version":1})
    );
}

#[test]
fn native_operation_failures_preserve_child_origins_and_operation_boundaries() {
    let child_line = line!() + 1;
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    for value in [
        NixValue::assert(failure.clone(), 42_i64),
        NixValue::assert(true, failure.clone()),
        failure
            .clone()
            .merge_attrs(NixValue::record([] as [(&str, NixValue); 0])),
        NixValue::builtin("lessThan").apply([failure.clone(), 2_i64.into()]),
        failure.clone().replace_text([("a", "b")]),
        NixValue::from("a").replace_text([(failure.clone(), "b")]),
        NixValue::from("a").replace_text([("a", failure.clone())]),
        failure.clone().has_attr("a"),
        NixValue::record([] as [(&str, NixValue); 0]).has_attr(failure.clone()),
        failure.clone().attr_or("a", 42_i64),
        NixValue::record([] as [(&str, NixValue); 0]).attr_or("a", failure.clone()),
        NixValue::record([("a", failure.clone())]).attr_or("a", 42_i64),
    ] {
        let error = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(Config::new().set_dynamic("result", value)).unwrap())
            .unwrap_err();
        assert_eq!(error.reason, "division by zero");
        assert_eq!(error.primary.as_ref().unwrap().line, child_line);
        assert!(!error.raw_nix.is_empty());
    }

    for wrong_left in [false, true] {
        let record = NixValue::record([] as [(&str, NixValue); 0]);
        let (left, right) = if wrong_left {
            (NixValue::from(1_i64), record)
        } else {
            (record, NixValue::from(1_i64))
        };
        let call_line = line!() + 1;
        let invalid = left.merge_attrs(right);
        let error = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(Config::new().set_dynamic("result", invalid)).unwrap())
            .unwrap_err();
        assert_eq!(error.primary.as_ref().unwrap().line, call_line);
        assert!(!error.raw_nix.is_empty());
    }

    let replace = NixValue::builtin("replaceStrings");
    let call_line = line!() + 1;
    let invalid = replace.apply([true.into(), false.into(), 1_i64.into()]);
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", invalid)).unwrap())
        .unwrap_err();
    assert_eq!(error.primary.as_ref().unwrap().line, call_line);
}

#[test]
fn named_operation_type_failures_report_the_public_call_site() {
    let text_line = line!() + 1;
    let invalid_text = NixValue::from(true).replace_text([("a", "b")]);
    let pattern_line = line!() + 1;
    let invalid_pattern = NixValue::from("a").replace_text([(true, "b")]);
    let name_line = line!() + 1;
    let invalid_name = NixValue::record([] as [(&str, NixValue); 0]).has_attr(42_i64);
    let cases = [
        (invalid_text, text_line),
        (invalid_pattern, pattern_line),
        (invalid_name, name_line),
        (NixValue::from(true).has_attr("a"), line!()),
        (NixValue::from(true).attr_or("a", 42_i64), line!()),
    ];
    for (value, line) in cases {
        let error = NixSession::new()
            .unwrap()
            .evaluate_interop(&compile(Config::new().set_dynamic("result", value)).unwrap())
            .unwrap_err();
        assert_eq!(error.primary.as_ref().unwrap().file, file!());
        assert_eq!(error.primary.as_ref().unwrap().line, line);
        assert!(!error.raw_nix.is_empty());
    }
}

#[test]
fn builtin_names_are_literal_path_segments_and_invalid_paths_are_rejected() {
    let line = line!() + 1;
    let missing = NixValue::builtin("not.a.builtin");
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", missing)).unwrap())
        .unwrap_err();
    assert!(error.reason.contains("not.a.builtin"));
    assert_eq!(error.primary.as_ref().unwrap().line, line);
    assert!(!error.raw_nix.is_empty());

    assert!(compile(Config::new().set_dynamic("invalid", NixValue::builtin("bad\0name"))).is_err());
}
