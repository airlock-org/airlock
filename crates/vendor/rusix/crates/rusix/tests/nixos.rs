#![cfg(feature = "evaluation")]

use rusix::{
    Diagnostic, DiagnosticKind, Generated, NixSession, Provenance,
    nixos::{compile_module, evaluation_source},
};
use rusix::{IntoConfig, ir::ValueKind, nixos::NixosModule};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[path = "../src/bin/cli/nixos_fixtures.rs"]
mod fixtures;

#[allow(dead_code)] // Shared fixture helpers include cases unused in this test target.
#[path = "../src/bin/cli/support.rs"]
mod fixture_support;

use fixture_support::{ExistingModule, OpenSsh};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn snapshot(name: &str, diagnostic: &Diagnostic) {
    let mut diagnostic = diagnostic.clone();
    let origin = diagnostic.primary.as_mut().unwrap();
    let original_file = origin.file.clone();
    origin.file = workspace()
        .join(&original_file)
        .canonicalize()
        .unwrap()
        .strip_prefix(workspace())
        .unwrap()
        .to_string_lossy()
        .into();
    diagnostic.reason = diagnostic.reason.replace(&original_file, &origin.file);
    assert_eq!(
        diagnostic.summary(),
        fs::read_to_string(workspace().join(format!("tests/snapshots/nixos-{name}.txt"))).unwrap()
    );
}

fn check_failure(name: &str, kind: DiagnosticKind, provenance: Provenance) -> Diagnostic {
    let module = fixtures::module(name).unwrap();
    let expected = match name {
        "type" | "unknown" => &module.config.assignments[1].origin,
        "assertion" => &module.assertions[0].origin,
        "external" => &module.imports[1].origin,
        _ => unreachable!(),
    };

    let artifact = compile_module(&module).unwrap();
    assert!(!artifact.module.source.contains("deepSeq"));

    let diagnostic = *NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, fixtures::selection(name), name == "assertion")
        .unwrap_err();
    assert_eq!(diagnostic.kind, kind, "{}", diagnostic.raw_nix);
    assert_eq!(diagnostic.primary.as_ref(), Some(expected));
    assert_eq!(diagnostic.provenance, provenance);
    assert!(diagnostic.raw_nix.contains("@nix "));
    assert!(diagnostic.render(&workspace()).contains("   |"));
    snapshot(name, &diagnostic);
    diagnostic
}

#[test]
fn typed_openssh_is_evaluated_by_real_module_system() {
    let module = fixtures::module("good").unwrap();

    let artifact = compile_module(&module).unwrap();
    assert_eq!(
        compile_module(&module).unwrap().module.source,
        artifact.module.source
    );
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(&artifact, fixtures::selection("good"), false)
            .unwrap()
            .value,
        serde_json::json!([22])
    );
    // An enabled scalar can also be demanded without evaluating package/service definitions.
    let enabled =
        NixosModule::new(OpenSsh::new().enable(true).into_config()).import(ExistingModule::OpenSsh);
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(
                &compile_module(&enabled).unwrap(),
                &["services", "openssh", "enable"],
                false
            )
            .unwrap()
            .value,
        true
    );
    assert!(
        module.config.assignments[0]
            .origin
            .file
            .ends_with("nixos_fixtures.rs")
    );
}

#[test]
fn generic_pinned_import_paths_are_validated_and_rendered_as_data() {
    for path in ["", "/absolute/module.nix", "../module.nix", "module\0.nix"] {
        let module = NixosModule::empty().import(path);
        let diagnostic = compile_module(&module).unwrap_err();
        assert_eq!(diagnostic.kind, DiagnosticKind::Validation);
        assert_eq!(diagnostic.primary.as_ref(), Some(&module.imports[0].origin));
    }
    let path = "nixos/modules/${not a variable}/module with spaces.nix";

    let artifact = compile_module(&NixosModule::empty().import(path)).unwrap();
    assert!(artifact.module.source.contains("\\${not a variable}"));

    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, &["assertions"], false)
        .unwrap_err();
    // A missing file is an evaluator error, never invalid generated syntax.
    assert_ne!(diagnostic.kind, DiagnosticKind::Compiler);
    assert!(diagnostic.reason.contains("${not a variable}"));
}

#[test]
fn module_type_error_keeps_definition_origin_and_option() {
    let d = check_failure(
        "type",
        DiagnosticKind::NixosType,
        Provenance::ModuleDefinition,
    );
    assert_eq!(d.option_path.as_deref(), Some("services.openssh.ports"));
    assert!(d.reason.contains("16 bit unsigned integer"));
    assert!(d.raw_nix.contains(&d.primary.as_ref().unwrap().id));
    // Definition metadata supplies this origin; no runtime origin frame is needed.
    for event in d
        .raw_nix
        .lines()
        .filter_map(|line| line.strip_prefix("@nix "))
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    {
        if let Some(frames) = event["trace"].as_array() {
            assert!(
                frames
                    .iter()
                    .all(|frame| { !frame["raw_msg"].as_str().unwrap_or("").starts_with("rn-") })
            );
        }
    }
}

#[test]
fn unknown_option_keeps_introducing_operation_and_option_path() {
    let d = check_failure(
        "unknown",
        DiagnosticKind::NixosModule,
        Provenance::ModuleDefinition,
    );
    assert_eq!(
        d.option_path.as_deref(),
        Some("services.openssh.rusixMissing")
    );
    assert!(d.reason.contains("does not exist"));
}

#[test]
fn assertion_message_maps_to_rust_assertion_construct() {
    let d = check_failure(
        "assertion",
        DiagnosticKind::NixosAssertion,
        Provenance::AssertionMessage,
    );
    assert_eq!(d.option_path.as_deref(), Some("assertions.port-policy"));
    assert!(d.reason.contains("SSH port policy rejected"));
    assert!(
        !d.reason
            .contains(&format!("[{}]", d.primary.as_ref().unwrap().id))
    );
}

#[test]
fn imported_real_module_failure_maps_only_to_import_boundary() {
    let d = check_failure(
        "external",
        DiagnosticKind::ExternalNix,
        Provenance::ImportBoundary,
    );
    assert_eq!(d.option_path.as_deref(), Some("system.nixos.label"));
    assert_eq!(
        d.external_file.as_deref(),
        Some("nixos/modules/misc/label.nix")
    );
    assert_eq!(d.reason, "attribute 'version' missing");
}

#[test]
fn module_selection_is_lazy_and_nested_operation_origin_survives() {
    let module = fixtures::module("lazy").unwrap();
    let ValueKind::List(items) = &module.config.assignments[1].value.kind else {
        panic!()
    };

    let artifact = compile_module(&module).unwrap();

    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos(&artifact, fixtures::selection("lazy"), false)
            .unwrap()
            .value,
        false
    );
    let d = session
        .evaluate_nixos(&artifact, fixtures::selection("good"), false)
        .unwrap_err();
    assert_eq!(d.kind, DiagnosticKind::NixEval);
    assert_eq!(d.primary, Some(items[1].origin.clone()));
    assert_eq!(d.provenance, Provenance::SourceMap);
    assert_eq!(d.option_path.as_deref(), Some("services.openssh.ports"));
    assert_eq!(d.reason, "division by zero");
    // Checking successful assertions must not classify an unrelated selected
    // expression failure as an assertion failure.
    assert_eq!(
        session
            .evaluate_nixos(&artifact, fixtures::selection("good"), true)
            .unwrap_err()
            .kind,
        DiagnosticKind::NixEval
    );

    // Strip runtime frames from a captured real module error; source-map fallback survives.
    let mut event: serde_json::Value = d
        .raw_nix
        .lines()
        .filter_map(|l| l.strip_prefix("@nix "))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|e| e["level"] == 0 && e["raw_msg"].is_string())
        .unwrap();
    event["trace"]
        .as_array_mut()
        .unwrap()
        .retain(|f| !f["raw_msg"].as_str().unwrap_or("").starts_with("rn-"));

    let fallback = Diagnostic::from_nix(
        DiagnosticKind::NixEval,
        &format!("@nix {event}"),
        &artifact.module,
        &session.root().join("module.nix"),
    );
    assert_eq!(fallback.primary, Some(items[1].origin.clone()));
    assert_eq!(fallback.provenance, Provenance::SourceMap);
    assert!(
        fallback
            .related
            .iter()
            .any(|o| o.purpose == "set services.openssh.ports")
    );
}

#[test]
fn assertion_checking_is_explicit_and_does_not_force_unrelated_options() {
    let artifact = compile_module(&fixtures::module("assertion").unwrap()).unwrap();

    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos(&artifact, fixtures::selection("good"), false)
            .unwrap()
            .value,
        serde_json::json!([22])
    );
    assert_eq!(
        session
            .evaluate_nixos(&artifact, fixtures::selection("good"), true)
            .unwrap_err()
            .kind,
        DiagnosticKind::NixosAssertion
    );
}

#[test]
fn invalid_generated_module_is_compiler_failure_without_rust_blame() {
    let mut artifact = compile_module(&fixtures::module("good").unwrap()).unwrap();
    artifact.module = Generated {
        backend_metadata: None,
        source: "{ imports = [ ; ]; }".into(),
        spans: artifact.module.spans,
    };
    let d = NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, fixtures::selection("good"), false)
        .unwrap_err();
    assert_eq!(d.kind, DiagnosticKind::Compiler);
    assert!(d.primary.is_none());
    assert!(!d.raw_nix.is_empty());
}

#[test]
fn checked_nixpkgs_is_staged_only_inside_disposable_session() {
    let session = NixSession::new().unwrap();
    let root = session.root().to_owned();
    session
        .evaluate_nixos(
            &compile_module(&fixtures::module("good").unwrap()).unwrap(),
            fixtures::selection("good"),
            false,
        )
        .unwrap();
    assert!(root.join("nixpkgs/lib/modules.nix").exists());
    assert!(root.join("store/nix/var/nix/db/db.sqlite").exists());
    drop(session);
    assert!(!root.exists());
    let source = evaluation_source(&["${throw \"injection\"}", "--store /nix/store"], false);
    assert!(source.contains("\\${throw \\\"injection\\\"}"));
}

#[test]
fn assertion_conditions_reject_invalid_ir_before_lowering() {
    use rusix::{
        interop::raw::NixValue,
        ir::IntoNode,
        ir::Origin,
        nixos::{NixosModule, OptionRef},
    };

    let mut escaped = None;
    let _function = NixValue::function(|parameter| {
        escaped = Some(parameter.clone());
        parameter
    });
    let conditions = [
        ("escaped", escaped.unwrap().into_expr::<bool>(), "escaped"),
        ("path", OptionRef::<bool>::new("").into_expr(), "nonempty"),
        ("nul", NixValue::from("\0").into_expr::<bool>(), "NUL"),
    ];

    for (name, condition, reason) in conditions {
        let expected = condition
            .clone()
            .into_node(Origin::caller("test condition"))
            .origin;
        let module = NixosModule::empty().assertion(name, condition, "must hold");
        let diagnostic = compile_module(&module).unwrap_err();
        assert_eq!(diagnostic.kind, DiagnosticKind::Validation);
        assert_eq!(diagnostic.primary, Some(expected));
        assert!(diagnostic.reason.contains(reason), "{}", diagnostic.reason);
        assert!(diagnostic.raw_nix.is_empty());
    }
}
