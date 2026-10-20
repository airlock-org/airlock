#![cfg(feature = "evaluation")]

use rusix::interop::raw::{InputRefExt, NixFunctionExt};
use rusix::ir::ReferencedExpression;
use rusix::{
    Config, IntoConfig,
    interop::{InputRef, Nixpkgs},
    nixos::NixosModule,
};
use rusix::{Diagnostic, NixSession, Provenance, compile, nixos::compile_module};

#[allow(dead_code)] // The fixture script runs main; tests share its authoring model.
#[path = "../../../examples/nix-interop.rs"]
mod example;

#[allow(dead_code)] // Shared fixture helpers include cases unused here.
#[path = "../src/bin/cli/support.rs"]
mod support;

mod config {
    use super::support::OpenSsh;
    use rusix::interop::raw::NixFunctionExt;
    use rusix::{Config, IntoConfig, interop::Nixpkgs, nixos::NixosModule};

    pub fn input() -> super::InputRef {
        super::InputRef::local(
            "example",
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/nix-interop-input.nix"),
        )
    }

    pub fn packages(packages: Vec<rusix::interop::PackageRef>) -> NixosModule {
        // Small evaluator surface: real package schema, excluding system defaults.
        NixosModule::empty()
            .import_ref(Nixpkgs::new().module("config/system-path.nix"))
            .module(
                NixosModule::new(super::example::packages(packages).into_config())
                    .priority(rusix::nixos::DefinitionPriority::Force),
            )
    }

    pub fn top_level() -> NixosModule {
        packages(vec![Nixpkgs::new().get("hello")])
    }

    pub fn nested() -> NixosModule {
        packages(vec![Nixpkgs::new().get("python312Packages.requests")])
    }

    pub fn missing() -> NixosModule {
        packages(vec![Nixpkgs::new().get("rusixDefinitelyMissing")])
    }

    pub fn missing_nested() -> NixosModule {
        packages(vec![
            Nixpkgs::new().get("python312Packages.rusixDefinitelyMissing"),
        ])
    }

    pub fn overlay() -> NixosModule {
        let pkgs = Nixpkgs::new().with_overlay(input().overlay("overlays.example"));
        packages(vec![pkgs.get("rusixOverlayHello")])
    }

    pub fn external_package() -> NixosModule {
        packages(vec![input().package("packages.example")])
    }

    pub fn missing_external() -> NixosModule {
        packages(vec![input().package("packages.missing")])
    }

    pub fn module() -> NixosModule {
        NixosModule::new(OpenSsh::new().enable(false).ports(vec![22]).into_config())
            .import_ref(Nixpkgs::new().module("services/networking/ssh/sshd.nix"))
    }

    pub fn broken_module() -> NixosModule {
        NixosModule::new(Config::new()).import_ref(Nixpkgs::new().module("misc/label.nix"))
    }

    pub fn missing_module() -> NixosModule {
        NixosModule::new(Config::new()).import_ref(Nixpkgs::new().module("rusix/missing.nix"))
    }

    pub fn external_module() -> NixosModule {
        NixosModule::new(Config::new().set_dynamic("services.rusixExternal.enable", true))
            .import_ref(input().module("nixosModules.example"))
    }

    pub fn broken_external_module() -> NixosModule {
        NixosModule::new(Config::new()).import_ref(input().module("nixosModules.broken"))
    }

    pub fn invalid_module() -> NixosModule {
        NixosModule::new(Config::new()).import_ref(input().module("nixosModules.invalid"))
    }

    pub fn function() -> Config {
        let uppercase = Nixpkgs::new().function("toUpper");
        Config::new().set_dynamic("result", uppercase.call("rusix"))
    }

    pub fn broken_function() -> Config {
        let uppercase = Nixpkgs::new().function("toUpper");
        Config::new().set_dynamic("result", uppercase.call(true))
    }

    pub fn missing_function() -> Config {
        Config::new().set_dynamic(
            "result",
            Nixpkgs::new().function("rusixMissing").call("value"),
        )
    }
}

fn package_value(module: NixosModule) -> serde_json::Value {
    let session = NixSession::new().unwrap();

    let artifact = compile_module(&module).unwrap();
    assert!(!artifact.module.source.contains("deepSeq"));
    let value = session.evaluate_system_packages(&artifact).unwrap().value;
    // Evaluation can materialize derivation records in this disposable store;
    // the external fixture's nonexistent builder proves that no build is run.
    value
}

fn package_error(module: NixosModule, purpose: &str) -> Box<Diagnostic> {
    let artifact = compile_module(&module).unwrap();
    let error = NixSession::new()
        .unwrap()
        .evaluate_system_packages(&artifact)
        .unwrap_err();
    assert_eq!(error.provenance, Provenance::ErrorContext, "{error:?}");
    let origin = error.primary.as_ref().unwrap();
    let rusix::ir::ValueKind::List(packages) = &module.modules[0].config.assignments[0].value.kind
    else {
        panic!("package IR list");
    };
    assert_eq!(*origin, packages[0].origin);
    assert_eq!(origin.purpose, purpose);
    assert!(origin.file.ends_with("tests/interop.rs"));
    assert_eq!(
        error.option_path.as_deref(),
        Some("environment.systemPackages")
    );
    assert!(!error.raw_nix.is_empty());
    error
}

#[test]
fn top_level_package_is_resolved_by_real_nixpkgs() {
    let value = package_value(config::top_level());
    assert_eq!(value[0]["pname"], "hello");
    assert_eq!(value[0]["isDerivation"], true);
}

#[test]
fn nested_package_is_resolved_without_rust_package_hierarchy() {
    let value = package_value(config::nested());
    assert_eq!(value[0]["pname"], "requests");
    assert_eq!(value[0]["isDerivation"], true);
}

#[test]
fn missing_top_level_package_maps_to_lookup() {
    let error = package_error(
        config::missing(),
        "nixpkgs package lookup rusixDefinitelyMissing",
    );
    assert!(error.reason.contains("rusixDefinitelyMissing"));
}

#[test]
fn missing_nested_package_maps_to_lookup() {
    let error = package_error(
        config::missing_nested(),
        "nixpkgs package lookup python312Packages.rusixDefinitelyMissing",
    );
    assert!(error.reason.contains("rusixDefinitelyMissing"));
}

#[test]
fn existing_opaque_module_composes_with_typed_openssh() {
    let value = NixSession::new()
        .unwrap()
        .evaluate_nixos_interop(
            &compile_module(&config::module()).unwrap(),
            &["services", "openssh", "ports"],
            false,
        )
        .unwrap()
        .value;
    assert_eq!(value, serde_json::json!([22]));
}

#[test]
fn upstream_module_failure_maps_to_import_boundary() {
    let module = config::broken_module();
    let error = NixSession::new()
        .unwrap()
        .evaluate_nixos_interop(
            &compile_module(&module).unwrap(),
            &["system", "nixos", "label"],
            false,
        )
        .unwrap_err();
    assert_eq!(error.provenance, Provenance::ImportBoundary, "{error:?}");
    assert_eq!(error.primary, Some(module.opaque_imports[0].1.clone()));
    assert!(error.reason.contains("version"));
}

#[test]
fn missing_module_maps_to_lookup() {
    let module = config::missing_module();
    let error = NixSession::new()
        .unwrap()
        .evaluate_nixos_interop(&compile_module(&module).unwrap(), &["services"], false)
        .unwrap_err();
    assert_eq!(
        error.primary,
        Some(module.opaque_imports[0].0.reference().origin.clone()),
        "{error:?}"
    );
    assert!(!error.raw_nix.is_empty());
}

#[test]
fn existing_lib_function_executes_in_nix() {
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(config::function()).unwrap())
        .unwrap()
        .value;
    assert_eq!(value["result"], "RUSIX");
}

#[test]
fn failed_lib_function_maps_to_rust_call() {
    let config = config::broken_function();

    let generated = compile(&config).unwrap();
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert_eq!(error.provenance, Provenance::ErrorContext);
    assert_eq!(
        error.primary,
        Some(config.assignments[0].value.origin.clone())
    );
    assert_eq!(
        error.primary.as_ref().unwrap().purpose,
        "opaque Nix function call"
    );
    assert!(
        error
            .primary
            .as_ref()
            .unwrap()
            .file
            .ends_with("tests/interop.rs")
    );
    assert!(!error.raw_nix.is_empty());
}

#[test]
fn missing_lib_function_maps_to_lookup() {
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(config::missing_function()).unwrap())
        .unwrap_err();
    assert_eq!(
        error.primary.unwrap().purpose,
        "nixpkgs lib function lookup rusixMissing"
    );
}

#[test]
fn nix_evaluates_overlay_and_resolves_its_package() {
    let value = package_value(config::overlay());
    assert_eq!(value[0]["pname"], "hello");
    assert_eq!(value[0]["isDerivation"], true);
}

#[test]
fn local_external_package_resolves() {
    let value = package_value(config::external_package());
    assert_eq!(value[0]["name"], "rusix-external-1.0");
    assert_eq!(value[0]["isDerivation"], true);
}

#[test]
fn local_external_module_resolves() {
    let value = NixSession::new()
        .unwrap()
        .evaluate_nixos_interop(
            &compile_module(&config::external_module()).unwrap(),
            &["services", "rusixExternal", "enable"],
            false,
        )
        .unwrap()
        .value;
    assert_eq!(value, true);
}

#[test]
fn missing_external_reference_maps_to_lookup() {
    let error = package_error(
        config::missing_external(),
        "external package lookup packages.missing",
    );
    assert!(error.reason.contains("missing"));
}

#[test]
fn failed_external_module_retains_import_boundary() {
    let module = config::broken_external_module();
    let error = NixSession::new()
        .unwrap()
        .evaluate_nixos_interop(
            &compile_module(&module).unwrap(),
            &["services", "rusixExternal", "value"],
            false,
        )
        .unwrap_err();
    assert_eq!(
        error.primary,
        Some(module.opaque_imports[0].1.clone()),
        "{error:?}"
    );
    assert_eq!(error.provenance, Provenance::ImportBoundary);
    assert!(error.reason.contains("local external module failed"));
}

#[test]
fn invalid_module_reports_rust_boundary() {
    let module = config::invalid_module();
    let error = NixSession::new()
        .unwrap()
        .evaluate_nixos_interop(&compile_module(&module).unwrap(), &["services"], false)
        .unwrap_err();
    assert_eq!(
        error.primary,
        Some(module.opaque_imports[0].1.clone()),
        "{error:?}"
    );
    assert_eq!(error.provenance, Provenance::ErrorContext);
}

#[test]
fn structured_paths_are_validated_and_escaped() {
    let pkgs = Nixpkgs::new();
    assert!(compile(Config::new().set_dynamic("value", pkgs.get("bad..path"))).is_err());
    assert!(
        compile_module(&NixosModule::new(Config::new()).import_ref(pkgs.module("../escape.nix")))
            .is_err()
    );
    let input = InputRef::local("example", "/nonexistent.nix");

    let generated =
        compile(Config::new().set_dynamic("value", input.package("packages.odd\"${key}"))).unwrap();
    assert!(generated.source.contains("\\\"\\${"));
}

#[test]
fn package_handle_can_be_passed_to_an_opaque_function() {
    let pkgs = Nixpkgs::new();
    let config =
        Config::new().set_dynamic("name", pkgs.function("getName").call(pkgs.get("hello")));
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(&config).unwrap())
        .unwrap()
        .value;
    assert_eq!(value["name"], "hello");
}

#[test]
fn missing_input_file_maps_to_rust_lookup() {
    let input = InputRef::local("missing", "/rusix-definitely-missing-input.nix");
    let package = input.package("packages.example");
    let origin = package.reference().origin.clone();

    let generated = compile(Config::new().set_dynamic("value", package)).unwrap();
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert_eq!(error.primary, Some(origin));
    assert_eq!(error.provenance, Provenance::ErrorContext);
}

#[test]
fn unrelated_opaque_values_remain_lazy() {
    let pkgs = Nixpkgs::new();
    let config = Config::new()
        .set_dynamic("good", pkgs.function("toUpper").call("works"))
        .set_dynamic("bad", pkgs.get("rusixMissing"));

    let session = NixSession::new().unwrap();
    // Stage sources using the public interop entrypoint, selecting a result
    // explicitly rather than demanding the sibling value.
    let selected = compile(
        Config::new().set_dynamic(
            "value",
            InputRef::local(
                "example",
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/nix-interop-input.nix"),
            )
            .value("values.lazy")
            .select("good"),
        ),
    )
    .unwrap();
    assert_eq!(
        session.evaluate_interop(&selected).unwrap().value["value"],
        42
    );
    assert_eq!(
        session
            .evaluate_attribute(&compile(&config).unwrap(), "good")
            .unwrap()
            .value,
        "WORKS"
    );
    let error = session
        .evaluate_attribute(&compile(&config).unwrap(), "bad")
        .unwrap_err();
    assert_eq!(
        error.primary.unwrap().purpose,
        "nixpkgs package lookup rusixMissing"
    );
}

#[test]
fn delayed_function_failure_maps_to_selection_not_original_call() {
    let value = config::input()
        .function("functions.lazy")
        .call(true)
        .select("bad");

    let generated = compile(Config::new().set_dynamic("result", value)).unwrap();
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert_eq!(error.primary.unwrap().purpose, "opaque Nix selection bad");
    assert_eq!(error.provenance, Provenance::SourceMap);
    assert_eq!(error.reason, "delayed external call failed");
}

#[test]
fn nixos_checks_actual_package_category() {
    let module = config::packages(vec![config::input().package("nixosModules.example")]);

    let artifact = compile_module(&module).unwrap();
    let error = NixSession::new()
        .unwrap()
        .evaluate_system_packages(&artifact)
        .unwrap_err();
    assert_eq!(error.kind, rusix::DiagnosticKind::NixosType);
    assert_eq!(
        error.option_path.as_deref(),
        Some("environment.systemPackages")
    );
    assert_eq!(
        error.primary,
        Some(module.modules[0].config.assignments[0].origin.clone())
    );
    assert!(error.reason.contains("package"));
}

#[test]
fn authoring_example_composes_packages_modules_overlay_and_external_input() {
    let module = example::module(config::input())
        .import_ref(Nixpkgs::new().module("config/system-path.nix"));
    // Keep the example's natural contribution boundaries; apply test-only priority
    // to its package contribution so unrelated full-system defaults are excluded.
    let mut module = module;
    module.modules[0] = module.modules[0]
        .clone()
        .priority(rusix::nixos::DefinitionPriority::Force);

    let artifact = compile_module(&module).unwrap();

    let session = NixSession::new().unwrap();
    let packages = session.evaluate_system_packages(&artifact).unwrap().value;
    assert_eq!(packages.as_array().unwrap().len(), 4);
    assert_eq!(packages[0]["pname"], "hello");
    assert_eq!(packages[1]["pname"], "requests");
    assert_eq!(packages[2]["pname"], "hello");
    assert_eq!(packages[3]["pname"], "rusix-external");
    assert_eq!(
        session
            .evaluate_nixos_interop(&artifact, &["services", "rusixExternal", "enable"], false,)
            .unwrap()
            .value,
        true,
    );
    assert_eq!(
        session
            .evaluate_nixos_interop(&artifact, &["services", "openssh", "ports"], false,)
            .unwrap()
            .value,
        serde_json::json!([22]),
    );
}

#[test]
fn authoring_example_owned_transport_keeps_its_structural_alternatives() {
    let session = NixSession::new().unwrap();
    for (transport, expected) in [
        (example::Transport::Plain, serde_json::json!({"tls": false})),
        (
            example::Transport::Tls {
                certificate: "/run/cert.pem".into(),
                private_key: "/run/key.pem".into(),
            },
            serde_json::json!({"tls": true, "certificate": "/run/cert.pem", "privateKey": "/run/key.pem"}),
        ),
    ] {
        let config = example::OwnedContribution { demo: transport }.into_config();
        let value = session.evaluate(&compile(&config).unwrap()).unwrap().value;
        assert_eq!(value["demo"], expected);
    }
}
