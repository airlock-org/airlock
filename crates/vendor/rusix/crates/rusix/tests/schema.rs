#![cfg(feature = "evaluation")]

//! Structural declarations reuse existing traits and real NixOS evaluation.
use rusix::interop::raw::NixFunctionExt;
use rusix::{
    Expr, IntoConfig, IntoRusixValue,
    interop::{InputRef, Nixpkgs, raw::NixValue},
    nixos::{NixosModule, OptionDecl, OptionRef, OptionType},
};
use rusix::{NixSession, Provenance, nixos::compile_module};
use std::path::Path;

#[rusix::config]
mod schema {
    use super::OptionDecl;

    // A complete declaration tree, distinct from definitions supplied by consumers.
    #[rusix(root)]
    pub struct Root {
        // Structural nesting determines the first option-path segment.
        pub services: Services,
    }

    // Limits the fixture to one service namespace, without importing an external schema.
    pub struct Services {
        // Hosts the options used to exercise defaults, symbolic dependencies and literal keys.
        pub example: Options,
    }

    // Declaration leaves can describe different backend types while sharing the same Rust type.
    pub struct Options {
        // Supplies a source default that an ordinary Nix definition may override.
        pub source: OptionDecl,
        // Can hold a symbolic default or a deferred failure, selected independently of source.
        pub derived: OptionDecl,
        // The dot belongs to a single attribute name, not an extra nesting level.
        #[rusix(rename = "literal.dot")]
        pub unusual: OptionDecl,
    }
}

#[test]
fn declaration_paths_are_structural_and_separate_from_definitions() {
    let module = NixosModule::empty().declare(schema::Root {
        services: schema::Services {
            example: schema::Options {
                source: OptionDecl::new(OptionType::named("int")).default(42_i64),
                derived: OptionDecl::new(OptionType::named("str")).default("value"),
                unusual: OptionDecl::new(OptionType::named("bool")).default(true),
            },
        },
    });
    let artifact = compile_module(&module).unwrap();
    assert!(artifact.definitions.is_empty());
    assert_eq!(artifact.declarations.len(), 3);
    assert!(
        artifact
            .declarations
            .iter()
            .all(|decl| decl.origin.file == file!())
    );
    assert_eq!(
        module.modules[0].options.assignments[2].path_segments(),
        &["services", "example", "literal.dot"]
    );

    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(&artifact, &["services", "example"], false)
            .unwrap()
            .value,
        serde_json::json!({"source":42, "derived":"value", "literal.dot":true})
    );
}

#[test]
fn symbolic_schema_defaults_follow_ordinary_nix_definitions_without_relowering() {
    let module = NixosModule::empty()
        .declare(schema::Root {
            services: schema::Services {
                example: schema::Options {
                    source: OptionDecl::new(OptionType::named("int")).default(42_i64),
                    derived: OptionDecl::new(OptionType::named("int"))
                        .default(OptionRef::<i64>::new("services.example.source").into_expr()),
                    unusual: OptionDecl::new(OptionType::named("bool")).default(true),
                },
            },
        })
        .import_ref(
            InputRef::local(
                "schema-consumer",
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/schema-consumer.nix"),
            )
            .module("ordinary"),
        );
    let artifact = compile_module(&module).unwrap();
    let source = artifact.module.source.clone();
    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos(&artifact, &["services", "example", "derived"], false)
            .unwrap()
            .value,
        6432
    );
    assert_eq!(artifact.module.source, source);
}

#[test]
fn unused_schema_defaults_remain_lazy_and_failure_keeps_expression_origin() {
    let operation_line = line!() + 1;
    let bad = Expr::int(44).divide(Expr::int(0));
    let module = NixosModule::empty().declare(schema::Root {
        services: schema::Services {
            example: schema::Options {
                source: OptionDecl::new(OptionType::named("int")).default(42_i64),
                derived: OptionDecl::new(OptionType::named("int")).default(bad),
                unusual: OptionDecl::new(OptionType::named("bool")).default(true),
            },
        },
    });
    let artifact = compile_module(&module).unwrap();
    assert!(!artifact.module.source.contains("deepSeq"));
    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos(&artifact, &["services", "example", "source"], false)
            .unwrap()
            .value,
        42
    );

    let failure = session
        .evaluate_nixos(&artifact, &["services", "example", "derived"], false)
        .unwrap_err();
    assert_eq!(failure.primary.as_ref().unwrap().line, operation_line);
    assert_eq!(failure.provenance, Provenance::SourceMap);
    assert!(failure.reason.contains("division by zero"));
    assert!(!failure.raw_nix.is_empty());
}

#[test]
fn submodule_structural_errors_use_existing_ir_validation() {
    // Deliberately malformed structural input: only records can be flattened.
    #[derive(IntoRusixValue)]
    struct Invalid {
        // A scalar must fail before becoming a NixOS submodule declaration.
        #[rusix(flatten)]
        value: i64,
    }

    let error = OptionType::submodule(Invalid { value: 42 }, None).unwrap_err();
    assert!(error.message.contains("flatten requires a record"));

    // A single declaration contribution cannot declare the same path twice.
    #[derive(IntoConfig)]
    struct Duplicate {
        // Establishes the path that the renamed field below also tries to declare.
        port: OptionDecl,
        // Exercises duplicate-path validation after explicit naming is applied.
        #[rusix(rename = "port")]
        other: OptionDecl,
    }

    let module = NixosModule::empty().declare(Duplicate {
        port: OptionDecl::new(OptionType::named("port")),
        other: OptionDecl::new(OptionType::named("port")),
    });
    assert!(
        compile_module(&module)
            .unwrap_err()
            .reason
            .contains("duplicate")
    );
}

#[test]
fn generated_import_failures_preserve_nix_call_provenance() {
    let operation_line = line!() + 1;
    let invalid = Nixpkgs::new().function("head").call(NixValue::list([]));
    let module = NixosModule::empty().import_value(invalid);
    let failure = NixSession::new()
        .unwrap()
        .evaluate_nixos(&compile_module(&module).unwrap(), &["assertions"], false)
        .unwrap_err();
    assert_eq!(failure.primary.as_ref().unwrap().line, operation_line);
    assert!(failure.reason.contains("empty"));
    assert!(!failure.raw_nix.is_empty());
}

#[test]
fn invalid_schema_defaults_are_attributed_to_declarations_not_foreign_consumers() {
    let declaration_line = line!() + 1;
    let module = NixosModule::empty().declare(schema::Root {
        services: schema::Services {
            example: schema::Options {
                source: OptionDecl::new(OptionType::named("int")).default("wrong"),
                derived: OptionDecl::new(OptionType::named("int")).default(42_i64),
                unusual: OptionDecl::new(OptionType::named("bool")).default(true),
            },
        },
    });
    let artifact = compile_module(&module).unwrap();
    let failure = NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, &["services", "example", "source"], false)
        .unwrap_err();
    assert_eq!(failure.kind, rusix::DiagnosticKind::NixosType);
    assert_eq!(failure.primary.as_ref().unwrap().line, declaration_line);
    assert_eq!(failure.provenance, Provenance::ModuleDefinition);
}
