//! Compare real failures across runtime-boundary policies and inspection rendering.
use super::*;
use crate::interop::raw::{InputRefExt, NixFunctionExt};
use crate::{
    Expr,
    interop::{InputRef, Nixpkgs, raw::NixValue},
    ir::Origin,
    nix_text,
};
use std::path::Path;

#[rusix::args]
mod args {
    #[rusix(root)]
    struct Inputs {
        /// Absent boolean argument used to check where a deferred Nix lookup failure is reported.
        missing: bool,
        /// Nested argument view used to exercise failures inside a supplied platform description.
        platform: Platform,
    }

    struct Platform {
        /// Absent platform string used to check the diagnostic for a nested deferred lookup.
        missing: String,
    }
}

// Test-only AST controls: no generated-source substitution or source-map edits.
pub(crate) fn contexts(expr: &mut NixExpr, legacy: bool) {
    expr.error_context = legacy
        && expr.origin.is_some()
        && (expr.error_context
            || matches!(
                expr.kind,
                NixKind::If(..)
                    | NixKind::Binary(BinaryOp::Equal | BinaryOp::Add, ..)
                    | NixKind::Call(Builtin::Div | Builtin::ToString | Builtin::GetAttr, ..)
                    | NixKind::Apply(..)
                    | NixKind::Select(..)
                    | NixKind::ArgumentSelect(..)
            ));

    match &mut expr.kind {
        NixKind::List(items) | NixKind::Call(_, items) => {
            items.iter_mut().for_each(|v| contexts(v, legacy));
        }
        NixKind::AttrSet(fields) => fields.iter_mut().for_each(|(_, v)| contexts(v, legacy)),
        NixKind::ArgumentFunction(defaults, body)
        | NixKind::OpenArgumentFunction(defaults, body) => {
            for (_, default) in defaults {
                if let Some(value) = default {
                    contexts(value, legacy);
                }
            }
            contexts(body, legacy);
        }
        NixKind::Group(value)
        | NixKind::Select(value, _)
        | NixKind::ArgumentSelect(value, _)
        | NixKind::Lambda(_, value)
        | NixKind::Function(_, value) => contexts(value, legacy),
        NixKind::Apply(left, right)
        | NixKind::Assert(left, right)
        | NixKind::Binary(_, left, right)
        | NixKind::Let(_, left, right) => {
            contexts(left, legacy);
            contexts(right, legacy);
        }
        NixKind::If(condition, yes, no) => {
            contexts(condition, legacy);
            contexts(yes, legacy);
            contexts(no, legacy);
        }
        _ => {}
    }
}

fn origin(value: NixValue) -> Origin {
    Config::new().set_dynamic("value", value).assignments[0]
        .value
        .origin
        .clone()
}

/// One demanded failure, with its expected operation rather than its enclosing container.
struct Case {
    /// Identifies the expression category in a failed assertion.
    name: &'static str,
    /// Complete deferred computation evaluated in both rendered policies.
    value: NixValue,
    /// The accessor, consumer or interop boundary that should receive Rust blame.
    expected: Origin,
    /// Expected way to recover the Rust location, such as an error trace or generated text range.
    provenance: Provenance,
}

fn case(name: &'static str, value: NixValue, provenance: Provenance) -> Case {
    Case {
        name,
        expected: origin(value.clone()),
        value,
        provenance,
    }
}

fn empty() -> NixValue {
    NixValue::record([] as [(&str, NixValue); 0])
}

/// Compare semantic diagnostics while retaining each rendering's original Nix trace.
pub(super) fn equivalent_diagnostics(normal: &Diagnostic, debug: &Diagnostic) {
    let comparable = |diagnostic: &Diagnostic| {
        let mut value = serde_json::to_value(diagnostic).unwrap();
        value.as_object_mut().unwrap().remove("raw_nix");
        value
    };
    assert_eq!(comparable(normal), comparable(debug));
    assert!(!normal.raw_nix.is_empty());
    assert!(!debug.raw_nix.is_empty());

    // The wire reason must also agree: generated locations/excerpts may differ.
    let reason = |diagnostic: &Diagnostic| {
        Diagnostic::from_nix(
            DiagnosticKind::NixEval,
            &diagnostic.raw_nix,
            &Generated::default(),
            Path::new("no-generated-source"),
        )
        .reason
    };
    assert_eq!(reason(normal), reason(debug));
}

#[test]
fn diagnostic_matrix_preserves_operations_paths_and_raw_errors() {
    let input = args::from_value(NixValue::record([("platform", empty())]));
    let missing = input.missing();
    let nested = input.platform.missing();
    let condition = NixValue::if_else("not bool", true, false);
    let division: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let equality = division.clone().equals(1_i64);
    let prefix: NixValue = NixValue::from(1_i64)
        .into_expr::<String>()
        .with_prefix("prefix")
        .into();
    let library = Nixpkgs::new()
        .function("concatStringsSep")
        .apply(["".into(), 42_i64.into()]);
    let opaque = Nixpkgs::new()
        .pkgs_function("writeText")
        .apply(["config".into(), empty()]);
    // mkDerivation returns a lazy record. Demand drvPath to test its actual recipe.
    let derivation = Nixpkgs::new()
        .pkgs_function("stdenv.mkDerivation")
        .call(NixValue::record([("name", empty())]))
        .select("drvPath");
    let validation: NixValue = Expr::int(3).in_range(0, 1, "out of range").into();
    let mut callback_origin = None;
    let callback = NixValue::function_attrs(["input"], |args| {
        (
            Vec::<(&str, NixValue)>::new(),
            NixValue::function(|_| {
                let missing = args.select("input.missing");
                callback_origin = Some(origin(missing.clone()));
                missing
            })
            .call(NixValue::null()),
        )
    })
    .call(NixValue::record([("input", empty())]));
    let input = InputRef::local(
        "audit",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/nix-interop-input.nix"),
    );
    let delayed = input.function("functions.lazy").call(true).select("bad");
    let imported = input.value("values.lazy.bad");
    let mut final_attrs_origin = None;
    let final_attrs = Nixpkgs::new()
        .pkgs_function("stdenv.mkDerivation")
        .call(NixValue::function(|attrs| {
            let missing = attrs.select("missing");
            final_attrs_origin = Some(origin(missing.clone()));
            NixValue::record([("name", missing)])
        }))
        .select("drvPath");

    let cases = [
        case("args missing", missing.into(), Provenance::SourceMap),
        case("nested missing", nested.into(), Provenance::SourceMap),
        case("condition", condition, Provenance::SourceMap),
        Case {
            name: "equality child",
            value: equality,
            expected: origin(division.clone()),
            provenance: Provenance::SourceMap,
        },
        case("division", division.clone(), Provenance::SourceMap),
        case("coercion", empty().to_text(), Provenance::SourceMap),
        case("prefix coercion", prefix, Provenance::ErrorContext),
        case("library argument", library, Provenance::ErrorContext),
        case("opaque function", opaque, Provenance::ErrorContext),
        case("mkDerivation recipe", derivation, Provenance::SourceMap),
        case("validation", validation, Provenance::ErrorContext),
        Case {
            name: "nested callback",
            value: callback,
            expected: callback_origin.unwrap(),
            provenance: Provenance::SourceMap,
        },
        case("delayed external value", delayed, Provenance::SourceMap),
        case("imported Nix", imported, Provenance::ErrorContext),
        case(
            "missing package",
            Nixpkgs::new().get("rusixMissing").into(),
            Provenance::ErrorContext,
        ),
        case(
            "missing function",
            Nixpkgs::new().function("rusixMissing").into(),
            Provenance::ErrorContext,
        ),
        case(
            "lazy guard",
            Nixpkgs::new()
                .library()
                .throw_if_not(false, "guard rejected", empty()),
            Provenance::ErrorContext,
        ),
        Case {
            name: "nested list child",
            value: NixValue::list([NixValue::list([division.clone()])]),
            expected: origin(division.clone()),
            provenance: Provenance::SourceMap,
        },
        Case {
            name: "interpolation child",
            value: nix_text!("port={port}", port = division.clone()).into(),
            expected: origin(division.clone()),
            provenance: Provenance::SourceMap,
        },
        Case {
            name: "mkDerivation finalAttrs callback",
            value: final_attrs,
            expected: final_attrs_origin.unwrap(),
            provenance: Provenance::SourceMap,
        },
    ];
    let session = NixSession::new().unwrap();

    for case in cases {
        let config = Config::new().set_dynamic("package.buildInputs", case.value);
        let ast = lower(&config);
        let current = render(&ast);
        let debug = render_with_options(
            &ast,
            RenderOptions {
                origin_comments: true,
                ..Default::default()
            },
        );
        let mut legacy = lower(&config);
        contexts(&mut legacy, true);
        let legacy = render(&legacy);
        let before = session.evaluate_interop(&legacy).unwrap_err();
        let after = session.evaluate_interop(&current).unwrap_err();
        let annotated = session.evaluate_interop(&debug).unwrap_err();
        equivalent_diagnostics(&after, &annotated);

        for diagnostic in [&before, &after] {
            assert_eq!(
                diagnostic.primary.as_ref(),
                Some(&case.expected),
                "{}: {diagnostic:?}",
                case.name
            );
            assert!(
                diagnostic
                    .related
                    .iter()
                    .any(|o| o.purpose == "set package.buildInputs"),
                "{}",
                case.name
            );
            assert!(!diagnostic.raw_nix.is_empty(), "{}", case.name);
        }
        assert_eq!(after.provenance, case.provenance, "{}", case.name);
        assert_eq!(before.reason, after.reason, "{}", case.name);

        // Exercise the legacy textual adapter on the same real evaluator event.
        let event = after
            .raw_nix
            .lines()
            .filter_map(|l| l.strip_prefix("@nix "))
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .find(|e| e["level"] == 0 && e["raw_msg"].is_string())
            .unwrap();
        let text = Diagnostic::from_nix(
            DiagnosticKind::NixEval,
            event["msg"].as_str().unwrap(),
            &current,
            &session.root().join("generated.nix"),
        );
        assert_eq!(text.primary, after.primary, "{}: {text:?}", case.name);
        assert!(
            text.related
                .iter()
                .any(|o| o.purpose == "set package.buildInputs")
        );
    }
}

#[test]
fn boundary_contexts_have_real_counterexamples_without_markers() {
    let library = Nixpkgs::new()
        .function("concatStringsSep")
        .apply(["".into(), 42_i64.into()]);
    let prefix: NixValue = NixValue::from(1_i64)
        .into_expr::<String>()
        .with_prefix("prefix")
        .into();
    let session = NixSession::new().unwrap();

    for value in [library, prefix] {
        let config = Config::new().set_dynamic("result", value);
        let expected = config.assignments[0].value.origin.clone();
        let mut ast = lower(&config);
        let boundary = session.evaluate_interop(&render(&ast)).unwrap_err();
        contexts(&mut ast, false);
        let unwrapped = session.evaluate_interop(&render(&ast)).unwrap_err();

        assert_eq!(boundary.primary, Some(expected.clone()));
        assert_ne!(unwrapped.primary, Some(expected));
        assert_eq!(boundary.reason, unwrapped.reason);
    }
}

#[test]
fn curried_calls_have_one_boundary_but_distinct_operations_keep_theirs() {
    let function = Nixpkgs::new().function("concatStringsSep");
    let applied = function
        .clone()
        .apply(["/".into(), NixValue::list(["a".into(), "b".into()])]);
    let first = function.call("/");
    let second = first.call(NixValue::list(["a".into(), "b".into()]));
    let applied = compile(Config::new().set_dynamic("result", applied)).unwrap();
    let distinct = compile(Config::new().set_dynamic("result", second)).unwrap();

    // The imported function boundary is shared; .apply needs only one extra marker.
    assert_eq!(applied.source.matches("addErrorContext").count(), 2);
    assert_eq!(distinct.source.matches("addErrorContext").count(), 3);
    assert_eq!(applied.spans.len(), distinct.spans.len());
    let session = NixSession::new().unwrap();
    assert_eq!(
        session.evaluate_interop(&applied).unwrap().value["result"],
        "a/b"
    );
    assert_eq!(
        session.evaluate_interop(&distinct).unwrap().value["result"],
        "a/b"
    );
}

#[test]
fn persisted_source_maps_without_site_metadata_remain_readable() {
    let generated =
        compile(Config::new().set_dynamic("value", Expr::int(1).divide(Expr::int(0)))).unwrap();
    let mut old = serde_json::to_value(&generated).unwrap();
    for span in old["spans"].as_array_mut().unwrap() {
        span.as_object_mut().unwrap().remove("diagnostic_site");
    }
    let old: Generated = serde_json::from_value(old).unwrap();

    assert_eq!(old.source, generated.source);
    assert_eq!(old.spans.len(), generated.spans.len());
    let error = NixSession::new().unwrap().evaluate(&old).unwrap_err();
    assert_eq!(error.primary.as_ref().unwrap().purpose, "integer division");
    assert_eq!(error.provenance, Provenance::SourceMap);
    assert!(error.related.iter().any(|o| o.purpose == "set value"));
}
