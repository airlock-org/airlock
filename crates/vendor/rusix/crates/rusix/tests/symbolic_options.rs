#![cfg(feature = "evaluation")]

//! Explicit dependencies resolve in NixOS, after ordinary Nix modules merge.
use rusix::{
    Config, Expr,
    interop::InputRef,
    nixos::{DefinitionPriority, NixosModule, OptionRef},
};
use rusix::{Diagnostic, DiagnosticKind, NixSession, Provenance, compile, nixos::compile_module};
use std::{
    fs,
    path::{Path, PathBuf},
};

const COMMAND: &[&str] = &[
    "systemd",
    "services",
    "example",
    "serviceConfig",
    "ExecStart",
];

#[allow(dead_code)] // Reuse the authoring example; the fixture script runs main.
#[path = "../../../examples/symbolic-option.rs"]
mod example;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> InputRef {
    InputRef::local(
        "symbolic-options",
        root().join("tests/fixtures/symbolic-options.nix"),
    )
}

fn base(command: Expr<String>) -> NixosModule {
    NixosModule::empty()
        .import_ref(fixtures().module("schema"))
        .module(
            NixosModule::new(Config::new().set_dynamic("services.example.port", 5432))
                .priority(DefinitionPriority::Default),
        )
        .add(example::service(command))
}

fn command() -> Expr<String> {
    OptionRef::<i64>::new("services.example.port")
        .into_expr()
        .to_text()
        .with_prefix("example --port=")
}

fn save_diagnostic(name: &str, diagnostic: &Diagnostic) {
    let out = root().join("target/symbolic-options").join(name);
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("diagnostic.txt"), diagnostic.render(&root())).unwrap();
    fs::write(
        out.join("diagnostic.json"),
        serde_json::to_vec_pretty(diagnostic).unwrap(),
    )
    .unwrap();
    fs::write(out.join("nix.stderr"), &diagnostic.raw_nix).unwrap();
}

#[test]
fn base_value_is_resolved_symbolically_and_contribution_boundaries_remain() {
    let module = base(command());
    assert_eq!(module.modules.len(), 2);

    let artifact = compile_module(&module).unwrap();
    assert_eq!(artifact.definitions.len(), 3);
    assert!(artifact.module.source.contains("({ config, ... }:"));
    assert!(
        artifact
            .module
            .source
            .contains("config.services.example.port")
    );
    assert!(!artifact.module.source.contains("deepSeq"));
    assert!(!artifact.module.source.contains("example --port=5432"));
    let value = NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, COMMAND, false)
        .unwrap()
        .value;
    assert_eq!(value, "example --port=5432");

    let out = root().join("target/symbolic-options/base");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("module.nix"), &artifact.module.source).unwrap();
    fs::write(
        out.join("value.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}

#[test]
fn unchanged_generated_artifact_follows_an_ordinary_nix_override() {
    let scratch = tempfile::tempdir().unwrap();
    let downstream = scratch.path().join("downstream.nix");
    fs::write(&downstream, "{ module = {}; }\n").unwrap();
    let module =
        base(command()).import_ref(InputRef::local("downstream", &downstream).module("module"));

    // Convert/lower once. Only the ordinary Nix contributor changes below.
    let artifact = compile_module(&module).unwrap();
    let source_before = artifact.module.source.clone();

    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos(&artifact, COMMAND, false)
            .unwrap()
            .value,
        "example --port=5432"
    );
    let nix_override = "{ module = { services.example.port = 6432; }; }\n";
    fs::write(&downstream, nix_override).unwrap();
    let value = session
        .evaluate_nixos(&artifact, COMMAND, false)
        .unwrap()
        .value;
    assert_eq!(value, "example --port=6432");
    assert_eq!(artifact.module.source, source_before);

    let out = root().join("target/symbolic-options/override");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("module.nix"), &artifact.module.source).unwrap();
    fs::write(out.join("downstream.nix"), nix_override).unwrap();
    fs::write(
        out.join("value.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}

#[test]
fn symbolic_reference_observes_nixos_force_over_normal_and_default() {
    let artifact = compile_module(
        &base(command())
            .import_ref(fixtures().module("ordinary"))
            .import_ref(fixtures().module("force")),
    )
    .unwrap();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(&artifact, COMMAND, false)
            .unwrap()
            .value,
        "example --port=7432"
    );
}

#[test]
fn symbolic_text_also_passes_a_real_upstream_nixos_option_type() {
    let module = base(command())
        .import("nixos/modules/services/networking/ssh/sshd.nix")
        .import_ref(fixtures().module("ordinary"))
        .add(Config::new().set_dynamic("services.openssh.banner", command()));
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate_nixos(
                &compile_module(&module).unwrap(),
                &["services", "openssh", "banner"],
                false
            )
            .unwrap()
            .value,
        "example --port=6432"
    );
}

#[test]
fn operation_on_final_port_retains_rust_origin_and_destination_path() {
    let reference = OptionRef::<i64>::new("services.example.port");
    let division_line = line!() + 1;
    let derived = Expr::int(44).divide(reference.into_expr());
    let module = base(derived.to_text().with_prefix("example --quotient="))
        .import_ref(fixtures().module("zero"));

    let artifact = compile_module(&module).unwrap();

    let session = NixSession::new().unwrap();

    let diagnostic = session
        .evaluate_nixos(&artifact, COMMAND, false)
        .unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixEval);
    assert_eq!(diagnostic.reason, "division by zero");
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, division_line);
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
    assert_eq!(
        diagnostic.option_path.as_deref(),
        Some("systemd.services.example.serviceConfig.ExecStart")
    );
    assert!(diagnostic.raw_nix.contains("rn-"));
    save_diagnostic("division", &diagnostic);
    fs::write(
        root().join("target/symbolic-options/division/module.nix"),
        &artifact.module.source,
    )
    .unwrap();

    // The same generated spans/ancestry remain useful without runtime markers.
    let mut event = diagnostic
        .raw_nix
        .lines()
        .filter_map(|line| line.strip_prefix("@nix "))
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["level"] == 0 && event["raw_msg"].is_string())
        .unwrap();
    event["trace"]
        .as_array_mut()
        .unwrap()
        .retain(|frame| !frame["raw_msg"].as_str().unwrap_or("").starts_with("rn-"));

    let fallback = Diagnostic::from_nix(
        DiagnosticKind::NixEval,
        &format!("@nix {event}"),
        &artifact.module,
        &session.root().join("module.nix"),
    );
    assert_eq!(fallback.primary, diagnostic.primary);
    assert_eq!(fallback.provenance, Provenance::SourceMap);
    assert!(
        fallback
            .related
            .iter()
            .any(|origin| origin.purpose == "set systemd.services.example.serviceConfig.ExecStart")
    );
}

#[test]
fn missing_option_reports_the_reference_creation_in_rust() {
    let reference_line = line!() + 1;
    let reference = OptionRef::<i64>::new("services.example.missing");

    let artifact = compile_module(&base(
        reference
            .into_expr()
            .to_text()
            .with_prefix("example --port="),
    ))
    .unwrap();

    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, COMMAND, false)
        .unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixEval);
    assert!(diagnostic.reason.contains("missing"));
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, reference_line);
    assert_eq!(
        diagnostic.primary.as_ref().unwrap().purpose,
        "NixOS option reference services.example.missing"
    );
    assert_eq!(diagnostic.provenance, Provenance::ErrorContext);
    save_diagnostic("missing", &diagnostic);
}

#[test]
fn unused_symbolic_dependency_does_not_force_a_throwing_option() {
    let artifact =
        compile_module(&base(command()).import_ref(fixtures().module("failing"))).unwrap();

    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_nixos(&artifact, &["services", "example", "enable"], false)
            .unwrap()
            .value,
        true
    );

    // A control evaluation proves that the referenced value really does fail.
    let diagnostic = session
        .evaluate_nixos(&artifact, COMMAND, false)
        .unwrap_err();
    assert!(diagnostic.reason.contains("unused port was evaluated"));
    assert_eq!(diagnostic.provenance, Provenance::ErrorContext);
    assert_eq!(
        diagnostic.primary.as_ref().unwrap().purpose,
        "NixOS option reference services.example.port"
    );
}

#[test]
fn boolean_string_and_list_values_use_the_same_scoped_reference() {
    let module = base(command())
        .assertion(
            "enabled",
            OptionRef::<bool>::new("services.example.enable").into_expr(),
            "must be enabled",
        )
        .add(
            Config::new()
                .set_dynamic(
                    "environment.ports",
                    vec![OptionRef::<i64>::new("services.example.port").into_expr()],
                )
                .set_dynamic(
                    "environment.command",
                    OptionRef::<String>::new("systemd.services.example.serviceConfig.ExecStart")
                        .into_expr(),
                ),
        );
    let value = NixSession::new()
        .unwrap()
        .evaluate_nixos(&compile_module(&module).unwrap(), &["environment"], true)
        .unwrap()
        .value;
    assert_eq!(
        value,
        serde_json::json!({"ports": [5432], "command": "example --port=5432"})
    );
}

#[test]
fn invalid_paths_and_use_outside_nixos_are_validation_errors() {
    for path in ["", "services..port", "bad\0path"] {
        let reference = OptionRef::<i64>::new(path).into_expr();
        let error = compile_module(&base(reference.to_text())).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Validation);
        assert!(
            error
                .primary
                .as_ref()
                .unwrap()
                .purpose
                .starts_with("NixOS option reference")
        );
    }
    let config = Config::new().set_dynamic("output", command());
    let error = compile(&config).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);
    assert!(error.reason.contains("NixosModule"));
    assert_eq!(
        error.primary.as_ref().unwrap().purpose,
        "NixOS option reference services.example.port"
    );
    let error = compile_module(&base(command().with_prefix("bad\0prefix"))).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);

    // Attribute segments are data, including text resembling Nix interpolation.
    let artifact = compile_module(&base(
        OptionRef::<i64>::new("services.example.${throw \"injection\"}")
            .into_expr()
            .to_text(),
    ))
    .unwrap();
    assert!(
        artifact
            .module
            .source
            .contains("\\${throw \\\"injection\\\"}")
    );
    let error = NixSession::new()
        .unwrap()
        .evaluate_nixos(&artifact, COMMAND, false)
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixEval);
    assert!(error.reason.contains("missing"));
}
