#![cfg(feature = "evaluation")]

use rusix::{Config, IntoConfig, IntoRusixValue, nixos::NixosModule};

use rusix::{
    DiagnosticKind, Generated, NixSession, compile,
    nixos::{NixosArtifact, compile_module},
};

#[allow(dead_code)] // Tests share the actual model; the example main is run by the fixture script.
#[path = "../../../examples/enum-option.rs"]
mod enum_option;

#[allow(dead_code)] // Tests share the actual model; the example main is run by the fixture script.
#[path = "../../../examples/exhaustive-match.rs"]
mod exhaustive_match;

#[allow(dead_code)] // Tests share the actual model; the example main is run by the fixture script.
#[path = "../../../examples/function-contracts.rs"]
mod function_contracts;

#[allow(dead_code)] // Tests share the actual model; the example main is run by the fixture script.
#[path = "../../../examples/invalid-states.rs"]
mod invalid_states;

#[allow(dead_code)] // Tests share the actual model; the example main is run by the fixture script.
#[path = "../../../examples/layered-validation.rs"]
mod layered_validation;

#[allow(dead_code)] // Tests share the actual model; the example main is run by the fixture script.
#[path = "../../../examples/typed-submodule.rs"]
mod typed_submodule;

#[allow(dead_code)] // Tests share the actual model; the example main is run by the fixture script.
#[path = "../../../examples/typed-values.rs"]
mod typed_values;

fn nix_module(source: &str) -> NixosArtifact {
    // Handwritten Nix comparison fixtures, not a frontend lowering shortcut.
    NixosArtifact {
        module: Generated {
            backend_metadata: None,
            source: source.into(),
            spans: vec![],
        },
        definitions: vec![],
        declarations: vec![],
        assertions: vec![],
        imports: vec![],
    }
}

macro_rules! roundtrip {
    ($test:ident, $example:ident, $name:literal, $expected:tt) => {
        #[test]
        fn $test() {
            let conversion_line = line!();
            let config = $example::model().into_config();
            let generated = compile(&config).unwrap();

            assert!(!generated.source.contains("deepSeq"));
            assert!(
                config
                    .assignments
                    .iter()
                    .all(|a| a.origin.file == file!() && a.origin.line == conversion_line)
            );

            let session = NixSession::new().unwrap();
            let evaluated = session.evaluate(&generated).unwrap();

            let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/typed-examples")
                .join($name);
            std::fs::create_dir_all(&out).unwrap();
            std::fs::write(out.join("generated.nix"), &generated.source).unwrap();
            std::fs::write(
                out.join("source-map.json"),
                serde_json::to_vec_pretty(&generated).unwrap(),
            )
            .unwrap();
            std::fs::write(
                out.join("value.json"),
                serde_json::to_vec_pretty(&evaluated.value).unwrap(),
            )
            .unwrap();
            std::fs::write(out.join("nix.stderr"), evaluated.raw_nix).unwrap();

            let rust = evaluated.value;
            assert_eq!(rust["demo"], serde_json::json!($expected));

            let nix = session
                .evaluate_nixos(
                    &nix_module(include_str!(concat!(
                        "../../../tests/comparisons/",
                        $name,
                        ".nix"
                    ))),
                    &["demo"],
                    true,
                )
                .unwrap()
                .value;

            assert_eq!(rust["demo"], nix);
        }
    };
}

roundtrip!(enum_is_typed_and_serializes_to_nixos_enum_value, enum_option, "enum-option",
    {"mode":"server", "acceptsConnections":true});

roundtrip!(endpoint_fields_survive_ir_ast_and_nix, typed_submodule, "typed-submodule",
    {"endpoint":{"host":"service.internal", "port":443}});

roundtrip!(tls_variant_carries_its_certificate, invalid_states, "invalid-states",
    {"transport":{"tls":true, "certificate":"/run/keys/service.pem", "privateKey":"/run/keys/service.key"}});

roundtrip!(typed_function_contract_produces_service_data, function_contracts, "function-contracts",
    {"endpoint":{"host":"service.internal", "port":8080}, "transport":{"tls":false}});

roundtrip!(enum_consumers_handle_both_variants, exhaustive_match, "exhaustive-match",
    {"mode":"client", "firewall":{"allowedPorts":[]}, "service":{"acceptsConnections":false}});

roundtrip!(distinct_domains_preserve_equal_primitive_values, typed_values, "typed-values",
    {"port":1000, "userId":1000, "host":"admin", "owner":"admin"});

// Deliberately invalid assemblies belong in tests, not the authoring example.
#[derive(IntoRusixValue)]
struct PortsOnly {
    ports: Vec<layered_validation::Port>,
}

#[derive(IntoConfig)]
struct ConflictingPorts {
    #[rusix(flatten)]
    first: layered_validation::SshContribution<PortsOnly>,
    #[rusix(flatten)]
    second: layered_validation::SshContribution<PortsOnly>,
}

fn ir_failure() -> Config {
    use layered_validation::{Port, Services, SshContribution};

    ConflictingPorts {
        first: SshContribution {
            services: Services {
                openssh: PortsOnly {
                    ports: vec![Port(22)],
                },
            },
        },
        second: SshContribution {
            services: Services {
                openssh: PortsOnly {
                    ports: vec![Port(2222)],
                },
            },
        },
    }
    .into_config()
}

fn nixos_failure() -> NixosModule {
    // Intentional generic escape hatch: NixOS owns the option schema.
    layered_validation::module()
        .add(Config::new().set_dynamic("services.openssh.exampleUnsupported", true))
}

#[test]
fn layered_validation_uses_ir_check_and_real_nixos_schema() {
    let config = ir_failure();
    let error = compile(&config).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);
    assert_eq!(error.primary, Some(config.assignments[1].origin.clone()));
    assert!(
        error
            .reason
            .contains("duplicate or conflicting option path")
    );

    let session = NixSession::new().unwrap();
    let module = layered_validation::module();
    let value = session
        .evaluate_nixos(
            &compile_module(&module).unwrap(),
            &["services", "openssh", "ports"],
            false,
        )
        .unwrap()
        .value;
    assert_eq!(value, serde_json::json!([22]));
    assert_eq!(
        value,
        session
            .evaluate_nixos(
                &nix_module(include_str!(
                    "../../../tests/comparisons/layered-validation.nix"
                )),
                &["services", "openssh", "ports"],
                false
            )
            .unwrap()
            .value
    );
    let module = nixos_failure();
    let error = session
        .evaluate_nixos(
            &compile_module(&module).unwrap(),
            &["services", "openssh", "enable"],
            false,
        )
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixosModule);
    assert_eq!(
        error.primary,
        Some(module.modules[1].config.assignments[0].origin.clone())
    );
    assert_eq!(
        error.option_path.as_deref(),
        Some("services.openssh.exampleUnsupported")
    );
}

#[test]
fn native_nixos_also_rejects_invalid_enum_and_transport_combination() {
    let session = NixSession::new().unwrap();
    let source = include_str!("../../../tests/comparisons/enum-option.nix")
        .replace("mode = \"server\";", "mode = \"proxy\";");
    let error = session
        .evaluate_nixos(&nix_module(&source), &["demo"], true)
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixosType);
    assert!(error.reason.contains("one of"));
    let source = include_str!("../../../tests/comparisons/invalid-states.nix")
        .replace("tls = true;", "tls = false;");
    let error = session
        .evaluate_nixos(&nix_module(&source), &["demo"], true)
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixosAssertion);
    assert!(
        error
            .reason
            .contains("certificate and private key presence must match TLS")
    );
    let source = include_str!("../../../tests/comparisons/invalid-states.nix").replace(
        "privateKey = \"/run/keys/service.key\";",
        "privateKey = null;",
    );
    let error = session
        .evaluate_nixos(&nix_module(&source), &["demo"], true)
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixosAssertion);
    assert!(error.reason.contains("private key presence must match TLS"));
}

#[test]
fn newtypes_lower_transparently_without_inspecting_files() {
    #[derive(IntoConfig)]
    struct Values {
        ports: Vec<typed_values::Port>,
        certificate: invalid_states::Certificate,
        private_key: invalid_states::PrivateKey,
    }

    let config = Values {
        ports: vec![typed_values::Port(0), typed_values::Port(u16::MAX)],
        certificate: invalid_states::Certificate("/run/nonexistent.pem".into()),
        private_key: invalid_states::PrivateKey("/run/nonexistent.key".into()),
    }
    .into_config();
    let value = NixSession::new()
        .unwrap()
        .evaluate(&compile(&config).unwrap())
        .unwrap()
        .value;
    assert_eq!(
        value,
        serde_json::json!({
            "ports": [0, 65535], "certificate": "/run/nonexistent.pem", "privateKey": "/run/nonexistent.key",
        })
    );
}

#[test]
fn plain_transport_has_no_credentials_and_both_mode_consumers_work() {
    let config = invalid_states::Root {
        demo: invalid_states::ServiceConfig {
            transport: invalid_states::Transport::Plain,
        },
    }
    .into_config();
    let value = NixSession::new()
        .unwrap()
        .evaluate(&compile(&config).unwrap())
        .unwrap()
        .value;
    assert_eq!(value["demo"]["transport"], serde_json::json!({"tls":false}));
    assert!(enum_option::accepts_connections(&enum_option::Mode::Server));
    assert!(!enum_option::accepts_connections(
        &enum_option::Mode::Client
    ));
    assert_eq!(
        exhaustive_match::firewall_policy(&exhaustive_match::Mode::Server)
            .allowed_ports
            .into_iter()
            .map(|port| port.0)
            .collect::<Vec<_>>(),
        vec![443],
    );
    assert!(
        exhaustive_match::firewall_policy(&exhaustive_match::Mode::Client)
            .allowed_ports
            .is_empty()
    );
    assert!(exhaustive_match::service_policy(&exhaustive_match::Mode::Server).accepts_connections);
    assert!(!exhaustive_match::service_policy(&exhaustive_match::Mode::Client).accepts_connections);
    assert_eq!(typed_values::listen(typed_values::Port(1000)).0, 1000);
}
