#![cfg(feature = "evaluation")]

//! Inline module sugar uses the same traits, IR, and evaluator as explicit derives.
use rusix::interop::raw::NixFunctionExt;
use rusix::ir::ReferencedExpression;
use rusix::{DiagnosticKind, NixSession, Provenance, compile, nixos::compile_module};
use rusix::{
    Expr, IntoConfig, IntoRusixValue,
    interop::{InputRef, Nixpkgs},
    ir::ValueKind,
    nixos::{DefinitionPriority, NixosModule},
};

// A reusable type defined outside the authoring boundary.
#[derive(IntoRusixValue)]
struct Endpoint<'a> {
    host_name: &'a str,
    listen_port: u16,
}

#[rusix::config]
mod local {
    use super::Endpoint;
    use rusix::interop::InputRef;

    #[rusix(root)]
    pub struct Machine<'a> {
        pub services: Services<'a>,
        #[rusix(flatten)]
        pub metadata: Metadata,
        #[rusix(skip)]
        pub input: InputRef,
    }

    pub struct Services<'a> {
        pub example_service: Endpoint<'a>,
    }

    #[rusix(rename_all = "PascalCase")]
    pub struct Metadata {
        pub display_name: String,
        #[rusix(rename = "literal.name")]
        pub unusual_name: bool,
    }
}

#[test]
fn local_nesting_reuses_external_types_and_existing_mapping_rules() {
    let model = local::Machine {
        services: local::Services {
            example_service: Endpoint {
                host_name: "local",
                listen_port: 8080,
            },
        },
        metadata: local::Metadata {
            display_name: "demo".into(),
            unusual_name: true,
        },
        input: InputRef::local("unused", "not-read.nix"),
    };
    let _ = &model.input;
    let config = model.into_config();

    let value = NixSession::new()
        .unwrap()
        .evaluate(&compile(&config).unwrap())
        .unwrap()
        .value;
    assert_eq!(
        value,
        serde_json::json!({
            "services": {"exampleService": {"hostName": "local", "listenPort": 8080}},
            "DisplayName": "demo", "literal.name": true,
        })
    );
    // The fine-grained API remains available for the same reusable value.
    #[derive(IntoConfig)]
    struct Explicit<'a> {
        services: ExplicitServices<'a>,
    }

    #[derive(IntoRusixValue)]
    struct ExplicitServices<'a> {
        example_service: Endpoint<'a>,
    }

    let explicit = Explicit {
        services: ExplicitServices {
            example_service: Endpoint {
                host_name: "local",
                listen_port: 8080,
            },
        },
    }
    .into_config();
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&compile(&explicit).unwrap())
            .unwrap()
            .value["services"],
        value["services"]
    );
}

#[rusix::config]
mod custom {
    use rusix::{IntoRusixValue, RusixValue};

    pub enum Mode {
        Server,
        Client,
    }

    // Explicit control must not produce duplicate implementations.
    pub struct Count(pub i64);

    impl IntoRusixValue for Count {
        #[track_caller]
        fn into_value(self) -> RusixValue {
            (self.0 + 1).into_value()
        }
    }

    #[rusix(root)]
    pub struct Machine {
        pub mode: Mode,
        pub workers: Count,
    }

    #[rusix(root, rename_all = "PascalCase")]
    #[derive(IntoRusixValue)]
    #[cfg_attr(all(), derive(Debug))]
    pub struct AlreadyValue {
        pub listen_port: u16,
    }

    #[rusix(root)]
    #[derive(rusix::IntoConfig)]
    pub struct AlreadyConfig {
        pub enable: bool,
    }

    #[rusix(root)]
    #[derive(IntoRusixValue)]
    #[cfg_attr(all(), cfg(any()))]
    struct DisabledRoot {
        unused: bool,
    }

    #[rusix(root)]
    pub struct ManualRoot {
        pub value: i64,
    }

    impl IntoRusixValue for ManualRoot {
        #[track_caller]
        fn into_value(self) -> RusixValue {
            RusixValue::record([("manualValue", self.value.into_value())])
        }
    }
}

#[test]
fn local_unit_enums_lower_automatically_alongside_custom_conversions() {
    for (mode, expected) in [
        (custom::Mode::Server, "server"),
        (custom::Mode::Client, "client"),
    ] {
        let config = custom::Machine {
            mode,
            workers: custom::Count(2),
        }
        .into_config();
        let value = NixSession::new()
            .unwrap()
            .evaluate(&compile(&config).unwrap())
            .unwrap()
            .value;
        assert_eq!(value, serde_json::json!({"mode": expected, "workers": 3}));
    }
}

#[test]
fn explicit_value_config_and_manual_root_conversions_do_not_conflict() {
    for (config, expected) in [
        (
            custom::AlreadyValue { listen_port: 443 }.into_config(),
            serde_json::json!({"ListenPort":443}),
        ),
        (
            custom::AlreadyConfig { enable: true }.into_config(),
            serde_json::json!({"enable":true}),
        ),
        (
            custom::ManualRoot { value: 42 }.into_config(),
            serde_json::json!({"manualValue":42}),
        ),
    ] {
        assert_eq!(
            NixSession::new()
                .unwrap()
                .evaluate(&compile(&config).unwrap())
                .unwrap()
                .value,
            expected
        );
    }
}

#[rusix::config]
mod ssh_models {
    #[rusix(root)]
    pub struct Contribution<T> {
        pub services: Services<T>,
    }

    pub struct Services<T> {
        pub openssh: T,
    }

    pub struct Ports {
        pub ports: Vec<u16>,
    }

    pub struct User {
        pub authorized_keys_command_user: String,
    }
}

fn ports(values: Vec<u16>) -> ssh_models::Contribution<ssh_models::Ports> {
    ssh_models::Contribution {
        services: ssh_models::Services {
            openssh: ssh_models::Ports { ports: values },
        },
    }
}

fn user(value: &str) -> ssh_models::Contribution<ssh_models::User> {
    ssh_models::Contribution {
        services: ssh_models::Services {
            openssh: ssh_models::User {
                authorized_keys_command_user: value.into(),
            },
        },
    }
}

#[test]
fn module_components_keep_nixos_merge_and_priority_selection() {
    let module = NixosModule::empty()
        .import("nixos/modules/services/networking/ssh/sshd.nix")
        .add(ports(vec![22]))
        .add(ports(vec![2222]));
    assert_eq!(module.modules.len(), 2);

    let session = NixSession::new().unwrap();
    let selection = &["services", "openssh", "ports"];
    let value = session
        .evaluate_nixos(&compile_module(&module).unwrap(), selection, false)
        .unwrap()
        .value;
    let mut merged: Vec<_> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_i64().unwrap())
        .collect();
    merged.sort();
    assert_eq!(merged, vec![22, 2222]);

    let module = module.module(
        NixosModule::new(ports(vec![3333]).into_config()).priority(DefinitionPriority::Force),
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
fn conflicting_module_components_report_both_authoring_calls() {
    let first = line!() + 1;
    let module = NixosModule::empty().add(user("root"));
    let second = line!() + 1;
    let module = module.add(user("nobody"));
    let module = module.import("nixos/modules/services/networking/ssh/sshd.nix");

    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_nixos(
            &compile_module(&module).unwrap(),
            &["services", "openssh", "authorizedKeysCommandUser"],
            false,
        )
        .unwrap_err();
    assert_eq!(diagnostic.kind, DiagnosticKind::NixosMerge);
    assert_eq!(
        diagnostic.option_path.as_deref(),
        Some("services.openssh.authorizedKeysCommandUser")
    );
    let lines: Vec<_> = diagnostic
        .origins
        .iter()
        .filter_map(|o| o.origin.as_ref())
        .map(|o| o.line)
        .collect();
    assert!(
        lines.contains(&first) && lines.contains(&second),
        "{diagnostic:?}"
    );
}

#[rusix::config]
mod expressions {
    use rusix::Expr;

    #[rusix(root)]
    pub struct Root {
        pub good: i64,
        pub bad: Vec<Expr<i64>>,
    }
}

#[test]
fn unused_values_stay_lazy_and_operation_origins_survive_the_module() {
    let operation_line = line!() + 1;
    let bad = Expr::int(44).divide(Expr::int(0));

    let generated = compile(
        expressions::Root {
            good: 42,
            bad: vec![bad],
        }
        .into_config(),
    )
    .unwrap();

    let session = NixSession::new().unwrap();
    assert!(!generated.source.contains("deepSeq"));
    assert_eq!(
        session
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        42
    );

    let diagnostic = session.evaluate_attribute(&generated, "bad").unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, operation_line);
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert!(diagnostic.related.iter().any(|o| o.purpose == "set bad"));
    assert_eq!(diagnostic.provenance, Provenance::SourceMap);
}

#[rusix::config]
mod handles {
    use rusix::{
        Expr,
        interop::{InputRef, ModuleRef, NixFunction, OverlayRef, PackageRef, raw::NixValue},
        nixos::OptionRef,
    };

    #[rusix(root)]
    pub struct Root {
        pub package: PackageRef,
        pub module: ModuleRef,
        pub overlay: OverlayRef,
        pub function: NixFunction,
        pub value: NixValue,
        pub port: OptionRef<i64>,
        pub command: Expr<String>,
        #[rusix(skip)]
        pub input: InputRef,
    }
}

#[test]
fn symbolic_and_opaque_values_remain_native_ir_objects() {
    let pkgs = Nixpkgs::new();
    let input = InputRef::local("example", "not-read.nix");
    let package = pkgs.get("hello");
    let origin = package.reference().origin.clone();
    let model = handles::Root {
        package,
        module: pkgs.module("misc/label.nix"),
        overlay: input.overlay("overlays.example"),
        function: pkgs.function("toUpper"),
        value: pkgs.function("toUpper").call("rusix"),
        port: rusix::nixos::OptionRef::<i64>::new("services.example.port"),
        command: rusix::nixos::OptionRef::<i64>::new("services.example.port")
            .into_expr()
            .to_text(),
        input,
    };
    let _ = &model.input;
    let config = model.into_config();
    assert_eq!(config.assignments.len(), 7);
    assert_eq!(config.assignments[0].value.origin, origin);
    assert!(
        config.assignments[..4]
            .iter()
            .all(|a| matches!(a.value.kind, ValueKind::Reference(_)))
    );
    assert!(matches!(
        config.assignments[4].value.kind,
        ValueKind::Apply(..)
    ));
    assert!(matches!(
        config.assignments[5].value.kind,
        ValueKind::OptionReference(_)
    ));
    assert!(matches!(
        config.assignments[6].value.kind,
        ValueKind::ToText(_)
    ));
    compile_module(&NixosModule::empty().add(config)).unwrap();
}

#[rusix::config]
mod packages {
    use rusix::interop::PackageRef;

    #[rusix(root)]
    pub struct Root {
        pub environment: Environment,
    }

    pub struct Environment {
        pub system_packages: Vec<PackageRef>,
    }
}

#[test]
fn opaque_packages_in_automatic_structs_resolve_through_real_nixpkgs() {
    let module = NixosModule::empty()
        .import_ref(Nixpkgs::new().module("config/system-path.nix"))
        .module(
            NixosModule::new(
                packages::Root {
                    environment: packages::Environment {
                        system_packages: vec![Nixpkgs::new().get("hello")],
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
fn local_enum_derives_and_semantic_overrides_use_the_same_traits() {
    #[rusix::config]
    mod config {
        use rusix::{IntoRusixValue, RusixValue};

        #[rusix(rename_all = "PascalCase")]
        pub enum Mode {
            ReadOnly,
            #[rusix(rename = "read-write")]
            ReadWrite,
        }

        // Explicit reusable derives inside the boundary must not be duplicated.
        #[derive(IntoRusixValue)]
        pub enum State {
            Ready,
        }

        pub enum Permission {
            Deny,
            Allow,
        }

        impl IntoRusixValue for Permission {
            #[track_caller]
            fn into_value(self) -> RusixValue {
                // This is a boolean policy decision, not an enum label.
                matches!(self, Self::Allow).into_value()
            }
        }

        #[rusix(root)]
        pub struct Root {
            pub modes: Vec<Mode>,
            pub state: State,
            pub permissions: Vec<Permission>,
        }
    }

    let model = config::Root {
        modes: vec![config::Mode::ReadOnly, config::Mode::ReadWrite],
        state: config::State::Ready,
        permissions: vec![config::Permission::Deny, config::Permission::Allow],
    };
    assert_eq!(
        NixSession::new()
            .unwrap()
            .evaluate(&compile(model.into_config()).unwrap())
            .unwrap()
            .value,
        serde_json::json!({"modes":["ReadOnly","read-write"],"state":"ready","permissions":[false,true]})
    );
}
