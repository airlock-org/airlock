#![cfg(feature = "evaluation")]

//! Structured opaque arguments use real pinned functions; no outputs are built.
use rusix::interop::raw::{AsNixValue, InputRefExt, NixFunctionExt};
use rusix::ir::ReferencedExpression;
use rusix::{
    Config, Expr, IntoConfig, IntoRusixValue,
    interop::{InputRef, Nixpkgs, raw::NixValue},
    ir::ValueKind,
    nixos::{NixosModule, OptionRef},
};
use rusix::{DiagnosticKind, Generated, NixSession, Provenance, compile, nixos::compile_module};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(IntoConfig)]
struct ResultContribution {
    result: NixValue,
}

#[track_caller]
fn generated(value: NixValue) -> Generated {
    compile(ResultContribution { result: value }.into_config()).unwrap()
}

fn evaluate(value: NixValue) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate(&generated(value))
        .unwrap()
        .value["result"]
        .clone()
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn save(name: &str, source: &str, value: &serde_json::Value) {
    let out = root().join("target/structured-interop").join(name);
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("generated.nix"), source).unwrap();
    fs::write(
        out.join("value.json"),
        serde_json::to_vec_pretty(value).unwrap(),
    )
    .unwrap();
}

#[test]
fn primitive_and_optional_values_roundtrip() {
    let value = NixValue::record([
        ("enabled", true.into()),
        ("count", 42_i64.into()),
        ("text", "quote\" newline\n ${literal}".into()),
        ("absent", NixValue::null()),
        ("none", None::<NixValue>.into()),
        ("some", Some(7_i32).into()),
    ]);
    assert_eq!(
        evaluate(value),
        serde_json::json!({
            "enabled": true, "count": 42, "text": "quote\" newline\n ${literal}",
            "absent": null, "none": null, "some": 7,
        })
    );
}

#[test]
fn finite_floats_keep_float_syntax_and_nix_numeric_semantics() {
    let floats = [
        1.0,
        -1.25,
        0.0,
        -0.0,
        1e-100,
        1e100,
        f64::MAX,
        f64::MIN_POSITIVE,
    ];

    let artifact = generated(NixValue::list(floats.map(NixValue::from)));
    assert!(artifact.source.contains("1.0"));
    assert!(artifact.source.contains("1.0e-100"));

    let value = NixSession::new()
        .unwrap()
        .evaluate(&artifact)
        .unwrap()
        .value;
    for (actual, expected) in value["result"].as_array().unwrap().iter().zip(floats) {
        assert_eq!(actual.as_f64().unwrap(), expected);
    }
    // A real lib function confirms that integral-looking floats stay floats,
    // rather than relying on JSON, which doesn't distinguish all numeric types.
    let check = Nixpkgs::new().function("isFloat").call(1.0);
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_interop(&generated(check))
            .unwrap()
            .value["result"],
        true
    );
}

#[test]
fn unsupported_float_literals_are_ir_errors_at_the_literal_origin() {
    for float in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::from_bits(1),
        -f64::from_bits(1),
    ] {
        let origin_line = line!() + 1;
        let value = NixValue::literal(float);
        let error = compile(ResultContribution { result: value }.into_config()).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Validation);
        assert!(error.reason.contains("finite"));
        assert_eq!(error.primary.as_ref().unwrap().line, origin_line);
        assert_eq!(error.primary.as_ref().unwrap().file, file!());
    }
}

#[test]
fn nested_lists_records_and_dynamic_keys_stay_atomic_in_derive() {
    let keys = [
        "",
        "a.b",
        "with space",
        "hyphen-key",
        "quote\"",
        "${throw \"injection\"}",
        "new\nline",
    ];
    let map: BTreeMap<String, NixValue> = keys
        .into_iter()
        .enumerate()
        .map(|(index, key)| (key.into(), NixValue::literal(index as i64)))
        .collect();
    let value = NixValue::record([
        ("map", map.into()),
        (
            "list",
            NixValue::list([
                true.into(),
                NixValue::record([("inner", NixValue::list(["x".into(), NixValue::null()]))]),
            ]),
        ),
    ]);
    let config = ResultContribution { result: value }.into_config();
    assert_eq!(
        config.assignments.len(),
        1,
        "opaque records are values, not option paths"
    );
    assert!(matches!(
        config.assignments[0].value.kind,
        ValueKind::OpaqueRecord(_)
    ));

    let artifact = compile(&config).unwrap();
    let actual = NixSession::new()
        .unwrap()
        .evaluate(&artifact)
        .unwrap()
        .value;
    for (index, key) in keys.into_iter().enumerate() {
        assert_eq!(actual["result"]["map"][key], index);
    }
    assert_eq!(
        actual["result"]["list"],
        serde_json::json!([true, {"inner": ["x", null]}])
    );
    assert!(artifact.source.contains("\\${throw"));
}

#[test]
fn duplicate_and_nul_record_keys_are_rejected_at_the_record_boundary() {
    for keys in [["same", "same"], ["good", "bad\0key"]] {
        let record_line = line!() + 1;
        let result = NixValue::record(keys.map(|key| (key, true.into())));
        let error = compile(ResultContribution { result }.into_config()).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Validation);
        assert_eq!(error.primary.as_ref().unwrap().line, record_line);
    }
}

#[test]
fn opaque_categories_and_expression_origins_survive_inside_records() {
    let pkgs = Nixpkgs::new();
    let package = pkgs.get("hello");
    let package_origin = package.reference().origin.clone();
    let input = InputRef::local(
        "example",
        root().join("tests/fixtures/nix-interop-input.nix"),
    );
    let expression_line = line!() + 1;
    let expression = Expr::int(44).divide(Expr::int(0));
    let mixed = NixValue::record([
        ("package", package.into()),
        ("module", pkgs.module("misc/label.nix").into()),
        ("function", pkgs.function("toUpper").into()),
        ("overlay", input.overlay("overlays.example").into()),
        ("external", input.value("packages.example")),
        ("expression", expression.into()),
    ]);

    let artifact = generated(mixed.clone());
    assert!(
        artifact
            .spans
            .iter()
            .any(|span| span.origin == package_origin)
    );

    let session = NixSession::new().unwrap();
    // Demand only projections that can be serialized, preserving native objects.
    for (selection, expected) in [
        ("package.pname", "hello"),
        ("external.pname", "rusix-external"),
    ] {
        assert_eq!(
            session
                .evaluate_interop(&generated(mixed.clone().select(selection)))
                .unwrap()
                .value["result"],
            expected
        );
    }
    assert_eq!(
        session
            .evaluate_interop(&generated(mixed.clone().select("function").call("rusix")))
            .unwrap()
            .value["result"],
        "RUSIX"
    );
    for (function, selection) in [
        ("types.deferredModule.check", "module"),
        ("isFunction", "overlay"),
    ] {
        let check = pkgs
            .function(function)
            .call(mixed.clone().select(selection));
        assert_eq!(
            session.evaluate_interop(&generated(check)).unwrap().value["result"],
            true
        );
    }
    let error = session
        .evaluate_interop(&generated(mixed.select("expression")))
        .unwrap_err();
    assert_eq!(error.reason, "division by zero");
    assert_eq!(error.provenance, Provenance::SourceMap);
    assert_eq!(error.primary.as_ref().unwrap().line, expression_line);
}

fn file_summary(file: NixValue) -> NixValue {
    // Nix JSON coerces records containing outPath to strings. Rename the
    // projection's key so the metadata itself remains a JSON record.
    NixValue::record([
        ("name", file.clone().select("name")),
        ("kind", file.clone().select("type")),
        ("text", file.clone().select("text")),
        ("drvPath", file.clone().select("drvPath")),
        ("output", file.select("outPath")),
    ])
}

#[test]
fn real_curried_write_text_produces_a_derivation_without_building() {
    let text = format!("workers = {}\n", 4);
    let file = Nixpkgs::new()
        .pkgs_function("writeText")
        .call("postgresql.conf")
        .call(NixValue::literal(text.clone()));
    let raw_file = generated(file.clone());
    let artifact = generated(file_summary(file));

    let session = NixSession::new().unwrap();

    let result = session.evaluate_interop(&artifact).unwrap().value["result"].clone();
    assert_eq!(result["name"], "postgresql.conf");
    assert_eq!(result["kind"], "derivation");
    assert_eq!(result["text"], text);
    assert!(result["drvPath"].as_str().unwrap().ends_with(".drv"));
    assert!(
        result["output"]
            .as_str()
            .unwrap()
            .ends_with("-postgresql.conf")
    );
    assert_eq!(
        session.evaluate_interop(&raw_file).unwrap().value["result"],
        result["output"]
    );
    // These are logical /nix/store paths in the disposable local store. Check
    // the physical isolated root, never the host path reported by Nix.
    assert!(
        session
            .root()
            .join("store")
            .join(result["drvPath"].as_str().unwrap().trim_start_matches('/'))
            .is_file()
    );
    assert!(
        !session
            .root()
            .join("store")
            .join(result["output"].as_str().unwrap().trim_start_matches('/'))
            .exists(),
        "evaluation must not realize the output"
    );
    save("write-text", &artifact.source, &result);
}

#[test]
fn real_run_command_accepts_structured_environment_and_remains_unbuilt() {
    let pkgs = Nixpkgs::new();
    let args = NixValue::record([
        ("buildInputs", NixValue::list([pkgs.get("hello").into()])),
        ("workers", 4.into()),
        ("enabled", true.into()),
    ]);
    // If this were ever built, it would fail; evaluation must not execute it.
    let command = "exit 97";
    let file = pkgs
        .pkgs_function("runCommand")
        .call("rusix-unbuilt")
        .call(args)
        .call(command);

    let summary = NixValue::record([
        ("output", file.clone().select("outPath")),
        ("drvPath", file.clone().select("drvPath")),
        ("command", file.clone().select("buildCommand")),
        ("workers", file.select("workers")),
    ]);

    let session = NixSession::new().unwrap();
    let artifact = generated(summary);

    let result = session.evaluate_interop(&artifact).unwrap().value["result"].clone();
    assert_eq!(result["command"], command);
    assert_eq!(result["workers"], 4);
    assert!(result["drvPath"].as_str().unwrap().ends_with(".drv"));
    assert!(
        !session
            .root()
            .join("store")
            .join(result["output"].as_str().unwrap().trim_start_matches('/'))
            .exists()
    );
    save("run-command", &artifact.source, &result);
}

#[test]
fn real_attrset_builder_accepts_literals_lists_nested_records_and_package_references() {
    let pkgs = Nixpkgs::new();
    let args = NixValue::record([
        ("name", "rusix-structured".into()),
        ("text", "structured content".into()),
        ("executable", false.into()),
        (
            "passthru",
            NixValue::record([
                ("dependency", pkgs.get("hello").into()),
                ("labels", NixValue::list(["a".into(), "b".into()])),
                (
                    "settings",
                    NixValue::record([("ratio", 0.125.into()), ("optional", NixValue::null())]),
                ),
            ]),
        ),
    ]);
    let file = pkgs.pkgs_function("writeTextFile").call(args);

    let summary = NixValue::record([
        ("file", file_summary(file.clone())),
        ("dependency", file.clone().select("dependency.pname")),
        ("labels", file.clone().select("labels")),
        ("settings", file.select("settings")),
    ]);

    let artifact = generated(summary);
    let result = NixSession::new()
        .unwrap()
        .evaluate_interop(&artifact)
        .unwrap()
        .value["result"]
        .clone();
    assert_eq!(result["file"]["kind"], "derivation");
    assert_eq!(result["dependency"], "hello");
    assert_eq!(result["labels"], serde_json::json!(["a", "b"]));
    assert_eq!(
        result["settings"],
        serde_json::json!({"ratio": 0.125, "optional": null})
    );
    save("attrset-builder", &artifact.source, &result);
}

#[test]
fn invalid_real_function_argument_maps_to_the_rust_call_and_retains_raw_trace() {
    let writer = Nixpkgs::new().pkgs_function("writeText").call("bad.conf");
    let argument = NixValue::record([("wrong", true.into())]);

    let call_line = line!() + 1;
    let result = writer.call(argument);

    let artifact = generated(result);
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&artifact)
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixEval);
    assert_eq!(error.provenance, Provenance::ErrorContext);
    assert_eq!(error.primary.as_ref().unwrap().line, call_line);
    assert_eq!(error.primary.as_ref().unwrap().file, file!());
    assert!(
        error.reason.contains("second argument should be a string"),
        "{}",
        error.raw_nix
    );
    assert!(error.raw_nix.contains("trivial-builders"));

    let out = root().join("target/structured-interop/invalid-call");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("diagnostic.txt"), error.render(&root())).unwrap();
    fs::write(out.join("nix.stderr"), &error.raw_nix).unwrap();
    fs::write(out.join("generated.nix"), &artifact.source).unwrap();
}

#[derive(IntoConfig)]
struct FileContribution {
    environment: Files,
}

#[derive(IntoRusixValue)]
struct Files {
    generated_file: NixValue,
    safe: bool,
}

fn symbolic_module(downstream: &Path, attrset_call: bool) -> NixosModule {
    let text = OptionRef::<String>::new("services.postgresql.dataDir")
        .into_expr()
        .with_prefix("data_directory=");
    let pkgs = Nixpkgs::new();
    let file = if attrset_call {
        pkgs.pkgs_function("writeTextFile").call(NixValue::record([
            ("name", "postgresql.conf".into()),
            ("text", text.into()),
            (
                "passthru",
                NixValue::record([(
                    "nested",
                    NixValue::record([
                        (
                            "directory",
                            OptionRef::<String>::new("services.postgresql.dataDir")
                                .into_expr()
                                .into(),
                        ),
                        ("packages", NixValue::list([pkgs.get("hello").into()])),
                    ]),
                )]),
            ),
        ]))
    } else {
        pkgs.pkgs_function("writeText")
            .call("postgresql.conf")
            .call(text)
    };
    let summary = if attrset_call {
        NixValue::record([
            ("file", file_summary(file.clone())),
            ("directory", file.select("nested.directory")),
        ])
    } else {
        file_summary(file)
    };
    NixosModule::empty()
        .import_ref(
            InputRef::local(
                "schema",
                root().join("tests/fixtures/structured-interop.nix"),
            )
            .module("schema"),
        )
        .import_ref(InputRef::local("downstream", downstream).module("module"))
        .add(FileContribution {
            environment: Files {
                generated_file: summary,
                safe: true,
            },
        })
}

#[test]
fn symbolic_write_text_and_nested_attrset_calls_follow_nix_overrides_without_relowering() {
    for attrset in [false, true] {
        let scratch = tempfile::tempdir().unwrap();
        let downstream = scratch.path().join("downstream.nix");
        fs::write(&downstream, "{ module = {}; }\n").unwrap();
        let artifact = compile_module(&symbolic_module(&downstream, attrset)).unwrap();
        let original = artifact.module.source.clone();
        assert!(original.contains("config.services.postgresql.dataDir"));
        assert!(!original.contains("deepSeq"));
        let session = NixSession::new().unwrap();
        let selection = &["environment", "generatedFile"];
        let base = session
            .evaluate_nixos_interop(&artifact, selection, false)
            .unwrap()
            .value;
        let base_file = if attrset { &base["file"] } else { &base };
        assert_eq!(base_file["text"], "data_directory=/var/lib/postgresql");
        fs::write(&downstream, "{ module = { lib, ... }: { services.postgresql.dataDir = lib.mkForce \"/srv/postgresql\"; }; }\n").unwrap();
        let changed = session
            .evaluate_nixos_interop(&artifact, selection, false)
            .unwrap()
            .value;
        let changed_file = if attrset { &changed["file"] } else { &changed };
        assert_eq!(changed_file["text"], "data_directory=/srv/postgresql");
        assert_ne!(base_file["drvPath"], changed_file["drvPath"]);
        if attrset {
            assert_eq!(changed["directory"], "/srv/postgresql");
        }
        assert_eq!(artifact.module.source, original);
        save(
            if attrset {
                "symbolic-record"
            } else {
                "postgresql-smoke"
            },
            &original,
            &changed,
        );
    }
}

#[test]
fn unused_structured_symbolic_calls_stay_lazy_and_reference_failures_keep_origins() {
    let scratch = tempfile::tempdir().unwrap();
    let downstream = scratch.path().join("downstream.nix");
    fs::write(
        &downstream,
        "{ module = { services.postgresql.dataDir = throw \"unused directory was forced\"; }; }\n",
    )
    .unwrap();

    let artifact = compile_module(&symbolic_module(&downstream, true)).unwrap();

    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos_interop(&artifact, &["environment", "safe"], false)
            .unwrap()
            .value,
        true
    );
    let error = session
        .evaluate_nixos_interop(&artifact, &["environment", "generatedFile"], false)
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixEval);
    assert_eq!(error.provenance, Provenance::ErrorContext);
    assert_eq!(
        error.primary.as_ref().unwrap().purpose,
        "NixOS option reference services.postgresql.dataDir"
    );
    assert!(error.reason.contains("unused directory was forced"));
}

#[test]
fn scoped_symbols_inside_structured_arguments_cannot_escape_nixos() {
    let value = Nixpkgs::new()
        .pkgs_function("writeTextFile")
        .call(NixValue::record([
            ("name", "scoped.conf".into()),
            (
                "text",
                OptionRef::<String>::new("services.postgresql.dataDir")
                    .into_expr()
                    .into(),
            ),
        ]));
    let error = compile(ResultContribution { result: value }.into_config()).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);
    assert!(error.reason.contains("NixosModule"));
}

#[test]
fn lazy_record_merging_keeps_unselected_fallible_values_unforced() {
    let left = NixValue::record([
        ("good", 42.into()),
        ("bad", Expr::int(44).divide(Expr::int(0)).into()),
    ]);
    let result = Nixpkgs::new()
        .function("recursiveUpdate")
        .call(left)
        .call(NixValue::record([("extra", true.into())]));

    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_interop(&generated(result.clone().select("good")))
            .unwrap()
            .value["result"],
        42
    );
    let error = session
        .evaluate_interop(&generated(result.select("bad")))
        .unwrap_err();
    assert_eq!(error.reason, "division by zero");
    assert_eq!(error.primary.as_ref().unwrap().purpose, "integer division");
}

#[test]
fn opaque_callbacks_capture_lexical_parameters_and_render_deterministically() {
    fn callback() -> NixValue {
        NixValue::function(|outer| {
            NixValue::function(|inner| {
                Nixpkgs::new()
                    .function("concatStringsSep")
                    .call(":")
                    .call(NixValue::list([outer, inner]))
            })
        })
    }

    let artifact = || generated(callback().call("outside").call("inside"));
    let first = artifact();
    let second = artifact();
    assert_eq!(first.source, second.source);
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_interop(&first)
            .unwrap()
            .value["result"],
        "outside:inside"
    );
}

#[test]
fn callback_parameters_cannot_escape_their_lexical_scope() {
    let mut escaped = None;
    let _callback = NixValue::function(|parameter| {
        escaped = Some(parameter);
        true.into()
    });
    let error = compile(
        ResultContribution {
            result: escaped.unwrap(),
        }
        .into_config(),
    )
    .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);
    assert!(error.reason.contains("escaped its function scope"));
}

#[test]
fn opaque_choices_leave_unused_branches_lazy() {
    let failure: NixValue = Expr::int(44).divide(Expr::int(0)).into();
    let value = NixValue::if_else(NixValue::from(42).equals(42), "selected", failure);
    assert_eq!(evaluate(value), "selected");
}

#[test]
fn opaque_callbacks_map_final_collection_options_and_preserve_origin() {
    let session = NixSession::new().unwrap();
    let callback_line = line!() + 1;
    let callback = NixValue::function(|item| item.select("missing"));
    let value = Nixpkgs::new()
        .function("map")
        .call(callback)
        .call(NixValue::list([NixValue::record([(
            "present",
            true.into(),
        )])]));
    let error = session.evaluate_interop(&generated(value)).unwrap_err();
    assert!(error.reason.contains("missing"));
    assert!(error.primary.as_ref().unwrap().line >= callback_line);
    assert_eq!(error.primary.as_ref().unwrap().file, file!());
    assert!(!error.raw_nix.is_empty());
}

#[test]
fn module_package_handles_are_scoped_and_follow_module_package_arguments() {
    let pkgs = Nixpkgs::from_module();
    let value = pkgs.get("hello").as_value().select("pname");
    let error = compile(
        ResultContribution {
            result: value.clone(),
        }
        .into_config(),
    )
    .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);

    let artifact =
        compile_module(&NixosModule::empty().add(ResultContribution { result: value })).unwrap();
    assert!(artifact.module.source.contains("pkgs"));

    // This selection belongs to the existing minimal module harness's namespaces.
    let artifact = compile_module(&NixosModule::empty().add(Config::new().set_dynamic(
        "environment.result",
        pkgs.get("hello").as_value().select("pname"),
    )))
    .unwrap();
    assert_eq!(session_value(&artifact), "hello");
}

fn session_value(artifact: &rusix::nixos::NixosArtifact) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate_nixos_interop(artifact, &["environment", "result"], false)
        .unwrap()
        .value
}

#[test]
fn curried_apply_preserves_argument_order_and_the_authoring_call_origin() {
    let pkgs = Nixpkgs::new();

    let artifact = generated(
        pkgs.pkgs_function("writeText")
            .apply([
                "curried.conf".into(),
                rusix::nix_text!("workers={count}", count = 4).into(),
            ])
            .select("text"),
    );

    let session = NixSession::new().unwrap();
    assert_eq!(
        session.evaluate_interop(&artifact).unwrap().value["result"],
        "workers=4"
    );

    let writer = pkgs.pkgs_function("writeText");
    let arguments = ["bad.conf".into(), rusix::nix_record! { "wrong": true }];

    let call_line = line!() + 1;
    let invalid = writer.apply(arguments);

    let diagnostic = session.evaluate_interop(&generated(invalid)).unwrap_err();
    assert_eq!(diagnostic.provenance, Provenance::ErrorContext);
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, call_line);
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert!(
        diagnostic
            .reason
            .contains("second argument should be a string")
    );
    assert!(!diagnostic.raw_nix.is_empty());
}

#[test]
fn mixed_record_and_text_macros_preserve_opaque_values_and_literal_keys() {
    let pkgs = Nixpkgs::new();
    let dynamic_key = "literal.dot ${not interpolation}\"";

    let value = rusix::nix_record! {
        (dynamic_key): rusix::nix_record! {},
        "package": pkgs.get("hello"),
        "text": rusix::nix_text!("hello ", "world"),
        "emptyText": rusix::nix_text!(),
        "list": NixValue::list([1.into(), 2.into()]),
    };

    let summary = rusix::nix_record! {
        "key": pkgs.function("getAttr").apply([dynamic_key.into(), value.clone()]),
        "package": value.clone().select("package.pname"),
        "text": value.clone().select("text"),
        "emptyText": value.clone().select("emptyText"),
        "list": value.select("list"),
    };
    let result = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated(summary))
        .unwrap()
        .value;
    assert_eq!(
        result["result"],
        serde_json::json!({
            "key": {}, "package": "hello", "text": "hello world", "emptyText": "", "list": [1, 2]
        })
    );
}

#[test]
fn joined_text_retains_store_dependencies_and_deferred_list_operations() {
    let pkgs = Nixpkgs::new();
    let dependency = pkgs.get("hello").as_value().to_text();
    let text = rusix::nix_text!(
        "prefix ",
        dependency.clone().into_expr::<String>(),
        " suffix"
    );

    let contexts_equal = InputRef::local(
        "context",
        root().join("tests/fixtures/structured-interop.nix"),
    )
    .function("getContext")
    .call(text)
    .equals(
        InputRef::local(
            "context",
            root().join("tests/fixtures/structured-interop.nix"),
        )
        .function("getContext")
        .call(dependency),
    );

    let list = Nixpkgs::new().function("map").apply([
        NixValue::function(|item| item.to_text()),
        NixValue::list([1.into(), 2.into()]),
    ]);

    let summary = rusix::nix_record! {
        "contextsEqual": contexts_equal,
        "joined": NixValue::join_text("/", list),
    };

    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated(summary))
        .unwrap()
        .value;
    assert_eq!(
        value["result"],
        serde_json::json!({"contextsEqual": true, "joined": "1/2"})
    );
}

#[test]
fn mixed_macros_retain_symbolic_references_and_unused_fields_stay_lazy() {
    let value = rusix::nix_record! {
        "command": rusix::nix_text!("port=", OptionRef::<i64>::new("services.example.port").into_expr().to_text()),
        "unused": Expr::int(1).divide(Expr::int(0)),
    };
    let schema = InputRef::local(
        "options",
        root().join("tests/fixtures/symbolic-options.nix"),
    );

    let artifact = compile_module(
        &NixosModule::empty()
            .import_ref(schema.module("schema"))
            .add(
                Config::new()
                    .set_dynamic("services.example.port", 5432)
                    .set_dynamic("environment.result", value),
            ),
    )
    .unwrap();

    let session = NixSession::new().unwrap();
    assert!(
        artifact
            .module
            .source
            .contains("config.services.example.port")
    );
    assert_eq!(
        session
            .evaluate_nixos_interop(&artifact, &["environment", "result", "command"], false)
            .unwrap()
            .value,
        "port=5432"
    );
}
