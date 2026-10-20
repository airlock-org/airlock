#![cfg(feature = "evaluation")]

//! Derive semantics exercised through real lowering and NixOS merges.
use rusix::interop::raw::NixFunctionExt;
use rusix::ir::ReferencedExpression;
use rusix::{
    Config, Expr, IntoConfig, IntoRusixValue, RusixValue,
    interop::{InputRef, ModuleRef, NixFunction, Nixpkgs, OverlayRef, PackageRef, raw::NixValue},
    ir::ValueKind,
    nixos::{DefinitionPriority, NixosModule, OptionRef},
};
use rusix::{DiagnosticKind, NixSession, Provenance, compile, nixos::compile_module};
use std::path::Path;

#[derive(IntoRusixValue)]
struct Port(u16);

#[derive(IntoRusixValue)]
struct Endpoint {
    host: String,
    port: Port,
}

#[test]
fn nesting_decides_placement_and_reusable_values_have_no_global_paths() {
    // IntoConfig also supplies value conversion, so the same derive may be nested.
    #[derive(IntoConfig)]
    struct Reusable {
        endpoint: Endpoint,
    }

    #[derive(IntoConfig)]
    struct Placement {
        first: Reusable,
        second: Reusable,
    }

    let line = line!() + 1;
    let config = Placement {
        first: Reusable {
            endpoint: Endpoint {
                host: "one".into(),
                port: Port(80),
            },
        },
        second: Reusable {
            endpoint: Endpoint {
                host: "two".into(),
                port: Port(443),
            },
        },
    }
    .into_config();
    // track_caller reports the conversion call, whose token is on this line.
    let conversion_line = line!() - 2;
    assert!(conversion_line >= line);
    let ids: std::collections::BTreeSet<_> = config
        .assignments
        .iter()
        .map(|a| &a.value.origin.id)
        .collect();
    assert_eq!(ids.len(), 4);

    for binding in &config.assignments {
        assert_eq!(binding.origin.file, file!());
        assert_eq!(binding.origin.line, conversion_line);
        assert_eq!(binding.value.origin.line, conversion_line);
    }
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&compile(&config).unwrap())
            .unwrap()
            .value,
        serde_json::json!({"first":{"endpoint":{"host":"one","port":80}},"second":{"endpoint":{"host":"two","port":443}}})
    );
}

#[test]
fn field_names_are_literal_data_and_raw_identifiers_have_logical_names() {
    #[derive(IntoConfig)]
    struct Names {
        r#type: bool,
        #[rusix(rename = "api.v1")]
        version: i64,
        #[rusix(rename = "${throw \"injection\"}")]
        escaped: String,
    }

    let config = Names {
        r#type: true,
        version: 1,
        escaped: "safe".into(),
    }
    .into_config();
    assert_eq!(config.assignments[1].path_segments(), &["api.v1"]);
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&compile(&config).unwrap())
            .unwrap()
            .value,
        serde_json::json!({"type":true,"api.v1":1,"${throw \"injection\"}":"safe"})
    );
}

#[test]
fn flatten_skip_generics_and_borrowed_values_have_only_requested_effects() {
    struct LocalState;

    #[derive(IntoRusixValue)]
    struct Metadata<'a> {
        label: &'a str,
    }

    #[derive(IntoConfig)]
    struct Document<'a, T> {
        #[rusix(flatten)]
        metadata: Metadata<'a>,
        value: T,
        #[rusix(skip)]
        state: LocalState,
    }

    let document = Document {
        metadata: Metadata { label: "demo" },
        value: Port(22),
        state: LocalState,
    };
    let _ = &document.state; // Not converted and needs no IntoRusixValue implementation.

    let generated = compile(document.into_config()).unwrap();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&generated)
            .unwrap()
            .value,
        serde_json::json!({"label":"demo","value":22})
    );
}

#[test]
fn derived_unit_enums_remain_exhaustive_rust_types() {
    #[derive(IntoRusixValue)]
    enum Mode {
        Server,
        Client,
    }

    #[derive(IntoConfig)]
    struct ConfigModel {
        modes: Vec<Mode>,
    }

    let config = ConfigModel {
        modes: vec![Mode::Server, Mode::Client],
    }
    .into_config();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&compile(&config).unwrap())
            .unwrap()
            .value,
        serde_json::json!({"modes":["server","client"]})
    );
}

#[test]
fn record_lists_are_structured_lazy_and_keep_operation_origins() {
    #[derive(IntoRusixValue)]
    struct Item {
        number: Expr<i64>,
    }

    #[derive(IntoConfig)]
    struct Model {
        good: bool,
        items: Vec<Item>,
    }

    let division_line = line!() + 1;
    let bad = Expr::int(44).divide(Expr::int(0));
    let model = Model {
        good: true,
        items: vec![Item { number: bad }],
    };
    let conversion_line = line!() + 1;
    let config = model.into_config();

    let generated = compile(&config).unwrap();

    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        true
    );

    let diagnostic = session.evaluate_attribute(&generated, "items").unwrap_err();
    assert_eq!(diagnostic.reason, "division by zero");
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, division_line);
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
    assert!(
        diagnostic
            .related
            .iter()
            .any(|o| o.purpose == "set items" && o.line == conversion_line)
    );

    let ValueKind::List(items) = &config.assignments[1].value.kind else {
        panic!()
    };
    assert_eq!(items[0].origin.file, file!());
    assert_eq!(items[0].origin.line, conversion_line);
    assert!(!generated.source.contains("deepSeq"));

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

    let fallback = rusix::Diagnostic::from_nix(
        DiagnosticKind::NixEval,
        &format!("@nix {event}"),
        &generated,
        &session.root().join("generated.nix"),
    );
    assert_eq!(fallback.primary, diagnostic.primary);
    assert_eq!(fallback.provenance, Provenance::SourceMap);
}

#[derive(IntoConfig)]
struct Ssh<T> {
    services: Services<T>,
}

#[derive(IntoRusixValue)]
struct Services<T> {
    openssh: T,
}

#[derive(IntoRusixValue)]
struct Ports {
    ports: Vec<u16>,
}

#[derive(IntoRusixValue)]
struct AuthorizedUser {
    authorized_keys_command_user: String,
}

fn ssh() -> NixosModule {
    NixosModule::empty().import("nixos/modules/services/networking/ssh/sshd.nix")
}

#[test]
fn independent_derived_contributions_merge_lists_and_keep_nixos_priorities() {
    let module = ssh()
        .add(Ssh {
            services: Services {
                openssh: Ports { ports: vec![22] },
            },
        })
        .add(Ssh {
            services: Services {
                openssh: Ports { ports: vec![2222] },
            },
        });
    assert_eq!(module.modules.len(), 2);

    let session = NixSession::new().unwrap();
    let selection = &["services", "openssh", "ports"];

    let mut ports = session
        .evaluate_nixos(&compile_module(&module).unwrap(), selection, false)
        .unwrap()
        .value
        .as_array()
        .unwrap()
        .clone();
    ports.sort_by_key(|v| v.as_i64().unwrap());
    assert_eq!(
        ports,
        serde_json::json!([22, 2222]).as_array().unwrap().clone()
    );
    let module = module.module(
        NixosModule::new(
            Ssh {
                services: Services {
                    openssh: Ports { ports: vec![3333] },
                },
            }
            .into_config(),
        )
        .priority(DefinitionPriority::Force),
    );
    assert_eq!(
        session
            .evaluate_nixos(&compile_module(&module).unwrap(), selection, false)
            .unwrap()
            .value,
        serde_json::json!([3333])
    );
}

#[test]
fn conflicting_derived_contributions_report_both_add_calls() {
    let first_line = line!() + 1;
    let module = ssh().add(Ssh {
        services: Services {
            openssh: AuthorizedUser {
                authorized_keys_command_user: "root".into(),
            },
        },
    });

    let second_line = line!() + 1;
    let module = module.add(Ssh {
        services: Services {
            openssh: AuthorizedUser {
                authorized_keys_command_user: "nobody".into(),
            },
        },
    });

    let artifact = compile_module(&module).unwrap();
    assert_eq!(artifact.definitions.len(), 2);

    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_nixos(
            &artifact,
            &["services", "openssh", "authorizedKeysCommandUser"],
            false,
        )
        .unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixosMerge);
    assert_eq!(diagnostic.origins.len(), 2);
    for line in [first_line, second_line] {
        assert!(diagnostic.origins.iter().any(|o| {
            o.origin
                .as_ref()
                .is_some_and(|o| o.file == file!() && o.line == line)
        }));
    }
    assert_eq!(
        diagnostic.option_path.as_deref(),
        Some("services.openssh.authorizedKeysCommandUser")
    );
}

#[test]
fn derive_preserves_opaque_package_module_function_overlay_and_value_identities() {
    #[derive(IntoConfig)]
    struct Handles {
        package: PackageRef,
        module: ModuleRef,
        function: NixFunction,
        overlay: OverlayRef,
        value: NixValue,
    }

    let pkgs = Nixpkgs::new();
    let package = pkgs.get("hello");
    let package_origin = package.reference().origin.clone();
    let input = InputRef::local("fixture", "unused.nix");
    let config = Handles {
        package,
        module: pkgs.module("misc/label.nix"),
        function: pkgs.function("toUpper"),
        overlay: input.overlay("overlay"),
        value: pkgs.function("toUpper").call("rusix"),
    }
    .into_config();

    for binding in &config.assignments[..4] {
        assert!(matches!(binding.value.kind, ValueKind::Reference(_)));
    }
    assert_eq!(config.assignments[0].value.origin, package_origin);
    assert!(matches!(
        config.assignments[4].value.kind,
        ValueKind::Apply(..)
    ));
    compile(&config).unwrap(); // No lookup is performed during Rust conversion/lowering.
}

#[test]
fn derived_package_lists_resolve_real_nixpkgs_packages() {
    #[derive(IntoConfig)]
    struct Packages {
        environment: Environment,
    }

    #[derive(IntoRusixValue)]
    struct Environment {
        system_packages: Vec<PackageRef>,
    }

    let pkgs = Nixpkgs::new();
    let module = NixosModule::empty()
        .import_ref(pkgs.module("config/system-path.nix"))
        .module(
            NixosModule::new(
                Packages {
                    environment: Environment {
                        system_packages: vec![pkgs.get("hello")],
                    },
                }
                .into_config(),
            )
            .priority(DefinitionPriority::Force),
        );

    let value = NixSession::new()
        .unwrap()
        .evaluate_system_packages(&compile_module(&module).unwrap())
        .unwrap()
        .value;
    assert_eq!(value[0]["pname"], "hello");
    assert_eq!(value[0]["isDerivation"], true);
}

#[test]
fn symbolic_fields_follow_ordinary_nix_overrides_without_reconversion() {
    #[derive(IntoConfig)]
    struct Model {
        services: ExampleServices,
        environment: Environment,
    }

    #[derive(IntoRusixValue)]
    struct ExampleServices {
        example: Example,
    }

    #[derive(IntoRusixValue)]
    struct Example {
        enable: bool,
        port: i64,
    }

    #[derive(IntoRusixValue)]
    struct Environment {
        command: Expr<String>,
        expected_ports: Vec<OptionRef<i64>>,
    }

    let reference = OptionRef::<i64>::new("services.example.port");
    let reference_line = line!() + 1;
    let missing = OptionRef::<i64>::new("services.example.missing");

    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path().join("downstream.nix");
    std::fs::write(&path, "{ module = {}; }").unwrap();
    let input = InputRef::local("downstream", &path);
    let fixture = InputRef::local(
        "schema",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/symbolic-options.nix"),
    );
    let module = NixosModule::empty()
        .import_ref(fixture.module("schema"))
        .import_ref(input.module("module"))
        .module(
            NixosModule::new(
                Model {
                    services: ExampleServices {
                        example: Example {
                            enable: true,
                            port: 5432,
                        },
                    },
                    environment: Environment {
                        command: reference
                            .into_expr()
                            .to_text()
                            .with_prefix("example --port="),
                        expected_ports: vec![missing],
                    },
                }
                .into_config(),
            )
            .priority(DefinitionPriority::Default),
        );

    let artifact = compile_module(&module).unwrap();

    let session = NixSession::new().unwrap();
    let selection = &["environment", "command"];
    assert_eq!(
        session
            .evaluate_nixos(&artifact, selection, false)
            .unwrap()
            .value,
        "example --port=5432"
    );
    std::fs::write(&path, "{ module = { services.example.port = 6432; }; }").unwrap();
    assert_eq!(
        session
            .evaluate_nixos(&artifact, selection, false)
            .unwrap()
            .value,
        "example --port=6432"
    );

    // The unused missing dependency stayed lazy during both evaluations.
    let diagnostic = session
        .evaluate_nixos(&artifact, &["environment", "expectedPorts"], false)
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, reference_line);
    assert_eq!(diagnostic.provenance, Provenance::ErrorContext);
}

#[test]
fn invalid_flatten_shapes_and_duplicate_fields_are_ir_errors() {
    #[derive(IntoConfig)]
    struct BadShape {
        #[rusix(flatten)]
        number: i64,
    }

    assert!(
        compile(BadShape { number: 1 }.into_config())
            .unwrap_err()
            .reason
            .contains("flatten requires a record")
    );
    #[derive(IntoConfig)]
    struct Duplicate {
        listen_port: i64,
        #[rusix(rename = "listenPort")]
        second: i64,
    }

    let error = compile(
        Duplicate {
            listen_port: 1,
            second: 2,
        }
        .into_config(),
    )
    .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);
    assert!(error.reason.contains("duplicate"));

    let error = compile(Config::from_value(1.into_value())).unwrap_err();
    assert!(error.reason.contains("rooted record"));
}

#[test]
fn empty_records_and_empty_lists_are_preserved() {
    #[derive(IntoRusixValue)]
    struct Empty {}

    #[derive(IntoConfig)]
    struct Model {
        empty: Empty,
        list: Vec<Endpoint>,
    }

    let config = Model {
        empty: Empty {},
        list: vec![],
    }
    .into_config();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&compile(&config).unwrap())
            .unwrap()
            .value,
        serde_json::json!({"empty":{},"list":[]})
    );
}

#[test]
fn default_lower_camel_and_explicit_lower_camel_match() {
    #[derive(IntoRusixValue)]
    struct DefaultNames {
        enable_feature: bool,
        listen_port: u16,
    }

    #[derive(IntoRusixValue)]
    #[rusix(rename_all = "lowerCamelCase")]
    struct ExplicitNames {
        enable_feature: bool,
        listen_port: u16,
    }

    #[derive(IntoConfig)]
    struct Root {
        default_names: DefaultNames,
        explicit_names: ExplicitNames,
        #[rusix(rename = "literal_name")]
        exceptional_name: String,
    }

    let config = Root {
        default_names: DefaultNames {
            enable_feature: true,
            listen_port: 8080,
        },
        explicit_names: ExplicitNames {
            enable_feature: true,
            listen_port: 8080,
        },
        exceptional_name: "preserved".into(),
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
            "defaultNames": {"enableFeature": true, "listenPort": 8080},
            "explicitNames": {"enableFeature": true, "listenPort": 8080},
            "literal_name": "preserved",
        })
    );
}

#[test]
fn naming_conventions_are_local_and_literal_rename_wins() {
    #[derive(IntoConfig)]
    #[rusix(rename_all = "PascalCase")]
    struct Root {
        service_config: ServiceConfig,
    }

    #[derive(IntoRusixValue)]
    #[rusix(rename_all = "PascalCase")]
    struct ServiceConfig {
        exec_start: String,
        restart: String,
        user: String,
        working_directory: String,
        #[rusix(rename = "exact_external_name")]
        arbitrary_field: bool,
        nested_value: LowerCamel,
        #[rusix(flatten)]
        flattened: LowerCamel,
    }

    #[derive(IntoRusixValue)]
    struct LowerCamel {
        listen_port: u16,
    }

    let config = Root {
        service_config: ServiceConfig {
            exec_start: "example".into(),
            restart: "always".into(),
            user: "example".into(),
            working_directory: "/tmp".into(),
            arbitrary_field: true,
            nested_value: LowerCamel { listen_port: 8080 },
            flattened: LowerCamel { listen_port: 9090 },
        },
    }
    .into_config();

    let value = NixSession::new()
        .unwrap()
        .evaluate(&compile(&config).unwrap())
        .unwrap()
        .value;
    assert_eq!(
        value,
        serde_json::json!({"ServiceConfig": {
            "ExecStart": "example", "Restart": "always", "User": "example", "WorkingDirectory": "/tmp",
            "exact_external_name": true, "NestedValue": {"listenPort": 8080}, "listenPort": 9090,
        }})
    );
}

#[test]
fn unit_enum_naming_and_provenance_survive_lowering() {
    #[derive(IntoRusixValue)]
    enum Mode {
        Server,
        ReadOnly,
        #[rusix(rename = "client-only")]
        Client,
        #[cfg(any())]
        Disabled,
    }

    #[derive(IntoRusixValue)]
    #[rusix(rename_all = "PascalCase")]
    enum ExternalMode {
        ReadOnly,
        #[rusix(rename = "exact-name")]
        ReadWrite,
    }

    #[derive(IntoConfig)]
    struct Root {
        modes: Vec<Mode>,
        external: Vec<ExternalMode>,
    }

    let model = Root {
        modes: vec![Mode::Server, Mode::ReadOnly, Mode::Client],
        external: vec![ExternalMode::ReadOnly, ExternalMode::ReadWrite],
    };
    let conversion_line = line!() + 1;
    let config = model.into_config();

    for binding in &config.assignments {
        assert_eq!(binding.origin.file, file!());
        assert_eq!(binding.origin.line, conversion_line);
        let ValueKind::List(values) = &binding.value.kind else {
            panic!("enum list expected");
        };
        for value in values {
            assert_eq!(value.origin.line, conversion_line);
        }
    }
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&compile(&config).unwrap())
            .unwrap()
            .value,
        serde_json::json!({"modes":["server","readOnly","client-only"],"external":["ReadOnly","exact-name"]})
    );
}

#[test]
fn derived_records_remain_atomic_when_used_as_opaque_nix_values() {
    #[derive(IntoRusixValue)]
    struct BuilderArguments {
        endpoint: Endpoint,
        items: Vec<Endpoint>,
        package: PackageRef,
        #[rusix(rename = "literal.dot")]
        literal_key: bool,
    }

    #[derive(IntoConfig)]
    struct Contribution {
        payload: NixValue,
    }

    let arguments = BuilderArguments {
        endpoint: Endpoint {
            host: "service.internal".into(),
            port: Port(443),
        },
        items: vec![Endpoint {
            host: "other.internal".into(),
            port: Port(80),
        }],
        package: Nixpkgs::new().get("hello"),
        literal_key: true,
    }
    .try_into_nix_value()
    .unwrap();
    let conversion_line = line!() - 2;
    let config = Contribution {
        payload: arguments.clone(),
    }
    .into_config();
    assert_eq!(config.assignments.len(), 1);
    assert_eq!(config.assignments[0].path_segments(), &["payload"]);
    assert!(matches!(
        config.assignments[0].value.kind,
        ValueKind::OpaqueRecord(_)
    ));
    let ValueKind::OpaqueRecord(fields) = &config.assignments[0].value.kind else {
        unreachable!()
    };
    let endpoint = &fields
        .iter()
        .find(|(name, _)| name == "endpoint")
        .unwrap()
        .1;
    assert_eq!(endpoint.origin.file, file!());
    assert_eq!(endpoint.origin.line, conversion_line);
    assert_eq!(endpoint.origin.purpose, "record at endpoint");
    let ValueKind::OpaqueRecord(fields) = &endpoint.kind else {
        unreachable!()
    };
    let port = &fields.iter().find(|(name, _)| name == "port").unwrap().1;
    assert_eq!(port.origin.line, conversion_line);
    assert_eq!(port.origin.purpose, "value of endpoint.port");

    // An ordinary Nix function receives the complete record; package references
    // remain actual objects, and lists of structural records remain nested values.
    let received = Nixpkgs::new()
        .function("id")
        .call(arguments.clone())
        .select("package.pname");
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", received)).unwrap())
        .unwrap()
        .value;
    assert_eq!(value["result"], "hello");

    let value = NixSession::new()
        .unwrap()
        .evaluate(&compile(Config::new().set_dynamic("result", arguments.select("items"))).unwrap())
        .unwrap()
        .value;
    assert_eq!(
        value["result"],
        serde_json::json!([{"host": "other.internal", "port": 80}])
    );
}

#[test]
fn structural_to_opaque_conversion_preserves_laziness_and_child_error_origin() {
    #[derive(IntoRusixValue)]
    struct Deferred {
        good: String,
        bad: Expr<i64>,
    }

    #[derive(IntoRusixValue)]
    struct Nested {
        deferred: Deferred,
    }

    let operation_line = line!() + 1;
    let bad = Expr::int(44).divide(Expr::int(0));
    let value = Nested {
        deferred: Deferred {
            good: "unused bad field stays lazy".into(),
            bad,
        },
    }
    .try_into_nix_value()
    .unwrap();
    let artifact = compile(
        Config::new()
            .set_dynamic("good", value.clone().select("deferred.good"))
            .set_dynamic("bad", value.select("deferred.bad")),
    )
    .unwrap();
    let session = NixSession::new().unwrap();

    assert_eq!(
        session.evaluate_attribute(&artifact, "good").unwrap().value,
        "unused bad field stays lazy"
    );
    let diagnostic = session.evaluate_attribute(&artifact, "bad").unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, operation_line);
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
    assert!(diagnostic.reason.contains("division by zero"));
}

#[test]
fn structural_to_opaque_conversion_retains_flatten_validation() {
    #[derive(IntoRusixValue)]
    struct Invalid {
        #[rusix(flatten)]
        not_a_record: bool,
    }

    let error = Invalid { not_a_record: true }
        .try_into_nix_value()
        .unwrap_err();
    assert_eq!(error.message, "rusix flatten requires a record value");

    #[derive(IntoRusixValue)]
    struct Flattened {
        #[rusix(flatten)]
        endpoint: Endpoint,
    }

    let value = Flattened {
        endpoint: Endpoint {
            host: "flattened.internal".into(),
            port: Port(5432),
        },
    }
    .try_into_nix_value()
    .unwrap();
    let evaluated = NixSession::new()
        .unwrap()
        .evaluate(&compile(Config::new().set_dynamic("result", value)).unwrap())
        .unwrap()
        .value;
    assert_eq!(
        evaluated["result"],
        serde_json::json!({"host": "flattened.internal", "port": 5432})
    );
}

#[test]
fn direct_value_conversion_composes_naming_nulls_omission_and_dynamic_values() {
    #[derive(IntoRusixValue)]
    enum Mode {
        ReadOnly,
        #[rusix(rename = "client-only")]
        Client,
    }

    #[derive(IntoRusixValue)]
    #[rusix(rename_all = "PascalCase", omit_none)]
    struct Arguments {
        nested_value: Endpoint,
        modes: Vec<Mode>,
        nullable: NixValue,
        included: Option<String>,
        omitted: Option<String>,
        #[rusix(rename = "literal.dot")]
        dynamic: NixValue,
        symbolic: Expr<i64>,
    }

    let dynamic: std::collections::BTreeMap<String, NixValue> = [
        ("quoted\"key".into(), true.into()),
        ("optional".into(), None::<String>.into()),
    ]
    .into();
    let value = Arguments {
        nested_value: Endpoint {
            host: "nested.internal".into(),
            port: Port(80),
        },
        modes: vec![Mode::ReadOnly, Mode::Client],
        nullable: None::<String>.try_into_nix_value().unwrap(),
        included: Some("present".into()),
        omitted: None,
        dynamic: dynamic.into(),
        symbolic: Expr::int(84).divide(Expr::int(2)),
    }
    .try_into_nix_value()
    .unwrap();
    let result = NixSession::new()
        .unwrap()
        .evaluate(&compile(Config::new().set_dynamic("result", value)).unwrap())
        .unwrap()
        .value;
    assert_eq!(
        result["result"],
        serde_json::json!({
            "NestedValue": {"host": "nested.internal", "port": 80},
            "Modes": ["readOnly", "client-only"],
            "Nullable": null,
            "Included": "present",
            "literal.dot": {"quoted\"key": true, "optional": null},
            "Symbolic": 42,
        })
    );
}

#[test]
fn derived_flatten_can_fail_based_on_the_value_or_a_nested_conversion() {
    #[derive(IntoRusixValue)]
    struct Flatten<T> {
        #[rusix(flatten)]
        value: T,
    }

    #[derive(IntoRusixValue)]
    struct Nested {
        items: Vec<Flatten<bool>>,
    }

    let present = Flatten {
        value: Some(Endpoint {
            host: "present.internal".into(),
            port: Port(80),
        }),
    }
    .try_into_nix_value();
    assert!(present.is_ok());

    let absent = Flatten {
        value: None::<Endpoint>,
    }
    .try_into_nix_value()
    .unwrap_err();
    assert_eq!(absent.message, "rusix flatten requires a record value");

    // An opaque Nix record is a value, not a Rust structural record that can
    // be expanded into configuration paths before Nix evaluation.
    let opaque = NixValue::record([("valid", true.into())]);
    let opaque_line = line!() - 1;
    let error = Flatten { value: opaque }.try_into_nix_value().unwrap_err();
    assert_eq!(error.message, absent.message);
    assert_eq!(error.origin.line, opaque_line);

    let conversion_line = line!() + 4;
    let error = Nested {
        items: vec![Flatten { value: true }],
    }
    .try_into_nix_value()
    .unwrap_err();
    assert_eq!(error.message, absent.message);
    assert_eq!(error.origin.file, file!());
    assert_eq!(error.origin.line, conversion_line);
    assert_eq!(error.origin.purpose, "value of items.[0]");
}

#[test]
fn direct_conversion_keeps_the_same_structure_and_origins_as_the_low_level_path() {
    struct Custom(RusixValue);

    impl IntoRusixValue for Custom {
        fn into_value(self) -> RusixValue {
            self.0
        }
    }

    let structural = RusixValue::record([(
        "nested",
        RusixValue::record([("port", 443_i64.into_value())]),
    )]);
    let direct = Custom(structural.clone()).try_into_nix_value().unwrap();
    let legacy = structural.into_nix_value().unwrap();
    assert_eq!(
        format!(
            "{:?}",
            Config::new().set_dynamic("value", direct).assignments[0].value
        ),
        format!(
            "{:?}",
            Config::new().set_dynamic("value", legacy).assignments[0].value
        )
    );

    // Invalid manual flattening returns the same error through either entry point.
    let invalid = RusixValue::__record(vec![(None, true.into_value())]);
    let direct = Custom(invalid.clone()).try_into_nix_value().unwrap_err();
    let legacy = invalid.into_nix_value().unwrap_err();
    assert_eq!(direct.origin, legacy.origin);
    assert_eq!(direct.message, legacy.message);
}

#[test]
fn value_conversion_does_not_replace_compilation_validation() {
    let invalid = [
        RusixValue::record([
            ("duplicate", true.into_value()),
            ("duplicate", false.into_value()),
        ]),
        RusixValue::record([("nul", "invalid\0string".into_value())]),
        RusixValue::record([("float", f64::NAN.into_value())]),
    ];

    for structural in invalid {
        // No flatten error occurs here. These values are rejected when compiled,
        // just like records constructed directly with NixValue::record.
        let value = structural.into_nix_value().unwrap();
        let error = compile(Config::new().set_dynamic("result", value)).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::Validation);
        assert_eq!(error.primary.unwrap().file, file!());
    }
}

#[test]
fn omit_none_is_local_and_ordinary_options_remain_nullable_values() {
    #[derive(IntoConfig)]
    #[rusix(omit_none, rename_all = "PascalCase")]
    struct Root<T> {
        included: Option<T>,
        omitted: Option<String>,
        #[rusix(rename = "enableJIT")]
        enabled: std::option::Option<bool>,
        nested: Nested,
        empty: Vec<String>,
    }

    #[derive(IntoRusixValue)]
    struct Nested {
        ordinary: Option<String>,
        #[rusix(omit_none, rename = "literal.dot")]
        exceptional: Option<i64>,
        #[rusix(omit_none)]
        absent: core::option::Option<i64>,
        #[rusix(omit_none)]
        explicit_null: Option<Option<i64>>,
    }

    let conversion_line = line!() + 1;
    let config = Root {
        included: Some(0_i64),
        omitted: None,
        enabled: Some(false),
        nested: Nested {
            ordinary: None,
            exceptional: Some(42),
            absent: None,
            explicit_null: Some(None),
        },
        empty: vec![],
    }
    .into_config();
    let actual_conversion_line = line!() - 1;
    assert!(actual_conversion_line > conversion_line);

    let artifact = compile(&config).unwrap();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&artifact)
            .unwrap()
            .value,
        serde_json::json!({
            "Included": 0, "enableJIT": false, "Empty": [],
            "Nested": {"ordinary": null, "literal.dot": 42, "explicitNull": null},
        })
    );
    assert_eq!(config.assignments.len(), 6);

    for assignment in &config.assignments {
        assert_eq!(assignment.origin.line, actual_conversion_line);
        assert!(!assignment.path.contains("Omitted"));
        assert!(!assignment.path.contains("absent"));
    }
    for span in &artifact.spans {
        for origin in std::iter::once(&span.origin).chain(&span.enclosing) {
            assert!(!origin.purpose.contains("Omitted"));
            assert!(!origin.purpose.contains("absent"));
        }
    }
}

#[test]
fn a_fully_omitted_root_has_no_definitions_or_field_source_spans() {
    #[derive(IntoConfig)]
    struct Root {
        #[rusix(omit_none)]
        unused: Option<Expr<i64>>,
    }

    let config = Root { unused: None }.into_config();
    assert!(config.assignments.is_empty());
    let artifact = compile(&config).unwrap();
    assert!(!artifact.source.contains("unused"));
    assert!(artifact.spans.iter().all(|span| {
        !span.origin.purpose.contains("unused")
            && span.enclosing.iter().all(|o| !o.purpose.contains("unused"))
    }));
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&artifact)
            .unwrap()
            .value,
        serde_json::json!({})
    );
}

#[test]
fn emitted_optional_expressions_and_packages_keep_child_origins_and_laziness() {
    #[rusix::config]
    mod config {
        use super::{Expr, PackageRef};

        #[rusix(root, omit_none)]
        pub struct Root {
            pub good: Option<String>,
            pub bad: Option<Expr<i64>>,
            pub package: Option<PackageRef>,
        }
    }

    let operation_line = line!() + 1;
    let bad = Expr::int(44).divide(Expr::int(0));
    let lookup_line = line!() + 1;
    let missing = Nixpkgs::new().get("rusixDefinitelyMissing");
    let artifact = compile(
        config::Root {
            good: Some("only this field is selected".into()),
            bad: Some(bad),
            package: Some(missing.clone()),
        }
        .into_config(),
    )
    .unwrap();
    let session = NixSession::new().unwrap();

    assert_eq!(
        session.evaluate_attribute(&artifact, "good").unwrap().value,
        "only this field is selected"
    );
    let diagnostic = session.evaluate_attribute(&artifact, "bad").unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, operation_line);
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
    assert!(diagnostic.reason.contains("division by zero"));

    let package = compile(
        config::Root {
            good: None,
            bad: None,
            package: Some(missing),
        }
        .into_config(),
    )
    .unwrap();
    let diagnostic = session.evaluate_interop(&package).unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, lookup_line);
    assert!(diagnostic.reason.contains("rusixDefinitelyMissing"));
    assert!(!diagnostic.raw_nix.is_empty());
}

#[test]
fn omitted_definitions_keep_nixos_defaults_while_explicit_null_changes_them() {
    #[rusix::config]
    mod config {
        #[rusix(root)]
        pub struct Root {
            pub services: Services,
        }

        pub struct Services {
            pub example: Example,
        }

        pub struct Example {
            #[rusix(omit_none)]
            pub port: Option<i64>,
            #[rusix(omit_none)]
            pub label: Option<String>,
        }

        #[rusix(root)]
        pub struct NullableRoot {
            pub services: NullableServices,
        }

        pub struct NullableServices {
            pub example: NullableExample,
        }

        pub struct NullableExample {
            pub label: Option<String>,
        }
    }

    let fixture = InputRef::local(
        "omission",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/omit-none.nix"),
    );
    let module = NixosModule::empty()
        .import_ref(fixture.module("schema"))
        .add(config::Root {
            services: config::Services {
                example: config::Example {
                    port: None,
                    label: None,
                },
            },
        });
    let session = NixSession::new().unwrap();
    let artifact = compile_module(&module).unwrap();
    let selection = &["services", "example"];
    assert_eq!(
        session
            .evaluate_nixos(&artifact, selection, false)
            .unwrap()
            .value,
        serde_json::json!({"port": 5432, "label": "fallback"})
    );

    let explicit_null = module.clone().add(config::NullableRoot {
        services: config::NullableServices {
            example: config::NullableExample { label: None },
        },
    });
    assert_eq!(
        session
            .evaluate_nixos(&compile_module(&explicit_null).unwrap(), selection, false)
            .unwrap()
            .value,
        serde_json::json!({"port": 5432, "label": null})
    );
    let invalid = module.add(Config::new().set_dynamic("services.example.port", NixValue::null()));
    let diagnostic = session
        .evaluate_nixos(&compile_module(&invalid).unwrap(), selection, false)
        .unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixosType);
    assert!(diagnostic.reason.contains("services.example.port"));
}
