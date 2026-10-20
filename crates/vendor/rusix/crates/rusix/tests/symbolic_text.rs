#![cfg(feature = "evaluation")]

//! Interpolation is authoring sugar: native Nix coercion, contexts and laziness.
use rusix::interop::raw::{AsNixValue, NixFunctionExt};
use rusix::{
    Config, Expr, IntoConfig,
    interop::{InputRef, Nixpkgs, raw::NixValue},
    nix_text,
    nixos::{DefinitionPriority, NixosModule, OptionRef},
};
use rusix::{Generated, NixSession, Provenance, compile, nixos::compile_module};
use std::{cell::Cell, fs, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[derive(IntoConfig)]
struct ResultContribution {
    result: NixValue,
}

fn generated(result: NixValue) -> Generated {
    compile(ResultContribution { result }.into_config()).unwrap()
}

#[test]
fn literals_braces_and_multiline_whitespace_are_preserved_verbatim() {
    let template = nix_text!(
        r#"  {{shell}} ${{literal}} "quote" \
    {word} {{nested: {{}}}}
"#,
        word = "café",
    );
    let result = NixValue::record(
        [
            ("template", template),
            ("empty", nix_text!("")),
            ("braces", nix_text!("{{}}")),
            ("fragments", nix_text!("literal {", "}", " remains")),
            ("emptyFragments", nix_text!()),
        ]
        .map(|(key, text)| (key, NixValue::from(text))),
    );
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_interop(&generated(result))
            .unwrap()
            .value["result"],
        serde_json::json!({
            "template": "  {shell} ${literal} \"quote\" \\\n    café {nested: {}}\n",
            "empty": "", "braces": "{}", "fragments": "literal {} remains",
            "emptyFragments": "",
        })
    );
}

#[test]
fn indented_blocks_preserve_text_and_interpolate_without_reindenting_values() {
    let text = nix_text!(
        r#"
            if {enabled}; then
              {{shell}} {word}:{word}

              {multiline}
            fi
        "#,
        enabled = true,
        word = "café",
        multiline = "first\n  second",
    );
    let result = NixValue::record(
        [
            ("block", text),
            ("noNewline", nix_text!("\n    end")),
            ("newline", nix_text!("\n    end\n    ")),
            ("empty", nix_text!("\n    ")),
            ("escapedBraces", nix_text!("\n    ${{PATH}}\n    ")),
            ("tabs", nix_text!("\n\tfirst\n\t\tnested\n\t")),
            ("mixed", nix_text!("\n\tfirst\n    second\n    ")),
            ("shortIndent", nix_text!("\n    first\n  second\n    ")),
        ]
        .map(|(key, text)| (key, NixValue::from(text))),
    );

    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated(result))
        .unwrap()
        .value;
    assert_eq!(
        value["result"],
        serde_json::json!({
            "block": "if 1; then\n  {shell} café:café\n\n  first\n  second\nfi\n",
            "noNewline": "end",
            "newline": "end\n",
            "empty": "",
            "escapedBraces": "${PATH}\n",
            "tabs": "first\n\tnested\n",
            "mixed": "\tfirst\n    second\n",
            "shortIndent": "  first\nsecond\n",
        }),
    );
}

#[test]
fn repeated_named_arguments_are_constructed_once_and_use_nix_coercion() {
    let constructions = Cell::new(0);
    let text = nix_text!(
        "{word}:{count}:{word}:{enabled}:{disabled}:{expr}",
        word = {
            constructions.set(constructions.get() + 1);
            String::from("value")
        },
        count = 42_u16,
        enabled = true,
        disabled = false,
        expr = Expr::int(44).divide(Expr::int(2)),
    );
    assert_eq!(constructions.get(), 1);
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_interop(&generated(text.into()))
            .unwrap()
            .value["result"],
        "value:42:value:1::22"
    );
    assert_eq!(constructions.get(), 1);
}

#[test]
fn package_and_derivation_interpolation_preserves_exact_string_contexts() {
    let pkgs = Nixpkgs::new();
    let package = pkgs.get("hello");
    let file = pkgs
        .pkgs_function("writeText")
        .apply(["example.conf".into(), "workers=4\n".into()]);
    let formatted = nix_text!(
        r#"
            {package}/bin/hello {file} {package}"#,
        package = package.clone(),
        file = file.clone(),
    );
    let fragments = NixValue::concat_text([
        package.as_value().to_text(),
        "/bin/hello ".into(),
        file.to_text(),
        " ".into(),
        package.as_value().to_text(),
    ]);
    let context = InputRef::local(
        "contexts",
        root().join("tests/fixtures/structured-interop.nix"),
    )
    .function("getContext");
    let result = NixValue::record([
        (
            "textEqual",
            NixValue::from(formatted.clone()).equals(fragments.clone()),
        ),
        ("newContext", context.call(formatted)),
        ("oldContext", context.call(fragments)),
    ]);
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated(result))
        .unwrap()
        .value;
    assert_eq!(value["result"]["textEqual"], true);
    let contexts = &value["result"]["newContext"];
    assert_eq!(contexts, &value["result"]["oldContext"]);
    assert_eq!(contexts.as_object().unwrap().len(), 2);
}

#[test]
fn same_artifact_interpolation_follows_ordinary_and_force_overrides() {
    let scratch = tempfile::tempdir().unwrap();
    let downstream = scratch.path().join("downstream.nix");
    fs::write(&downstream, "{ module = {}; }").unwrap();
    let text: NixValue = nix_text!(
        r#"
            postgres --port={port}"#,
        port = OptionRef::<i64>::new("services.example.port").into_expr(),
    )
    .into();

    let artifact = compile_module(
        &NixosModule::empty()
            .import_ref(
                InputRef::local("schema", root().join("tests/fixtures/symbolic-options.nix"))
                    .module("schema"),
            )
            .import_ref(InputRef::local("downstream", &downstream).module("module"))
            .module(
                NixosModule::new(Config::new().set_dynamic("services.example.port", 5432))
                    .priority(DefinitionPriority::Default),
            )
            .add(Config::new().set_dynamic("environment.command", text)),
    )
    .unwrap();
    let source = artifact.module.source.clone();
    assert!(source.contains("config.services.example.port"));
    assert!(!source.contains("deepSeq"));

    let session = NixSession::new().unwrap();
    for (module, expected) in [
        ("{}", "postgres --port=5432"),
        ("{ services.example.port = 6432; }", "postgres --port=6432"),
        (
            "{ lib, ... }: { services.example.port = lib.mkForce 7432; }",
            "postgres --port=7432",
        ),
    ] {
        fs::write(&downstream, format!("{{ module = {module}; }}")).unwrap();
        assert_eq!(
            session
                .evaluate_nixos(&artifact, &["environment", "command"], false)
                .unwrap()
                .value,
            expected,
        );
        assert_eq!(artifact.module.source, source);
    }
}

#[test]
fn unused_interpolated_failure_stays_lazy_and_selected_failure_keeps_child_origin() {
    let operation_line = line!() + 1;
    let failure = Expr::int(44).divide(Expr::int(0));
    let result = NixValue::record([
        ("good", 42.into()),
        (
            "bad",
            nix_text!(
                r#"
                    answer={failure}"#,
                failure = failure,
            )
            .into(),
        ),
    ]);

    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_interop(&generated(result.clone().select("good")))
            .unwrap()
            .value["result"],
        42,
    );

    let diagnostic = session
        .evaluate_interop(&generated(result.select("bad")))
        .unwrap_err();
    assert_eq!(diagnostic.reason, "division by zero");
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, operation_line);
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert_eq!(
        diagnostic.primary.as_ref().unwrap().purpose,
        "integer division"
    );
    assert!(!diagnostic.raw_nix.is_empty());
}

#[test]
fn unused_interpolation_does_not_force_a_throwing_final_option() {
    let reference_line = line!() + 1;
    let port = OptionRef::<i64>::new("services.example.port").into_expr();
    let fixture = InputRef::local("schema", root().join("tests/fixtures/symbolic-options.nix"));

    let artifact = compile_module(
        &NixosModule::empty()
            .import_ref(fixture.module("schema"))
            .import_ref(fixture.module("failing"))
            .add(
                Config::new().set_dynamic(
                    "environment.result",
                    NixValue::record([
                        ("safe", true.into()),
                        (
                            "command",
                            nix_text!(
                                r#"
                                port={port}"#,
                                port = port,
                            )
                            .into(),
                        ),
                    ]),
                ),
            ),
    )
    .unwrap();

    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos(&artifact, &["environment", "result", "safe"], false)
            .unwrap()
            .value,
        true,
    );

    let diagnostic = session
        .evaluate_nixos(&artifact, &["environment", "result", "command"], false)
        .unwrap_err();
    assert!(diagnostic.reason.contains("unused port was evaluated"));
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, reference_line);
}
