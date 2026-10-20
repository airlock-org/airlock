#![cfg(feature = "evaluation")]

//! Argument views share existing selections, typed expressions and lazy native functions.
use rusix::interop::raw::NixFunctionExt;
use rusix::interop::raw::NixRepresentation;
use rusix::{
    Config, Expr,
    interop::{InputRef, Nixpkgs, raw::NixValue},
    ir::ValueKind,
};
use rusix::{Generated, NixSession, Provenance, compile};
use std::fs;

#[rusix::args]
#[allow(dead_code)]
mod interface {
    use rusix::interop::raw::NixValue;

    #[rusix(root, rename_all = "PascalCase")]
    struct Inputs {
        name: String,
        label: String,
        #[rusix(rename = "pkg-config")]
        pkg_config: NixValue,
        platform: Platform,
    }

    struct Platform {
        system: String,
    }
}

#[rusix::args]
mod args {
    use rusix::interop::raw::NixValue;
    use std::collections::{BTreeMap, HashMap};

    #[rusix(root)]
    struct Inputs {
        platform: Platform,
        second: Platform,
        package: NixValue,
        names: Vec<String>,
        optional: Option<String>,
        values: BTreeMap<String, i64>,
        hash_values: HashMap<String, bool>,
        #[rusix(rename = "literal.node ${key}\"")]
        literal: Flags,
    }

    #[rusix(value)]
    struct Platform {
        is_linux: bool,
        system: String,
        count: i64,
        missing: String,
    }

    #[rusix(rename_all = "PascalCase")]
    struct Flags {
        other_flag: bool,
        #[rusix(rename = "with space")]
        special_flag: String,
    }
}

fn platform(count: i64) -> NixValue {
    NixValue::record([
        ("isLinux", true.into()),
        ("system", "x86_64-linux".into()),
        ("count", count.into()),
    ])
}

fn input() -> NixValue {
    NixValue::record([
        ("platform", platform(42)),
        ("second", platform(7)),
        ("package", Nixpkgs::new().get("hello").into()),
        ("names", NixValue::list(["a".into(), "b".into()])),
        ("optional", NixValue::null()),
        ("values", NixValue::record([("x", 1_i64.into())])),
        ("hashValues", NixValue::record([("y", false.into())])),
        (
            "literal.node ${key}\"",
            NixValue::record([
                ("OtherFlag", true.into()),
                ("with space", "literal key".into()),
            ]),
        ),
    ])
}

fn artifact(value: NixValue) -> Generated {
    compile(Config::new().set_dynamic("result", value)).unwrap()
}

fn evaluate(value: NixValue) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate_interop(&artifact(value))
        .unwrap()
        .value["result"]
        .clone()
}

#[test]
fn declared_argument_names_preserve_mapping_and_native_dependent_defaults() {
    assert_eq!(
        args::argument_names(),
        [
            "platform",
            "second",
            "package",
            "names",
            "optional",
            "values",
            "hashValues",
            "literal.node ${key}\"",
        ]
    );
    assert_eq!(
        interface::argument_names(),
        ["Name", "Label", "pkg-config", "Platform"]
    );

    let factory = rusix::interop::PackageFunction::from_function_attrs(
        interface::argument_names().iter().copied(),
        |value| {
            let input = interface::from_value(value);

            (vec![("Label", input.name().into())], input.label())
        },
    );
    assert_eq!(
        evaluate(NixValue::builtin("functionArgs").call(factory.as_expression())),
        serde_json::json!({"Name": false, "Label": true, "pkg-config": false, "Platform": false})
    );
    let arguments = rusix::nix_record! {
        "Name": "Git",
        "pkg-config": Expr::int(1).divide(Expr::int(0)),
        "Platform": NixValue::record([] as [(&str, NixValue); 0]),
    };
    assert_eq!(
        evaluate(factory.as_expression().call(arguments.clone())),
        "Git"
    );
    assert_eq!(
        evaluate(
            factory
                .as_expression()
                .call(arguments.merge_attrs(rusix::nix_record! {
                    "Label": "explicit",
                }))
        ),
        "explicit"
    );
}

#[test]
fn construction_nested_navigation_clone_and_expected_leaf_types() {
    let input = args::from_value(input());
    let cloned = input.clone();
    let _: Expr<bool> = input.platform.is_linux();
    let _: Expr<String> = input.platform.system();
    let _: Expr<i64> = input.platform.count();
    let _: NixValue = input.package();
    let _: NixValue = input.names();
    let _: NixValue = input.optional();
    let _: NixValue = input.values();
    let _: NixValue = input.hash_values();

    assert_eq!(
        evaluate(NixValue::list([
            input.platform.count().into(),
            cloned.second.count().into(),
        ])),
        serde_json::json!([42, 7]),
    );
}

#[test]
fn scalar_opaque_and_collection_leaves_evaluate_without_materializing_in_rust() {
    let input = args::from_value(input());
    let value = NixValue::record([
        ("linux", input.platform.is_linux().into()),
        ("system", input.platform.system().into()),
        ("count", input.platform.count().into()),
        ("package", input.package().select("pname")),
        ("names", input.names()),
        ("optional", input.optional()),
        ("values", input.values()),
        ("hashValues", input.hash_values()),
        ("platform", input.platform.as_value()),
    ]);

    assert_eq!(
        evaluate(value),
        serde_json::json!({
            "linux": true, "system": "x86_64-linux", "count": 42,
            "package": "hello", "names": ["a", "b"], "optional": null,
            "values": {"x": 1}, "hashValues": {"y": false},
            "platform": {"isLinux": true, "system": "x86_64-linux", "count": 42},
        }),
    );
}

#[test]
fn naming_and_special_character_segments_are_literal_data() {
    let input = args::from_value(input());

    assert_eq!(
        evaluate(NixValue::list([
            input.literal.other_flag().into(),
            input.literal.special_flag().into(),
        ])),
        serde_json::json!([true, "literal key"]),
    );
}

#[test]
fn leaf_and_subtree_origins_belong_to_the_accessor_call_not_construction() {
    let input = args::from_value(input());
    let line = line!() + 1;
    let reference = input.platform.count();
    let config = Config::new().set_dynamic("result", reference);
    let node = &config.assignments[0].value;
    assert_eq!(node.origin.line, line);
    assert_eq!(node.origin.file, file!());
    let ValueKind::Select(_, path) = &node.kind else {
        panic!("argument view must use ordinary selection IR")
    };
    assert_eq!(path.parts(), &["platform", "count"]);

    let line = line!() + 1;
    let subtree = input.platform.as_value();
    let config = Config::new().set_dynamic("result", subtree);
    assert_eq!(config.assignments[0].value.origin.line, line);
}

#[test]
fn missing_argument_selection_maps_to_accessor_and_preserves_nix_trace() {
    let input = args::from_value(input());
    let line = line!() + 1;
    let missing = input.platform.missing();
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&artifact(missing.into()))
        .unwrap_err();

    let origin = diagnostic.primary.as_ref().unwrap();
    assert_eq!(origin.line, line);
    assert_eq!(origin.file, file!());
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
    assert!(diagnostic.reason.contains("missing"));
    assert!(!diagnostic.raw_nix.is_empty());
}

#[test]
fn typed_consuming_operation_keeps_its_more_precise_origin() {
    let input = args::from_value(NixValue::record([("platform", platform(0))]));
    let line = line!() + 1;
    let division = Expr::int(42).divide(input.platform.count());
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&artifact(division.into()))
        .unwrap_err();

    assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
    assert_eq!(diagnostic.reason, "division by zero");
}

#[test]
fn unused_views_and_unselected_leaves_do_not_force_argument_values() {
    let _unused = args::from_value(Expr::int(1).divide(Expr::int(0)).into());
    let factory = NixValue::function_attrs(["platform", "package"], |value| {
        let input = args::from_value(value);
        (
            vec![("package", Expr::int(1).divide(Expr::int(0)).into())],
            NixValue::record([
                ("good", input.platform.count().into()),
                ("bad", input.package()),
            ]),
        )
    });
    let result = factory.call(NixValue::record([("platform", platform(42))]));
    let artifact = compile(
        Config::new()
            .set_dynamic("good", result.clone().select("good"))
            .set_dynamic("bad", result.select("bad")),
    )
    .unwrap();
    let session = NixSession::new().unwrap();
    assert!(!artifact.source.contains("deepSeq"));

    assert_eq!(
        session.evaluate_attribute(&artifact, "good").unwrap().value,
        42,
    );
    assert_eq!(
        session
            .evaluate_attribute(&artifact, "bad")
            .unwrap_err()
            .reason,
        "division by zero",
    );
}

#[test]
fn same_artifact_follows_ordinary_nix_caller_overrides_without_relowering() {
    let factory = NixValue::function_attrs(["platform"], |value| {
        let input = args::from_value(value);
        (
            vec![("platform", platform(42))],
            input
                .platform
                .count()
                .to_text()
                .with_prefix("count=")
                .into(),
        )
    });
    let scratch = tempfile::tempdir().unwrap();
    let caller = scratch.path().join("caller.nix");
    let artifact = artifact(
        InputRef::local("args-caller", &caller)
            .function("invoke")
            .call(factory),
    );
    let original_source = artifact.source.clone();

    for (source, expected) in [
        ("{ invoke = factory: factory {}; }", "count=42"),
        (
            "{ invoke = factory: factory { platform.count = 7; }; }",
            "count=7",
        ),
    ] {
        fs::write(&caller, source).unwrap();
        assert_eq!(
            NixSession::new()
                .unwrap()
                .evaluate_interop(&artifact)
                .unwrap()
                .value["result"],
            expected,
        );
        assert_eq!(artifact.source, original_source);
    }
}

#[test]
fn native_function_arguments_defaults_and_lexical_capture_stay_unchanged() {
    let factory = NixValue::function_attrs(["platform", "package"], |outer| {
        let input = args::from_value(outer);
        let inner = NixValue::function(|_| input.platform.count().into());
        (
            vec![("package", input.platform.count().into())],
            NixValue::list([input.package(), inner.call(NixValue::null())]),
        )
    });
    assert_eq!(
        evaluate(
            Nixpkgs::new()
                .function("functionArgs")
                .call(factory.clone())
        ),
        serde_json::json!({"platform": false, "package": true}),
    );
    assert_eq!(
        evaluate(factory.call(NixValue::record([("platform", platform(42))]))),
        serde_json::json!([42, 42]),
    );
}

#[test]
fn expected_scalar_types_do_not_claim_to_validate_actual_backend_types() {
    let input = args::from_value(NixValue::record([(
        "platform",
        NixValue::record([("isLinux", "not a boolean".into())]),
    )]));
    let condition: Expr<bool> = input.platform.is_linux();
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&artifact(NixValue::if_else(condition, true, false)))
        .unwrap_err();

    assert!(diagnostic.reason.contains("Boolean") || diagnostic.reason.contains("boolean"));
    assert!(!diagnostic.raw_nix.is_empty());
}
