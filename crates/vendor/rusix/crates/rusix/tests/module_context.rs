#![cfg(feature = "evaluation")]

//! Exercise caller-owned module arguments, intact configuration values and source diagnostics.

use rusix::{
    Config, DiagnosticKind, Expr, Generated, NixSession, Provenance,
    interop::{NixPath, raw::NixValue},
    nix_record,
    nixos::{ContextualModule, ModuleBody, NixosArtifact, NixosModule, compile_module},
};

/// Runs native module evaluation with a narrow schema and caller-supplied dependencies.
fn driver(selection: &str) -> Generated {
    Generated {
        source: format!(
            r#"
                let
                  lib = import ./nixpkgs/lib;
                  evaluated = lib.evalModules {{
                    specialArgs.projectSource = ./.;
                    modules = [ ./module.nix ({{ lib, ... }}: {{
                      options.answer = lib.mkOption {{ type = lib.types.str; }};
                      options.raw = lib.mkOption {{
                        type = lib.types.raw;
                        default = {{}};
                      }};
                      options.items = lib.mkOption {{
                        type = lib.types.listOf lib.types.str;
                        default = [];
                      }};
                      config._module.args = {{
                        pkgs.label = "caller-package-set";
                        extra = "late-argument";
                      }};
                    }}) ];
                  }};
                in evaluated.config.{selection}
            "#,
        ),
        ..Generated::default()
    }
}

/// Writes an imported module whose location depends on a special argument.
fn imported_module(session: &NixSession) {
    std::fs::write(
        session.root().join("imported module.nix"),
        r#"{ lib, ... }: {
          options.imported = lib.mkOption { type = lib.types.str; default = "imported"; };
        }"#,
    )
    .unwrap();
}

#[test]
fn arguments_keep_caller_packages_final_config_and_early_import_paths() {
    let constructor = NixValue::function(|prefix| {
        ContextualModule::with_arguments(["projectSource", "extra", "unused"], |context| {
            let source: NixPath = context.argument("projectSource");
            let extra: NixValue = context.argument("extra");
            let config = nix_record! {
                "answer": NixValue::concat_text([
                    prefix, ":".into(),
                    context.pkgs.select("label"), ":".into(),
                    context.config.select("imported"), ":".into(), extra,
                ]),
            };

            ModuleBody::new(config)
                .import_value(source.join_relative("imported module.nix").unwrap())
        })
        .into_expression()
    });
    let module = NixosModule::empty().import_value(constructor.call("captured"));
    let artifact = compile_module(&module).unwrap();
    let session = NixSession::new().unwrap();
    imported_module(&session);

    assert_eq!(
        session
            .evaluate_nixos_with_driver(&artifact, &driver("answer"))
            .unwrap()
            .value,
        "captured:caller-package-set:imported:late-argument",
    );
    assert!(!artifact.module.source.contains("deepSeq"));
}

#[test]
fn intact_records_priorities_ordering_and_keys_keep_native_module_semantics() {
    let module = NixosModule::with_context(|context| {
        let raw = nix_record! { "a.b": nix_record! { "_type": "ordinary-data", "value": 7_i64 } };
        let first = ModuleBody::new(nix_record! { "items": NixValue::list(["once".into()]) })
            .key("shared-component");
        let config = nix_record! {
            "answer": context.lib.clone().select("mkForce").call("forced"),
            "raw": raw,
            "items": context.lib.select("mkBefore").call(NixValue::list(["first".into()])),
        };

        ModuleBody::new(config).module(first.clone()).module(first)
    })
    .module(NixosModule::new(
        Config::new().set_dynamic("answer", "ordinary"),
    ));
    let artifact = compile_module(&module).unwrap();
    let session = NixSession::new().unwrap();

    assert_eq!(
        session
            .evaluate_nixos_with_driver(&artifact, &driver("answer"))
            .unwrap()
            .value,
        "forced",
    );
    assert_eq!(
        session
            .evaluate_nixos_with_driver(&artifact, &driver("items"))
            .unwrap()
            .value,
        serde_json::json!(["first", "once"]),
    );
    assert_eq!(
        session
            .evaluate_nixos_with_driver(&artifact, &driver("raw"))
            .unwrap()
            .value,
        serde_json::json!({"a.b": {"_type": "ordinary-data", "value": 7}}),
    );
}

#[test]
fn conflicts_keep_both_contribution_origins_after_artifact_serialization() {
    let mut first_line = 0;
    let mut second_line = 0;
    let module = NixosModule::with_context(|_| {
        first_line = line!() + 1;
        let first = nix_record! { "answer": "first" };
        second_line = line!() + 1;
        let second = nix_record! { "answer": "second" };

        ModuleBody::empty()
            .module(ModuleBody::new(first))
            .module(ModuleBody::new(second))
    });
    let artifact = compile_module(&module).unwrap();
    let artifact: NixosArtifact =
        serde_json::from_slice(&serde_json::to_vec(&artifact).unwrap()).unwrap();
    let session = NixSession::new().unwrap();
    let error = session
        .evaluate_nixos_with_driver(&artifact, &driver("answer"))
        .unwrap_err();

    assert_eq!(error.kind, DiagnosticKind::NixosMerge, "{}", error.raw_nix);
    assert_eq!(error.option_path.as_deref(), Some("answer"));
    assert_eq!(error.provenance, Provenance::ModuleDefinition);
    let lines: std::collections::BTreeSet<_> = error.related.iter().map(|o| o.line).collect();
    assert_eq!(lines, [first_line, second_line].into());
    assert!(error.related.iter().all(|o| o.file == file!()));
    assert!(!error.raw_nix.is_empty());
}

#[test]
fn wrong_types_keep_option_path_and_contribution_location() {
    let mut location = 0;
    let module = NixosModule::with_context(|_| {
        location = line!() + 1;
        let value = nix_record! { "answer": 42_i64 };
        ModuleBody::new(value)
    });
    let error = NixSession::new()
        .unwrap()
        .evaluate_nixos_with_driver(&compile_module(&module).unwrap(), &driver("answer"))
        .unwrap_err();

    assert_eq!(error.kind, DiagnosticKind::NixosType);
    assert_eq!(error.option_path.as_deref(), Some("answer"));
    assert_eq!(error.primary.as_ref().unwrap().line, location);
}

#[test]
fn ignored_failures_stay_lazy_and_expression_errors_keep_their_own_location() {
    let mut location = 0;
    let module = NixosModule::with_context(|_| {
        location = line!() + 1;
        let bad = Expr::int(1).divide(Expr::int(0));
        ModuleBody::new(nix_record! { "answer": "good", "raw": bad })
    });
    let artifact = compile_module(&module).unwrap();
    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos_with_driver(&artifact, &driver("answer"))
            .unwrap()
            .value,
        "good"
    );

    let error = session
        .evaluate_nixos_with_driver(&artifact, &driver("raw"))
        .unwrap_err();
    assert_eq!(error.reason, "division by zero");
    assert_eq!(error.primary.as_ref().unwrap().line, location);
}

#[test]
fn invalid_names_and_escaped_context_references_fail_before_evaluation() {
    for name in ["pkgs", "not.a.name", "if"] {
        let module = NixosModule::with_arguments([name], |_| ModuleBody::empty());
        assert_eq!(
            compile_module(&module).unwrap_err().kind,
            DiagnosticKind::Validation
        );
    }

    let mut escaped = None;
    let _ = NixosModule::with_context(|context| {
        escaped = Some(context.config);
        ModuleBody::empty()
    });
    let module = NixosModule::new(Config::new().set_dynamic("answer", escaped.unwrap()));
    assert!(
        compile_module(&module)
            .unwrap_err()
            .reason
            .contains("escaped")
    );
}

#[test]
fn errors_in_external_imports_keep_their_nix_definition_location() {
    let module = NixosModule::with_arguments(["projectSource"], |context| {
        let source: NixPath = context.argument("projectSource");
        ModuleBody::empty().import_value(source.join_relative("bad-import.nix").unwrap())
    });
    let session = NixSession::new().unwrap();
    std::fs::write(session.root().join("bad-import.nix"), "{ answer = 42; }").unwrap();

    let error = session
        .evaluate_nixos_with_driver(&compile_module(&module).unwrap(), &driver("answer"))
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixosType);
    assert!(error.primary.is_none());
    assert!(error.origins.iter().any(|origin| {
        origin
            .nix_file
            .as_deref()
            .is_some_and(|file| file.ends_with("bad-import.nix"))
    }));
}
