#![cfg(feature = "evaluation")]

use rusix::{
    Config, Expr,
    nixos::{DefinitionPriority, NixosModule},
};
use rusix::{
    Diagnostic, DiagnosticKind, NixSession, OriginRole, Provenance,
    nixos::{NixosArtifact, compile_module},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[path = "../src/bin/cli/merge_fixtures.rs"]
mod fixtures;

#[allow(dead_code)] // Shared fixture helpers include cases unused in this test target.
#[path = "../src/bin/cli/support.rs"]
mod fixture_support;

use fixture_support::ExistingModule;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn normalized(mut d: Diagnostic) -> Diagnostic {
    for source in &mut d.origins {
        if let Some(origin) = &mut source.origin {
            let old = origin.file.clone();
            origin.file = workspace()
                .join(&old)
                .canonicalize()
                .unwrap()
                .strip_prefix(workspace())
                .unwrap()
                .to_string_lossy()
                .into();
            d.reason = d.reason.replace(&old, &origin.file);
        }
    }

    if let Some(origin) = &mut d.primary {
        origin.file = workspace()
            .join(&origin.file)
            .canonicalize()
            .unwrap()
            .strip_prefix(workspace())
            .unwrap()
            .to_string_lossy()
            .into();
    }

    d
}

fn failure(name: &str) -> (NixosArtifact, Diagnostic) {
    let artifact = compile_module(&fixtures::module(name).unwrap()).unwrap();
    assert!(!artifact.module.source.contains("deepSeq"));
    let d = *NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, fixtures::selection(name), false)
        .unwrap_err();
    assert!(!d.raw_nix.is_empty());
    assert_eq!(
        normalized(d.clone()).render(&workspace()),
        fs::read_to_string(workspace().join(format!("tests/snapshots/{name}.txt"))).unwrap()
    );
    (artifact, d)
}

#[test]
fn two_individually_valid_modules_conflict_with_both_rust_origins() {
    for child in [fixtures::a(), fixtures::b()] {
        let module = NixosModule::new(Config::new())
            .import(ExistingModule::OpenSsh)
            .module(child);
        NixSession::new()
            .unwrap()
            .evaluate_nixos(
                &compile_module(&module).unwrap(),
                fixtures::selection("merge-two"),
                false,
            )
            .unwrap();
    }

    let (artifact, d) = failure("merge-two");
    assert_eq!(d.kind, DiagnosticKind::NixosMerge);
    assert_eq!(
        d.option_path.as_deref(),
        Some("services.openssh.authorizedKeysCommandUser")
    );
    assert_eq!(d.origins.len(), 2);

    for source in &d.origins {
        assert_eq!(source.role, OriginRole::ConflictingDefinition);
        assert_eq!(source.provenance, Provenance::ModuleDefinition);
        let origin = source.origin.as_ref().unwrap();
        assert!(artifact.definitions.iter().any(|b| &b.origin == origin));
        assert!(d.raw_nix.contains(&origin.id));
    }

    assert_ne!(d.origins[0].origin, d.origins[1].origin);
}

#[test]
fn two_lists_merge_and_both_definitions_contribute() {
    let artifact = compile_module(&fixtures::module("merge-ok").unwrap()).unwrap();
    assert_eq!(artifact.definitions.len(), 2);
    let value = NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, fixtures::selection("merge-ok"), false)
        .unwrap()
        .value;
    // Upstream module collection traverses imported definitions in this order.
    assert_eq!(value, serde_json::json!([2222, 22]));
}

#[test]
fn three_valid_definitions_preserve_exactly_the_conflicting_pair_reported_by_nix() {
    let (artifact, d) = failure("merge-three");
    assert_eq!(artifact.definitions.len(), 3);
    assert_eq!(d.kind, DiagnosticKind::NixosMerge);
    // Pinned mergeEqualOption stops at its first mismatch, C versus B. A isn't
    // reported, so do not invent a third causal source from the artifact table.
    assert_eq!(d.origins.len(), 2);
    assert_eq!(
        d.origins[0].origin.as_ref(),
        Some(&artifact.definitions[2].origin)
    );
    assert_eq!(
        d.origins[1].origin.as_ref(),
        Some(&artifact.definitions[1].origin)
    );
    assert!(!d.raw_nix.contains(&artifact.definitions[0].origin.id));
}

#[test]
fn three_invalid_definitions_retain_all_three_origins_from_nixos() {
    let (artifact, d) = failure("merge-three-type");
    assert_eq!(d.kind, DiagnosticKind::NixosType);
    assert_eq!(d.origins.len(), 3);

    for boundary in &artifact.definitions {
        assert!(
            d.origins
                .iter()
                .any(|o| o.origin.as_ref() == Some(&boundary.origin))
        );
        assert!(d.raw_nix.contains(&boundary.origin.id));
    }
    assert!(
        d.origins
            .iter()
            .all(|o| o.role == OriginRole::ContributingDefinition)
    );
}

#[test]
fn mixed_conflict_retains_rust_definition_and_upstream_import_boundary() {
    let (artifact, d) = failure("merge-mixed");
    assert_eq!(d.kind, DiagnosticKind::NixosMerge);
    assert_eq!(d.option_path.as_deref(), Some("system.nixos.label"));
    assert_eq!(d.origins.len(), 2);
    let external = d
        .origins
        .iter()
        .find(|o| o.role == OriginRole::ImportedBoundary)
        .unwrap();
    assert_eq!(external.origin.as_ref(), Some(&artifact.imports[0].origin));
    assert_eq!(
        external.nix_file.as_deref(),
        Some("nixos/modules/misc/label.nix")
    );
    assert_eq!(external.provenance, Provenance::ImportBoundary);
    let rust = d
        .origins
        .iter()
        .find(|o| o.role == OriginRole::ConflictingDefinition)
        .unwrap();
    assert_eq!(rust.origin.as_ref(), Some(&artifact.definitions[1].origin));
    assert_eq!(d.primary, rust.origin); // compatibility field, not the causal set
    assert!(!d.reason.contains("/tmp/"));
    assert!(d.raw_nix.contains("/tmp/rusix-"));
}

#[test]
fn priorities_are_resolved_by_nixos_and_discarded_values_stay_lazy() {
    let artifact = compile_module(&fixtures::module("merge-priority").unwrap()).unwrap();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(&artifact, fixtures::selection("merge-priority"), false)
            .unwrap()
            .value,
        "sshd"
    );
    let base = || NixosModule::new(Config::new()).import(ExistingModule::OpenSsh);
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(
                &compile_module(
                    &base()
                        .module(fixtures::a().priority(DefinitionPriority::Default))
                        .module(fixtures::b())
                )
                .unwrap(),
                fixtures::selection("merge-priority"),
                false
            )
            .unwrap()
            .value,
        "nobody"
    );
    // mkOverride 40 beats mkForce 50. The losing invalid expression is never
    // type checked or evaluated by Rusix to reconstruct provenance.
    let losing = NixosModule::new(Config::new().set_dynamic(
        "services.openssh.authorizedKeysCommandUser",
        Expr::int(44).divide(Expr::int(0)),
    ))
    .priority(DefinitionPriority::Default);
    let chosen = base()
        .module(losing)
        .module(fixtures::b().priority(DefinitionPriority::Force))
        .module(fixtures::a().priority(DefinitionPriority::Override(40)));
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(
                &compile_module(&chosen).unwrap(),
                fixtures::selection("merge-priority"),
                false
            )
            .unwrap()
            .value,
        "root"
    );
}

#[test]
fn rendered_and_json_diagnostics_preserve_the_entire_causal_set() {
    let (_, d) = failure("merge-three-type");
    let rendered = d.render(&workspace());
    assert_eq!(rendered.matches("  --> ").count(), 3);
    assert_eq!(rendered.matches("   = contributing definition").count(), 3);
    assert!(
        rendered.contains("invalid-a")
            && rendered.contains("invalid-b")
            && rendered.contains("invalid-c")
    );
    let json = serde_json::to_value(&d).unwrap();
    assert_eq!(json["origins"].as_array().unwrap().len(), 3);
    assert_eq!(json["raw_nix"], d.raw_nix);
    let roundtrip: Diagnostic = serde_json::from_value(json).unwrap();
    assert_eq!(roundtrip.origins, d.origins);
    assert_eq!(roundtrip.render(&workspace()), rendered);
}

#[test]
fn unselected_conflicting_option_does_not_break_a_selected_valid_option() {
    let module = fixtures::module("merge-two")
        .unwrap()
        .module(fixtures::ports_a());

    let artifact = compile_module(&module).unwrap();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(&artifact, fixtures::selection("merge-ok"), false)
            .unwrap()
            .value,
        serde_json::json!([22])
    );
}

#[test]
fn several_failed_assertions_keep_all_message_origins() {
    let module = NixosModule::new(Config::new())
        .import(ExistingModule::OpenSsh)
        .assertion("a", Expr::boolean(false), "policy a")
        .assertion("b", Expr::boolean(false), "policy b");
    let d = NixSession::new()
        .unwrap()
        .evaluate_nixos(
            &compile_module(&module).unwrap(),
            &["services", "openssh", "enable"],
            true,
        )
        .unwrap_err();
    assert_eq!(d.kind, DiagnosticKind::NixosAssertion);
    assert_eq!(d.origins.len(), 2);
    for assertion in module.assertions {
        assert!(
            d.origins
                .iter()
                .any(|o| o.origin.as_ref() == Some(&assertion.origin))
        );
    }
    assert_eq!(d.option_path.as_deref(), Some("assertions"));
}
