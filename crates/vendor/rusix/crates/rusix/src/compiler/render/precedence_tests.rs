//! Check Nix grammar and evaluation against explicitly grouped versions of the same AST.
use super::*;
use crate::NixSession;
use crate::compiler::ast::BinaryOp;

fn node(kind: NixKind) -> NixExpr {
    NixExpr::plain(kind)
}

fn variable(name: &str) -> NixExpr {
    node(NixKind::Variable(name.into()))
}

fn int(value: i64) -> NixExpr {
    node(NixKind::Int(value))
}

fn apply(function: NixExpr, argument: NixExpr) -> NixExpr {
    node(NixKind::Apply(Box::new(function), Box::new(argument)))
}

fn select(value: NixExpr, name: &str) -> NixExpr {
    node(NixKind::Select(Box::new(value), vec![name.into()]))
}

fn binary(op: BinaryOp, left: NixExpr, right: NixExpr) -> NixExpr {
    node(NixKind::Binary(op, Box::new(left), Box::new(right)))
}

fn lambda(argument: &str, body: NixExpr) -> NixExpr {
    node(NixKind::Lambda(argument.into(), Box::new(body)))
}

fn record(name: &str, value: NixExpr) -> NixExpr {
    node(NixKind::AttrSet(vec![(vec![name.into()], value)]))
}

fn rendered(value: &NixExpr) -> String {
    render(value)
        .source
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n")
}

/// A test oracle: explicitly group every node without changing its kind or children.
fn fully_grouped(value: &NixExpr) -> NixExpr {
    let mut value = value.clone();
    let child = |v: &mut NixExpr| *v = fully_grouped(v);
    match &mut value.kind {
        NixKind::List(items) | NixKind::Call(_, items) => items.iter_mut().for_each(child),
        NixKind::AttrSet(fields) => fields.iter_mut().for_each(|(_, value)| child(value)),
        NixKind::ArgumentFunction(parameters, body)
        | NixKind::OpenArgumentFunction(parameters, body) => {
            for (_, default) in parameters {
                if let Some(default) = default {
                    child(default);
                }
            }
            child(body);
        }
        NixKind::Group(value)
        | NixKind::Select(value, _)
        | NixKind::ArgumentSelect(value, _)
        | NixKind::Lambda(_, value)
        | NixKind::Function(_, value) => child(value),
        NixKind::Apply(left, right)
        | NixKind::Assert(left, right)
        | NixKind::Binary(_, left, right)
        | NixKind::Let(_, left, right) => {
            child(left);
            child(right);
        }
        NixKind::If(condition, yes, no) => {
            child(condition);
            child(yes);
            child(no);
        }
        _ => {}
    }

    node(NixKind::Group(Box::new(value)))
}

#[test]
fn application_spines_and_selection_paths_render_idiomatically() {
    let optional = apply(
        apply(select(variable("lib"), "optional"), variable("perlSupport")),
        select(variable("perlPackages"), "perl"),
    );
    assert_eq!(
        rendered(&optional),
        "lib.optional perlSupport perlPackages.perl"
    );
    let nested = select(select(variable("stdenv"), "hostPlatform"), "isDarwin");
    assert_eq!(rendered(&nested), "stdenv.hostPlatform.isDarwin");
    assert_eq!(
        rendered(&apply(variable("f"), apply(variable("g"), int(1)))),
        "f (g 1)"
    );
    assert_eq!(
        rendered(&select(apply(variable("f"), int(1)), "out")),
        "(f 1).out"
    );
    assert_eq!(
        rendered(&apply(variable("f"), record("k", int(1)))),
        "f { \"k\" = 1; }"
    );
    assert_eq!(
        rendered(&apply(variable("f"), lambda("x", variable("x")))),
        "f (x: x)"
    );
    assert_eq!(
        rendered(&node(NixKind::List(vec![optional]))),
        "[ (lib.optional perlSupport perlPackages.perl) ]"
    );
}

#[test]
fn every_supported_binary_operator_preserves_same_strength_association() {
    for (op, token, left_groups, right_groups) in [
        (BinaryOp::Add, "+", false, true),
        (BinaryOp::AttrMerge, "//", true, false),
        (BinaryOp::And, "&&", false, true),
        (BinaryOp::Equal, "==", true, true),
        (BinaryOp::GreaterEqual, ">=", true, true),
        (BinaryOp::LessEqual, "<=", true, true),
    ] {
        let left = binary(op, binary(op, variable("a"), variable("b")), variable("c"));
        let right = binary(op, variable("a"), binary(op, variable("b"), variable("c")));
        let grouped = |value: String, needed| if needed { format!("({value})") } else { value };
        assert_eq!(
            rendered(&left),
            format!("{} {token} c", grouped(format!("a {token} b"), left_groups))
        );
        assert_eq!(
            rendered(&right),
            format!(
                "a {token} {}",
                grouped(format!("b {token} c"), right_groups)
            )
        );
    }
}

#[test]
fn supported_grammar_categories_parse_in_every_parent_context_in_both_modes() {
    let origin = Origin::new("precedence.rs", 1, 1, "grammar probe");
    let mut expressions = vec![
        node(NixKind::Bool(true)),
        int(-42),
        int(i64::MIN),
        node(NixKind::Float(-1.5)),
        node(NixKind::Float(1.0)),
        node(NixKind::Null),
        node(NixKind::String("literal".into())),
        node(NixKind::Path("./source.path".into())),
        variable("a"),
        node(NixKind::List(vec![int(1)])),
        record("k", int(1)),
        node(NixKind::Group(Box::new(variable("a")))),
        apply(variable("f"), variable("a")),
        select(variable("a"), "k"),
        node(NixKind::ArgumentSelect(Box::new(variable("a")), vec![])),
        node(NixKind::ArgumentSelect(
            Box::new(variable("a")),
            vec!["odd.name".into()],
        )),
        lambda("x", variable("x")),
        node(NixKind::Function(vec!["x".into()], Box::new(variable("x")))),
        node(NixKind::ArgumentFunction(
            vec![("x".into(), Some(variable("a")))],
            Box::new(variable("x")),
        )),
        node(NixKind::If(
            Box::new(variable("a")),
            Box::new(variable("b")),
            Box::new(variable("c")),
        )),
        node(NixKind::Assert(
            Box::new(variable("a")),
            Box::new(variable("b")),
        )),
        node(NixKind::Let(
            "x".into(),
            Box::new(variable("a")),
            Box::new(variable("x")),
        )),
    ];
    for builtin in [
        Builtin::Div,
        Builtin::Throw,
        Builtin::Import,
        Builtin::GetAttr,
        Builtin::ToPath,
        Builtin::ToString,
    ] {
        expressions.push(node(NixKind::Call(builtin, vec![variable("a")])));
        expressions.push(node(NixKind::Call(builtin, vec![])));
    }
    let operators = [
        BinaryOp::Equal,
        BinaryOp::GreaterEqual,
        BinaryOp::LessEqual,
        BinaryOp::And,
        BinaryOp::Add,
        BinaryOp::AttrMerge,
    ];
    for op in operators {
        expressions.push(binary(op, variable("a"), variable("b")));
    }
    let mut fields = Vec::new();
    for expression in expressions {
        // Both attributed and bare bases must obey the same grammar, including paths.
        for attributed in [false, true] {
            let mut expression = expression.clone();
            if attributed {
                expression.origin = Some(origin.clone());
            }
            let mut parents = vec![
                expression.clone(),
                apply(variable("f"), expression.clone()),
                apply(expression.clone(), variable("a")),
                select(expression.clone(), "k"),
                node(NixKind::List(vec![expression.clone(), variable("b")])),
                node(NixKind::If(
                    Box::new(expression.clone()),
                    Box::new(variable("b")),
                    Box::new(variable("c")),
                )),
                node(NixKind::ArgumentFunction(
                    vec![("x".into(), Some(expression.clone()))],
                    Box::new(variable("x")),
                )),
            ];
            for op in operators {
                parents.push(binary(op, expression.clone(), variable("a")));
                parents.push(binary(op, variable("a"), expression.clone()));
            }
            for body in parents {
                let body = lambda("f", lambda("a", lambda("b", lambda("c", body))));
                fields.push((vec![format!("case{}", fields.len())], body));
            }
        }
    }

    assert_eq!(fields.len(), 1520);
    let ast = node(NixKind::AttrSet(fields));
    let session = NixSession::new().unwrap();
    for options in [
        RenderOptions::default(),
        RenderOptions {
            layout: RenderLayout::Compact,
            ..Default::default()
        },
        RenderOptions {
            origin_comments: true,
            ..Default::default()
        },
    ] {
        let generated = render_with_options(&ast, options);
        session
            .validate_generated(&generated, "grammar.nix")
            .unwrap();
    }
}

#[test]
fn grouped_oracle_and_minimal_rendering_evaluate_identically_in_both_modes() {
    let truth = || node(NixKind::Bool(true));
    let falsity = || node(NixKind::Bool(false));
    let throw = || {
        node(NixKind::Call(
            Builtin::Throw,
            vec![node(NixKind::String("must remain lazy".into()))],
        ))
    };
    let sum = || {
        lambda(
            "x",
            lambda("y", binary(BinaryOp::Add, variable("x"), variable("y"))),
        )
    };
    let identity = || lambda("x", variable("x"));
    let merge = |left, right| binary(BinaryOp::AttrMerge, left, right);
    let float = |value| node(NixKind::Float(value));
    let choices = vec![
        apply(apply(sum(), int(3)), int(4)),
        apply(identity(), apply(apply(sum(), int(3)), int(4))),
        select(
            apply(lambda("x", record("out", variable("x"))), int(4)),
            "out",
        ),
        apply(lambda("x", select(variable("x"), "k")), record("k", int(4))),
        apply(apply(identity(), identity()), int(4)),
        node(NixKind::List(vec![apply(identity(), int(4)), int(5)])),
        apply(
            identity(),
            node(NixKind::If(
                Box::new(truth()),
                Box::new(int(4)),
                Box::new(throw()),
            )),
        ),
        select(
            node(NixKind::If(
                Box::new(truth()),
                Box::new(record("k", int(4))),
                Box::new(throw()),
            )),
            "k",
        ),
        apply(identity(), binary(BinaryOp::Add, int(3), int(4))),
        binary(
            BinaryOp::Equal,
            binary(BinaryOp::Equal, int(3), int(3)),
            truth(),
        ),
        binary(
            BinaryOp::Equal,
            truth(),
            binary(BinaryOp::Equal, int(3), int(3)),
        ),
        binary(BinaryOp::And, falsity(), throw()),
        select(
            merge(
                merge(record("k", int(1)), record("k", int(2))),
                record("k", int(3)),
            ),
            "k",
        ),
        select(
            merge(
                record("k", int(1)),
                merge(record("k", int(2)), record("k", int(3))),
            ),
            "k",
        ),
        binary(
            BinaryOp::Equal,
            binary(BinaryOp::GreaterEqual, int(3), int(2)),
            truth(),
        ),
        binary(
            BinaryOp::LessEqual,
            binary(BinaryOp::Add, int(1), int(2)),
            int(3),
        ),
        binary(
            BinaryOp::And,
            binary(BinaryOp::Equal, int(2), int(2)),
            binary(BinaryOp::LessEqual, int(1), int(2)),
        ),
        binary(
            BinaryOp::Add,
            binary(BinaryOp::Add, float(1e16), float(-1e16)),
            float(1.0),
        ),
        binary(
            BinaryOp::Add,
            float(1e16),
            binary(BinaryOp::Add, float(-1e16), float(1.0)),
        ),
        apply(identity(), int(-42)),
        apply(identity(), int(i64::MIN)),
        apply(identity(), float(-1.5)),
        apply(identity(), float(1.0)),
        node(NixKind::Call(Builtin::Div, vec![int(-42), int(2)])),
        apply(node(NixKind::Call(Builtin::ToString, vec![])), int(42)),
        node(NixKind::Assert(Box::new(truth()), Box::new(int(4)))),
        node(NixKind::Let(
            "x".into(),
            Box::new(int(4)),
            Box::new(apply(identity(), variable("x"))),
        )),
        apply(
            node(NixKind::ArgumentFunction(
                vec![
                    ("x".into(), Some(identity())),
                    ("y".into(), Some(apply(variable("x"), int(4)))),
                ],
                Box::new(variable("y")),
            )),
            node(NixKind::AttrSet(vec![])),
        ),
        apply(
            node(NixKind::Function(vec!["x".into()], Box::new(variable("x")))),
            record("x", int(4)),
        ),
        node(NixKind::Call(
            Builtin::GetAttr,
            vec![node(NixKind::String("a.b".into())), record("a.b", int(4))],
        )),
        select(record("or", int(4)), "or"),
        node(NixKind::ArgumentSelect(
            Box::new(record("pkg-config", record("odd.name", int(4)))),
            vec!["pkg-config".into(), "odd.name".into()],
        )),
        binary(
            BinaryOp::Add,
            node(NixKind::String("a".into())),
            binary(
                BinaryOp::Add,
                node(NixKind::String("b".into())),
                node(NixKind::String("c".into())),
            ),
        ),
        apply(
            lambda(
                "deterministic-host-uname",
                variable("deterministic-host-uname"),
            ),
            int(4),
        ),
    ];
    let expected = serde_json::json!([
        7,
        7,
        4,
        4,
        4,
        [4, 5],
        4,
        4,
        7,
        true,
        true,
        false,
        3,
        3,
        true,
        true,
        true,
        1.0,
        0.0,
        -42,
        i64::MIN,
        -1.5,
        1.0,
        -21,
        "42",
        4,
        4,
        4,
        4,
        4,
        4,
        4,
        "abc",
        4
    ]);
    let actual = node(NixKind::List(choices));
    let pair = node(NixKind::AttrSet(vec![
        (vec!["minimal".into()], actual.clone()),
        (vec!["grouped".into()], fully_grouped(&actual)),
    ]));
    let session = NixSession::new().unwrap();
    for options in [
        RenderOptions::default(),
        RenderOptions {
            layout: RenderLayout::Compact,
            ..Default::default()
        },
        RenderOptions {
            origin_comments: true,
            ..Default::default()
        },
    ] {
        let evaluated = session
            .evaluate(&render_with_options(&pair, options))
            .unwrap()
            .value;
        assert_eq!(evaluated["minimal"], expected);
        assert_eq!(evaluated["grouped"], expected);
    }
}

#[test]
fn path_tokens_and_signed_arguments_keep_necessary_grouping() {
    for root in [
        NixKind::Select(
            Box::new(node(NixKind::Path("./source.path".into()))),
            vec![],
        ),
        NixKind::ArgumentSelect(
            Box::new(node(NixKind::Path("./source.path".into()))),
            vec![],
        ),
    ] {
        assert_eq!(rendered(&select(node(root), "k")), "(./source.path).k");
    }

    assert_eq!(
        rendered(&select(node(NixKind::Path("./source.path".into())), "k")),
        "(./source.path).k"
    );
    assert_eq!(rendered(&apply(variable("f"), int(-42))), "f (-42)");
    assert_eq!(
        rendered(&apply(variable("f"), node(NixKind::Float(-1.5)))),
        "f (-1.5)"
    );
    assert_eq!(
        rendered(&binary(BinaryOp::Add, int(-42), int(1))),
        "-42 + 1"
    );
    assert_eq!(
        rendered(&binary(
            BinaryOp::Add,
            node(NixKind::Path("./source.path".into())),
            node(NixKind::String("/child".into()))
        )),
        "./source.path + \"/child\""
    );
}

#[test]
fn attributed_application_prefixes_keep_all_occurrence_spans() {
    let head = NixExpr::attributed(
        NixKind::Variable("f".into()),
        Origin::new("spine.rs", 1, 1, "function"),
    );
    let first = NixExpr::attributed(
        NixKind::Apply(Box::new(head), Box::new(int(1))),
        Origin::new("spine.rs", 2, 1, "first application"),
    );
    let outer = NixExpr::attributed(
        NixKind::Apply(Box::new(first), Box::new(int(2))),
        Origin::new("spine.rs", 3, 1, "second application"),
    );
    for options in [
        RenderOptions::default(),
        RenderOptions {
            layout: RenderLayout::Compact,
            ..Default::default()
        },
        RenderOptions {
            origin_comments: true,
            ..Default::default()
        },
    ] {
        let generated = render_with_options(&outer, options);
        assert_eq!(generated.spans.len(), 3);
        let head = &generated.spans[0];
        let first = &generated.spans[1];
        let outer = &generated.spans[2];
        assert_eq!(&generated.source[head.start..head.end], "f");
        assert!(first.start <= head.start && first.end >= head.end);
        assert!(outer.start <= first.start && outer.end >= first.end);
        assert_eq!(first.enclosing.len(), 1);
        assert_eq!(head.enclosing.len(), 2);
        assert!(first.diagnostic_site && outer.diagnostic_site);
    }
}

#[test]
fn source_map_selection_failures_choose_the_consumer_without_parenthesis_markers() {
    let variable_origin = Origin::new("lookup.rs", 1, 1, "argument");
    let lookup_origin = Origin::new("lookup.rs", 2, 1, "lookup");
    let session = NixSession::new().unwrap();
    for alias in [false, true] {
        let mut base =
            NixExpr::attributed(NixKind::Variable("args".into()), variable_origin.clone());
        if alias {
            base = NixExpr::attributed(
                NixKind::ArgumentSelect(Box::new(base), vec![]),
                Origin::new("lookup.rs", 3, 1, "view root"),
            );
        }
        let lookup = NixExpr::attributed(
            NixKind::Select(Box::new(base), vec!["missing".into()]),
            lookup_origin.clone(),
        );
        let root = node(NixKind::Let(
            "args".into(),
            Box::new(node(NixKind::AttrSet(vec![]))),
            Box::new(lookup),
        ));
        for options in [
            RenderOptions::default(),
            RenderOptions {
                layout: RenderLayout::Compact,
                ..Default::default()
            },
            RenderOptions {
                origin_comments: true,
                ..Default::default()
            },
        ] {
            let generated = render_with_options(&root, options);
            let error = session.evaluate(&generated).unwrap_err();
            assert_eq!(error.primary.as_ref(), Some(&lookup_origin));
            assert_eq!(error.provenance, crate::Provenance::SourceMap);
            assert_eq!(
                generated.origin(&variable_origin.id),
                Some(&variable_origin)
            );
            assert!(!generated.source.contains("addErrorContext"));
        }
    }
}

#[test]
fn independently_attributed_selection_steps_keep_the_failing_step() {
    let first = Origin::new("steps.rs", 1, 1, "first lookup");
    let second = Origin::new("steps.rs", 2, 1, "second lookup");
    let session = NixSession::new().unwrap();
    for (base, expected) in [
        (record("present", node(NixKind::AttrSet(vec![]))), &second),
        (node(NixKind::AttrSet(vec![])), &first),
    ] {
        let inner = NixExpr::attributed(
            NixKind::Select(Box::new(base), vec!["present".into()]),
            first.clone(),
        );
        let outer = NixExpr::attributed(
            NixKind::Select(Box::new(inner), vec!["missing".into()]),
            second.clone(),
        );
        for options in [
            RenderOptions::default(),
            RenderOptions {
                layout: RenderLayout::Compact,
                ..Default::default()
            },
            RenderOptions {
                origin_comments: true,
                ..Default::default()
            },
        ] {
            let generated = render_with_options(&outer, options);
            let error = session.evaluate(&generated).unwrap_err();
            assert_eq!(
                error.primary.as_ref(),
                Some(expected),
                "{}",
                generated.source
            );
            assert_eq!(error.provenance, crate::Provenance::SourceMap);
        }
    }
}
