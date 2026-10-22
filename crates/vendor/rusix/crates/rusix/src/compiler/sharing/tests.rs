//! Exercise memoization, lexical scope and diagnostic preservation with the real evaluator.

use super::*;
use crate::{
    Config, NixSession,
    compiler::{
        context_audit::equivalent_diagnostics, lower, render::render as render_without_sharing,
    },
    interop::{NixExpression, raw::NixValue},
};

/// Compile the optimized tree while retaining the raw renderer as an evaluator oracle.
fn render(ast: &NixExpr) -> crate::Generated {
    render_without_sharing(&optimize(ast.clone()))
}

/// Construct an intentionally nontrivial traced value whose clones should be evaluated once.
fn traced(value: NixValue) -> NixValue {
    NixValue::builtin("trace")
        .call("rusix-sharing-evaluated")
        .call(
            NixValue::builtin("concatStringsSep")
                .call("")
                .call(NixValue::list([
                    "one".into(),
                    "two".into(),
                    "three".into(),
                    "four".into(),
                    value,
                ])),
        )
}

/// Sharing is observable through trace count, rather than generated formatting or byte count alone.
#[test]
fn repeated_computations_are_memoized() {
    let value = traced("five".into());
    let config = Config::new().set_dynamic("values", NixValue::list([value.clone(), value]));
    let ast = lower(&config);
    let original = render_without_sharing(&ast);
    let optimized = render(&ast);
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&original).unwrap();
    let after = session.evaluate(&optimized).unwrap();

    assert_eq!(before.value, after.value);
    assert_eq!(before.raw_nix.matches("rusix-sharing-evaluated").count(), 2);
    assert_eq!(after.raw_nix.matches("rusix-sharing-evaluated").count(), 1);
}

/// One function's invariant work is shared across different calls without changing argument values.
#[test]
fn function_invariants_are_memoized_across_calls() {
    let constant = traced("payload".into());
    let function = NixValue::function(|argument| {
        NixValue::record([("argument", argument), ("constant", constant)])
    });
    let value = function
        .bind(|function| NixValue::list([function.clone().call(1_i64), function.call(2_i64)]));
    let ast = lower(&Config::new().set_dynamic("values", value));
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value, after.value);
    assert_eq!(after.value["values"][0]["argument"], 1);
    assert_eq!(after.value["values"][1]["argument"], 2);
    assert_eq!(before.raw_nix.matches("rusix-sharing-evaluated").count(), 2);
    assert_eq!(after.raw_nix.matches("rusix-sharing-evaluated").count(), 1);
}

/// Sibling closures can share the same captured computation despite having separate arguments.
#[test]
fn sibling_functions_share_captured_computations() {
    let values = [1_i64, 2_i64].map(|argument| {
        let expensive = traced("payload".into());
        NixValue::function(|input| NixValue::list([input, expensive])).call(argument)
    });
    let ast = lower(&Config::new().set_dynamic("values", NixValue::list(values)));
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value, after.value);
    assert_eq!(after.value["values"][0][0], 1);
    assert_eq!(after.value["values"][1][0], 2);
    assert_eq!(before.raw_nix.matches("rusix-sharing-evaluated").count(), 2);
    assert_eq!(after.raw_nix.matches("rusix-sharing-evaluated").count(), 1);
}

/// Existing recursive lets may share outer captures without moving work outside its declaration.
#[test]
fn sibling_lets_share_work_inside_the_required_outer_binding() {
    let expensive = lower(&Config::new().set_dynamic("value", traced("payload".into())));
    let siblings = [1_i64, 2_i64].map(|value| {
        NixExpr::plain(NixKind::Let(
            "inner".into(),
            Box::new(NixExpr::plain(NixKind::Int(value))),
            Box::new(NixExpr::plain(NixKind::List(vec![
                NixExpr::plain(NixKind::Variable("inner".into())),
                arithmetic("outer"),
                expensive.clone(),
            ]))),
        ))
    });
    let ast = NixExpr::plain(NixKind::Let(
        "outer".into(),
        Box::new(NixExpr::plain(NixKind::Int(10))),
        Box::new(NixExpr::plain(NixKind::List(siblings.into()))),
    ));
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value, after.value);
    assert_eq!(after.value[0][1], 18);
    assert_eq!(after.value[1][1], 18);
    assert_eq!(before.raw_nix.matches("rusix-sharing-evaluated").count(), 2);
    assert_eq!(after.raw_nix.matches("rusix-sharing-evaluated").count(), 1);
}

/// Literal arguments expose equivalent work in separate calls without forcing unused arguments.
#[test]
fn known_lambda_arguments_expose_shared_work_and_remain_lazy() {
    let values = [0_i64, 1_i64].map(|tag| {
        NixValue::function(|input| NixValue::list([tag.into(), traced(input)])).call("payload")
    });
    let unused = NixValue::function(|_| NixValue::from(42_i64))
        .call(NixValue::builtin("throw").call("unused lambda argument"));
    let ast = lower(
        &Config::new()
            .set_dynamic("values", NixValue::list(values))
            .set_dynamic("unused", unused),
    );
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value, after.value);
    assert_eq!(after.value["unused"], 42);
    assert_eq!(before.raw_nix.matches("rusix-sharing-evaluated").count(), 2);
    assert_eq!(after.raw_nix.matches("rusix-sharing-evaluated").count(), 1);
}

/// Moving a shared failure above sibling functions must preserve selective evaluation and blame.
#[test]
fn failures_shared_across_scopes_stay_lazy_and_keep_diagnostics() {
    let calls = [1_i64, 2_i64].map(|argument| {
        let bad = traced(NixValue::builtin("throw").call("cross-scope failure"));
        NixValue::function(|input| NixValue::record([("argument", input), ("bad", bad)]))
            .call(argument)
    });
    let good = NixValue::list(calls.iter().map(|call| call.clone().select("argument")));
    let ast = lower(
        &Config::new()
            .set_dynamic("good", good)
            .set_dynamic("bad", NixValue::list(calls)),
    );
    let session = NixSession::new().unwrap();
    let original = render_without_sharing(&ast);
    let optimized = render(&ast);

    assert_eq!(
        session
            .evaluate_attribute(&optimized, "good")
            .unwrap()
            .value,
        serde_json::json!([1, 2]),
    );
    let before = session.evaluate_attribute(&original, "bad").unwrap_err();
    let after = session.evaluate_attribute(&optimized, "bad").unwrap_err();
    equivalent_diagnostics(&before, &after);
}

/// Some attribute consumers inspect a function before forcing thunks, as Nix's flake loader does.
#[test]
fn exported_functions_keep_their_direct_function_shape() {
    let outputs = NixValue::function_attrs(["self"], |_| {
        (Vec::<(&str, NixValue)>::new(), traced("payload".into()))
    });
    let ast = lower(&Config::new().set_dynamic("outputs", outputs));
    let optimized = optimize(ast);
    let NixKind::AttrSet(fields) = optimized.kind else {
        panic!("root attribute set changed shape")
    };
    let NixKind::Group(value) = &fields[0].1.kind else {
        panic!("assignment attribution lost")
    };
    assert!(matches!(value.kind, NixKind::ArgumentFunction(..)));
}

/// Captured outer arguments remain different for each factory call while inner calls share invariants.
#[test]
fn lifted_inner_work_retains_outer_captures() {
    let factory = NixValue::function(|outer| NixValue::function(|_| traced(outer)));
    let value = factory.bind(|factory| {
        NixValue::list([
            factory
                .clone()
                .call("one")
                .bind(|inner| NixValue::list([inner.clone().call(0_i64), inner.call(1_i64)])),
            factory
                .call("two")
                .bind(|inner| NixValue::list([inner.clone().call(0_i64), inner.call(1_i64)])),
        ])
    });
    let ast = lower(&Config::new().set_dynamic("values", value));
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value, after.value);
    assert_ne!(after.value["values"][0], after.value["values"][1]);
    assert_eq!(before.raw_nix.matches("rusix-sharing-evaluated").count(), 4);
    assert_eq!(after.raw_nix.matches("rusix-sharing-evaluated").count(), 2);
}

/// Moving a potentially failing value outside a function must not force it or lose its source path.
#[test]
fn lifted_failures_stay_lazy_and_keep_diagnostics() {
    let bad = traced(NixValue::builtin("throw").call("invariant failure"));
    let function =
        NixValue::function(|argument| NixValue::record([("argument", argument), ("bad", bad)]));
    let value = function.call(42_i64);
    let ast = lower(
        &Config::new()
            .set_dynamic("good", value.clone().select("argument"))
            .set_dynamic("bad", value.select("bad")),
    );
    let session = NixSession::new().unwrap();
    let original = render_without_sharing(&ast);
    let optimized = render(&ast);
    assert_eq!(
        session
            .evaluate_attribute(&optimized, "good")
            .unwrap()
            .value,
        42
    );
    let before = session.evaluate_attribute(&original, "bad").unwrap_err();
    let after = session.evaluate_attribute(&optimized, "bad").unwrap_err();
    equivalent_diagnostics(&before, &after);
}

/// A shared failing definition must remain lazy and preserve the demanded operation's blame.
#[test]
fn unused_failures_stay_lazy_and_demanded_failures_keep_diagnostics() {
    let bad = traced(NixValue::builtin("throw").call("deliberate sharing failure"));
    let config = Config::new()
        .set_dynamic("good", 42_i64)
        .set_dynamic("bad", NixValue::list([bad.clone(), bad.clone()]))
        .set_dynamic("alsoBad", bad);
    let ast = lower(&config);
    let original = render_without_sharing(&ast);
    let optimized = render(&ast);
    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_attribute(&optimized, "good")
            .unwrap()
            .value,
        42
    );

    for field in ["bad", "alsoBad"] {
        let before = session.evaluate_attribute(&original, field).unwrap_err();
        let after = session.evaluate_attribute(&optimized, field).unwrap_err();
        equivalent_diagnostics(&before, &after);
    }
}

/// Make a sufficiently large arithmetic tree around a lexical reference.
fn arithmetic(name: &str) -> NixExpr {
    (0..8).fold(
        NixExpr::plain(NixKind::Variable(name.into())),
        |value, _| {
            NixExpr::plain(NixKind::Binary(
                BinaryOp::Add,
                Box::new(value),
                Box::new(NixExpr::plain(NixKind::Int(1))),
            ))
        },
    )
}

/// Existing generated-looking names and nested shadowed parameters must remain correctly bound.
#[test]
fn sharing_resolves_shadowed_parameters_without_capture() {
    let repeated = arithmetic("__rusix_shared_0");
    let inner = NixExpr::plain(NixKind::Apply(
        Box::new(NixExpr::plain(NixKind::Lambda(
            "__rusix_shared_0".into(),
            Box::new(NixExpr::plain(NixKind::List(vec![
                repeated.clone(),
                repeated.clone(),
            ]))),
        ))),
        Box::new(NixExpr::plain(NixKind::Int(100))),
    ));
    let outer = NixExpr::plain(NixKind::Apply(
        Box::new(NixExpr::plain(NixKind::Lambda(
            "__rusix_shared_0".into(),
            Box::new(NixExpr::plain(NixKind::List(vec![
                repeated.clone(),
                inner,
                repeated,
            ]))),
        ))),
        Box::new(NixExpr::plain(NixKind::Int(10))),
    ));
    let session = NixSession::new().unwrap();
    let original = session.evaluate(&render_without_sharing(&outer)).unwrap();
    let optimized = session.evaluate(&render(&outer)).unwrap();

    assert_eq!(original.value, serde_json::json!([18, [108, 108], 18]));
    assert_eq!(optimized.value, original.value);
}

/// A recursive initializer must not inherit a same-named outer constant during propagation.
#[test]
fn recursive_let_initializers_keep_their_own_binding() {
    let recursive = NixExpr::plain(NixKind::Let(
        "value".into(),
        Box::new(arithmetic("value")),
        Box::new(NixExpr::plain(NixKind::Variable("value".into()))),
    ));
    let ast = NixExpr::plain(NixKind::Let(
        "value".into(),
        Box::new(NixExpr::plain(NixKind::Int(100))),
        Box::new(recursive),
    ));
    let session = NixSession::new().unwrap();
    for generated in [render_without_sharing(&ast), render(&ast)] {
        let error = session.evaluate(&generated).unwrap_err();
        assert!(error.reason.contains("infinite recursion"));
    }
}

/// Native operations and diagnostic wrappers resolve the same shadowable namespace as Nix source.
#[test]
fn implicit_builtins_remain_lexical_dependencies() {
    /// Make a native operation substantial enough to qualify for sharing and lifting.
    fn payload() -> NixExpr {
        (0..8).fold(
            NixExpr::plain(NixKind::Call(
                Builtin::Div,
                vec![
                    NixExpr::plain(NixKind::Int(8)),
                    NixExpr::plain(NixKind::Int(2)),
                ],
            )),
            |value, _| {
                NixExpr::plain(NixKind::Binary(
                    BinaryOp::Add,
                    Box::new(value),
                    Box::new(NixExpr::plain(NixKind::Int(1))),
                ))
            },
        )
    }

    /// Override two ordinary Nix namespace members with a value observable by evaluation.
    fn namespace() -> NixExpr {
        let function = NixExpr::plain(NixKind::Lambda(
            "left".into(),
            Box::new(NixExpr::plain(NixKind::Lambda(
                "right".into(),
                Box::new(NixExpr::plain(NixKind::Int(77))),
            ))),
        ));
        NixExpr::plain(NixKind::AttrSet(vec![
            (vec!["div".into()], function.clone()),
            (vec!["addErrorContext".into()], function),
        ]))
    }

    let local = NixExpr::plain(NixKind::Let(
        "builtins".into(),
        Box::new(namespace()),
        Box::new(payload()),
    ));
    let named = NixExpr::plain(NixKind::Apply(
        Box::new(NixExpr::plain(NixKind::Function(
            vec!["builtins".into()],
            Box::new(payload()),
        ))),
        Box::new(NixExpr::plain(NixKind::AttrSet(vec![(
            vec!["builtins".into()],
            namespace(),
        )]))),
    ));
    let positional = NixExpr::plain(NixKind::Apply(
        Box::new(NixExpr::plain(NixKind::Lambda(
            "builtins".into(),
            Box::new(payload()),
        ))),
        Box::new(namespace()),
    ));
    let contextual = NixExpr::plain(NixKind::Let(
        "builtins".into(),
        Box::new(namespace()),
        Box::new(NixExpr::plain(NixKind::Let(
            "value".into(),
            Box::new(NixExpr::contextual(
                NixKind::Int(1),
                Origin::new("namespace.rs", 1, 1, "contextual value"),
            )),
            Box::new(NixExpr::plain(NixKind::Variable("value".into()))),
        ))),
    ));
    let ast = NixExpr::plain(NixKind::List(vec![
        payload(),
        local,
        named,
        positional,
        contextual,
    ]));
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value, serde_json::json!([12, 85, 85, 85, 77]));
    assert_eq!(after.value, before.value);
}

/// Parser-introduced calls and scalar prelude tokens must keep their original lexical lookup.
#[test]
fn implicit_scalar_and_comparison_names_remain_lexical() {
    /// Build a two-argument replacement for one of Nix's parser helpers.
    fn returning(value: NixExpr) -> NixExpr {
        NixExpr::plain(NixKind::Lambda(
            "left".into(),
            Box::new(NixExpr::plain(NixKind::Lambda(
                "right".into(),
                Box::new(value),
            ))),
        ))
    }

    /// Introduce a native recursive declaration around the supplied expression.
    fn local(name: &str, value: NixExpr, body: NixExpr) -> NixExpr {
        NixExpr::plain(NixKind::Let(name.into(), Box::new(value), Box::new(body)))
    }

    let scalars = NixExpr::plain(NixKind::List(
        (0..3)
            .flat_map(|_| {
                [
                    NixExpr::plain(NixKind::Bool(true)),
                    NixExpr::plain(NixKind::Bool(false)),
                    NixExpr::plain(NixKind::Null),
                ]
            })
            .collect(),
    ));
    let negatives = NixExpr::plain(NixKind::List(vec![
        NixExpr::plain(NixKind::Int(-5)),
        NixExpr::plain(NixKind::Float(-0.5)),
        NixExpr::plain(NixKind::Int(i64::MIN)),
        NixExpr::plain(NixKind::Int(1)),
        NixExpr::plain(NixKind::Int(2)),
        NixExpr::plain(NixKind::Int(3)),
        NixExpr::plain(NixKind::Int(4)),
    ]));
    let comparison = NixExpr::plain(NixKind::Binary(
        BinaryOp::GreaterEqual,
        Box::new(NixExpr::plain(NixKind::Int(4))),
        Box::new(NixExpr::plain(NixKind::Int(2))),
    ));
    let comparison_call = NixExpr::plain(NixKind::Apply(
        Box::new(NixExpr::plain(NixKind::Lambda(
            "__lessThan".into(),
            Box::new(NixExpr::plain(NixKind::List(vec![comparison; 8]))),
        ))),
        Box::new(returning(NixExpr::plain(NixKind::Bool(true)))),
    ));
    let cached = local(
        "cached",
        NixExpr::plain(NixKind::Bool(true)),
        local(
            "true",
            NixExpr::plain(NixKind::Int(77)),
            NixExpr::plain(NixKind::Variable("cached".into())),
        ),
    );
    let ast = NixExpr::plain(NixKind::List(vec![
        scalars.clone(),
        local("true", NixExpr::plain(NixKind::Int(77)), scalars.clone()),
        local("false", NixExpr::plain(NixKind::Int(88)), scalars.clone()),
        local("null", NixExpr::plain(NixKind::Int(99)), scalars),
        negatives.clone(),
        local(
            "__sub",
            returning(NixExpr::plain(NixKind::Int(77))),
            negatives,
        ),
        comparison_call,
        cached,
    ]));
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value[1][0], 77);
    assert_eq!(before.value[2][1], 88);
    assert_eq!(before.value[3][2], 99);
    assert_eq!(before.value[5][0], 77);
    assert_eq!(before.value[6], serde_json::json!(vec![false; 8]));
    assert_eq!(before.value[7], true);
    assert_eq!(after.value, before.value);
}

/// Observing generated source positions makes unrelated syntax movement observable too.
#[test]
fn source_position_observation_preserves_line_and_column() {
    let repeated = arithmetic("value");
    let ast = NixExpr::plain(NixKind::Let(
        "value".into(),
        Box::new(NixExpr::plain(NixKind::Int(10))),
        Box::new(NixExpr::plain(NixKind::List(vec![
            NixExpr::plain(NixKind::List(vec![repeated.clone(), repeated])),
            NixExpr::plain(NixKind::Variable("__curPos".into())),
        ]))),
    ));
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value[0], after.value[0]);
    for coordinate in ["line", "column"] {
        assert_eq!(before.value[1][coordinate], after.value[1][coordinate]);
    }
}

/// An unavailable common placement must not discard sharing inside each valid child region.
#[test]
fn blocked_cross_scope_sharing_preserves_local_memoization() {
    /// Build a substantial traced computation depending on the surrounding `base` declaration.
    fn payload() -> NixExpr {
        let builtin = |name: &str| {
            NixExpr::plain(NixKind::Select(
                Box::new(NixExpr::plain(NixKind::Variable("builtins".into()))),
                vec![name.into()],
            ))
        };
        let value = NixExpr::plain(NixKind::Apply(
            Box::new(NixExpr::plain(NixKind::Apply(
                Box::new(builtin("concatStringsSep")),
                Box::new(NixExpr::plain(NixKind::String(String::new()))),
            ))),
            Box::new(NixExpr::plain(NixKind::List(vec![
                NixExpr::plain(NixKind::String("one".into())),
                NixExpr::plain(NixKind::String("two".into())),
                NixExpr::plain(NixKind::String("three".into())),
                NixExpr::plain(NixKind::String("four".into())),
                NixExpr::plain(NixKind::Variable("base".into())),
            ]))),
        ));
        NixExpr::plain(NixKind::Apply(
            Box::new(NixExpr::plain(NixKind::Apply(
                Box::new(builtin("trace")),
                Box::new(NixExpr::plain(NixKind::String(
                    "rusix-sharing-evaluated".into(),
                ))),
            ))),
            Box::new(value),
        ))
    }

    let body = NixExpr::plain(NixKind::List(vec![payload(), payload()]));
    let defaults = NixExpr::plain(NixKind::Apply(
        Box::new(NixExpr::plain(NixKind::ArgumentFunction(
            vec![
                (
                    "base".into(),
                    Some(NixExpr::plain(NixKind::String("payload".into()))),
                ),
                ("unused".into(), Some(payload())),
            ],
            Box::new(body.clone()),
        ))),
        Box::new(NixExpr::plain(NixKind::AttrSet(vec![]))),
    ));
    let recursive = NixExpr::plain(NixKind::Let(
        "base".into(),
        Box::new(NixExpr::plain(NixKind::If(
            Box::new(NixExpr::plain(NixKind::Bool(false))),
            Box::new(payload()),
            Box::new(NixExpr::plain(NixKind::String("payload".into()))),
        ))),
        Box::new(body),
    ));
    let ast = NixExpr::plain(NixKind::List(vec![defaults, recursive]));
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value, after.value);
    assert_eq!(before.raw_nix.matches("rusix-sharing-evaluated").count(), 4);
    assert_eq!(after.raw_nix.matches("rusix-sharing-evaluated").count(), 2);
}

/// Compiler-generated bindings must not accidentally resolve an authored undefined variable.
#[test]
fn sharing_cannot_hide_undefined_variables() {
    let value = arithmetic("__rusix_shared_0");
    let ast = NixExpr::plain(NixKind::List(vec![value.clone(), value]));
    let session = NixSession::new().unwrap();
    for generated in [render_without_sharing(&ast), render(&ast)] {
        let error = session.evaluate(&generated).unwrap_err();
        assert_eq!(error.kind, crate::DiagnosticKind::Compiler);
        assert!(error.reason.contains("undefined variable"));
        assert!(error.raw_nix.contains("__rusix_shared_0"));
    }
}

/// Defaults may reference one another; separate default/body regions preserve Nix's argument semantics.
#[test]
fn native_defaults_and_existing_let_scopes_are_preserved() {
    let repeated = arithmetic("base");
    let body = NixExpr::plain(NixKind::Let(
        "local".into(),
        Box::new(arithmetic("chosen")),
        Box::new(NixExpr::plain(NixKind::List(vec![
            arithmetic("local"),
            arithmetic("local"),
        ]))),
    ));
    let choices = NixExpr::plain(NixKind::AttrSet(vec![
        (vec!["value".into()], repeated.clone()),
        (vec!["unused".into()], repeated),
    ]));
    let chosen = NixExpr::plain(NixKind::Call(
        Builtin::GetAttr,
        vec![NixExpr::plain(NixKind::String("value".into())), choices],
    ));
    let function = NixExpr::plain(NixKind::ArgumentFunction(
        vec![
            ("base".into(), Some(NixExpr::plain(NixKind::Int(2)))),
            ("chosen".into(), Some(chosen)),
        ],
        Box::new(body),
    ));
    let ast = NixExpr::plain(NixKind::Apply(
        Box::new(function),
        Box::new(NixExpr::plain(NixKind::AttrSet(vec![]))),
    ));
    let session = NixSession::new().unwrap();
    let before = session.evaluate(&render_without_sharing(&ast)).unwrap();
    let after = session.evaluate(&render(&ast)).unwrap();

    assert_eq!(before.value, serde_json::json!([26, 26]));
    assert_eq!(before.value, after.value);
}

/// Distinct authored failure sites must never be merged merely because their values match.
#[test]
fn different_origins_retain_distinct_failure_sites() {
    let value = arithmetic("missing");
    let mut first = NixExpr::plain(NixKind::List(vec![value.clone(), value]));
    first.origin = Some(Origin::new("first.rs", 4, 1, "first arithmetic"));
    let mut second = first.clone();
    second.origin = Some(Origin::new("second.rs", 9, 1, "second arithmetic"));
    // A lexical value keeps the generated expression statically valid; each
    // arithmetic operation fails dynamically because null is not a number.
    let ast = NixExpr::plain(NixKind::Let(
        "missing".into(),
        Box::new(NixExpr::plain(NixKind::Null)),
        Box::new(NixExpr::plain(NixKind::AttrSet(vec![
            (vec!["first".into()], first),
            (vec!["second".into()], second),
        ]))),
    ));
    let original = render_without_sharing(&ast);
    let optimized = render(&ast);
    let session = NixSession::new().unwrap();

    for name in ["first", "second"] {
        let before = session.evaluate_attribute(&original, name).unwrap_err();
        let after = session.evaluate_attribute(&optimized, name).unwrap_err();
        equivalent_diagnostics(&before, &after);
    }
}
