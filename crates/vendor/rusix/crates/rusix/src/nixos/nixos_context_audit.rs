//! Diagnostic comparison at assertion, downstream-override and PostgreSQL schema boundaries.
use super::*;
use crate::compiler::render;
use crate::{
    Config, Expr,
    nixos::{DefinitionPriority, OptionRef},
};

#[allow(dead_code)]
#[path = "../../../../examples/postgresql-nixos-module/model.rs"]
mod model;

#[allow(dead_code)]
#[path = "../../../../examples/postgresql-nixos-module/options.rs"]
mod options;

#[allow(dead_code)]
#[path = "../../../../examples/postgresql-nixos-module/lowering.rs"]
mod lowering;

#[allow(dead_code)]
#[path = "../../../../examples/postgresql-nixos-module/schema.rs"]
mod schema;

fn artifacts(module: &NixosModule) -> [NixosArtifact; 2] {
    let (mut ast, mut current) = lower_module(module).unwrap();
    current.module = render(&ast);
    crate::compiler::context_audit::contexts(&mut ast, true);
    let mut legacy = current.clone();
    legacy.module = render(&ast);
    [legacy, current]
}

#[test]
fn explicit_nixos_assertion_retains_its_origin_and_message_marker() {
    let module =
        NixosModule::empty().assertion("port-policy", Expr::boolean(false), "port rejected");
    let expected = module.assertions[0].origin.clone();
    let session = NixSession::new().unwrap();

    for artifact in artifacts(&module) {
        let error = session
            .evaluate_nixos(&artifact, &["services", "openssh", "enable"], true)
            .unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::NixosAssertion);
        assert_eq!(error.primary.as_ref(), Some(&expected));
        assert_eq!(error.option_path.as_deref(), Some("assertions.port-policy"));
        assert!(error.reason.contains("port rejected"));
        assert!(!error.raw_nix.is_empty());
    }
}

#[test]
fn postgresql_foreign_overrides_preserve_child_diagnostics_and_schema_blame() {
    let port = OptionRef::<i64>::new("services.postgresql.settings.port").into_expr();
    let operation_line = line!() + 1;
    let derived = Expr::int(44).divide(port);
    let module = schema::module()
        .module(lowering::implementation())
        .module(
            NixosModule::new(Config::new().set_dynamic("services.postgresql.settings.port", 5432))
                .priority(DefinitionPriority::Default),
        )
        .add(Config::new().set_dynamic("environment.variables.RUSIX_PORT", derived.to_text()));
    let artifacts = artifacts(&module);
    let session = NixSession::new().unwrap();
    let driver = Generated {
        source: r#"
            let system = import ./nixpkgs-full/nixos/lib/eval-config.nix {
              system = "x86_64-linux";
              modules = [ ./module.nix ./downstream.nix {
                disabledModules = [ "services/databases/postgresql.nix" ];
                system.stateVersion = "24.11";
                services.postgresql.enable = true;
              } ];
            }; in system.config.environment.variables.RUSIX_PORT
        "#
        .into(),
        ..Generated::default()
    };

    for artifact in &artifacts {
        let source = artifact.module.source.clone();
        fs::write(
            session.root().join("downstream.nix"),
            "{ services.postgresql.settings.port = 6432; }",
        )
        .unwrap();
        assert_eq!(
            session
                .evaluate_nixos_with_driver(artifact, &driver)
                .unwrap()
                .value,
            "0"
        );

        fs::write(
            session.root().join("downstream.nix"),
            "{ services.postgresql.settings.port = 0; }",
        )
        .unwrap();
        let failure = session
            .evaluate_nixos_with_driver(artifact, &driver)
            .unwrap_err();
        assert_eq!(failure.reason, "division by zero");
        assert_eq!(failure.primary.as_ref().unwrap().line, operation_line);
        assert_eq!(failure.primary.as_ref().unwrap().file, file!());
        assert!(
            failure
                .related
                .iter()
                .any(|o| o.purpose == "set environment.variables.RUSIX_PORT")
        );
        assert!(!failure.raw_nix.is_empty());

        fs::write(
            session.root().join("downstream.nix"),
            "{ services.postgresql.settings.port = -1; }",
        )
        .unwrap();
        let failure = session
            .evaluate_nixos_with_driver(artifact, &driver)
            .unwrap_err();
        assert!(failure.reason.contains("port"));
        assert!(failure.reason.contains("not of type"));
        assert!(
            failure.primary.is_none(),
            "foreign invalid definition: {failure:?}"
        );
        assert!(!failure.raw_nix.is_empty());
        assert_eq!(artifact.module.source, source);
    }
}

#[test]
fn merged_rust_value_failure_outranks_a_separate_final_option_reader() {
    let operation_line = line!() + 1;
    let failure = Expr::int(44).divide(Expr::int(0));
    let reader = OptionRef::<i64>::new("services.postgresql.settings.max_connections")
        .into_expr()
        .to_text()
        .with_prefix("connections=");
    let module = schema::module()
        .module(lowering::implementation())
        .add(Config::new().set_dynamic("services.postgresql.settings.max_connections", failure))
        .add(Config::new().set_dynamic("environment.variables.RUSIX_DIAGNOSTIC", reader));
    let session = NixSession::new().unwrap();
    let driver = Generated {
        source: r#"
            let system = import ./nixpkgs-full/nixos/lib/eval-config.nix {
              system = "x86_64-linux";
              modules = [ ./module.nix {
                disabledModules = [ "services/databases/postgresql.nix" ];
                system.stateVersion = "24.11";
              } ];
            }; in system.config.environment.variables.RUSIX_DIAGNOSTIC
        "#
        .into(),
        ..Generated::default()
    };

    for artifact in artifacts(&module) {
        let error = session
            .evaluate_nixos_with_driver(&artifact, &driver)
            .unwrap_err();
        assert_eq!(error.reason, "division by zero");
        assert_eq!(error.primary.as_ref().unwrap().line, operation_line);
        assert_eq!(error.primary.as_ref().unwrap().file, file!());
        assert!(
            error
                .related
                .iter()
                .any(|o| o.purpose == "set services.postgresql.settings.max_connections")
        );
        assert!(!error.raw_nix.is_empty());
    }
}
