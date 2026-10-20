#![cfg(feature = "evaluation")]

//! Compare the Rust implementation with real pinned NixOS, without building outputs.
use rusix::{Generated, NixSession, nixos::compile_module};
use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

#[allow(dead_code)]
#[path = "../../../examples/postgresql-nixos-module/main.rs"]
mod example;

fn candidate() -> rusix::nixos::NixosModule {
    example::schema::module().module(example::lowering::implementation())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn session() -> &'static Mutex<NixSession> {
    static SESSION: OnceLock<Mutex<NixSession>> = OnceLock::new();

    SESSION.get_or_init(|| Mutex::new(NixSession::new().unwrap()))
}

fn compare(case: &str) {
    let session = session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let module = if case == "rust_model" {
        candidate().add(example::model::model())
    } else {
        candidate()
    };

    let artifact = compile_module(&module).unwrap();

    fs::copy(
        root().join("tests/fixtures/postgresql-equivalence.nix"),
        session.root().join("postgresql-driver.nix"),
    )
    .unwrap();

    let evaluate = |rewritten| {
        session.evaluate_nixos_with_driver(&artifact, &Generated {
            source: format!("import ./postgresql-driver.nix {{ nixpkgs = ./nixpkgs-full; generated = ./module.nix; caseName = {}; rewritten = {rewritten}; }}", serde_json::to_string(case).unwrap()),
            ..Generated::default()
        }).unwrap_or_else(|error| panic!("{case} rewritten={rewritten}: {}\n{}", error.reason, error.raw_nix)).value
    };

    let upstream = evaluate(false);
    let rewritten = evaluate(true);

    let out = root().join("target/postgresql").join(case);
    fs::create_dir_all(&out).unwrap();
    fs::write(
        out.join("upstream.json"),
        serde_json::to_string_pretty(&upstream).unwrap(),
    )
    .unwrap();
    fs::write(
        out.join("rusix.json"),
        serde_json::to_string_pretty(&rewritten).unwrap(),
    )
    .unwrap();
    fs::write(out.join("module.nix"), &artifact.module.source).unwrap();

    assert!(
        upstream == rewritten,
        "{case}: semantic projection differs; see {}",
        out.display()
    );
}

#[test]
fn minimal_configuration_is_equivalent() {
    compare("minimal");
}

#[test]
fn disabled_configuration_is_equivalent() {
    compare("disabled");
}

#[test]
fn recovery_link_formatting_keeps_shell_arguments_and_nix_context() {
    let session = session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let artifact = compile_module(&candidate()).unwrap();
    fs::copy(
        root().join("tests/fixtures/postgresql-equivalence.nix"),
        session.root().join("postgresql-driver.nix"),
    )
    .unwrap();

    let evaluate = |rewritten| {
        session.evaluate_nixos_with_driver(&artifact, &Generated {
            source: format!("import ./postgresql-driver.nix {{ nixpkgs = ./nixpkgs-full; generated = ./module.nix; caseName = \"recovery\"; rewritten = {rewritten}; normalizeRecovery = false; }}"),
            ..Generated::default()
        }).unwrap().value
    };
    let original = evaluate(false);
    let rewritten = evaluate(true);
    let before = &original["service"]["preStart"];
    let after = &rewritten["service"]["preStart"];

    fn arguments(script: &serde_json::Value) -> Vec<Vec<u8>> {
        // Execute only the final recovery command, with a shell function that
        // records argv instead of creating links or touching the filesystem.
        let (_, command) = script["text"]
            .as_str()
            .unwrap()
            .rsplit_once("\nln -sfn ")
            .unwrap();
        let output = std::process::Command::new("bash")
            .env_clear()
            .args(["--noprofile", "--norc", "-c"])
            .arg(format!(
                "ln() {{ printf '%s\\0' \"$@\"; }}\nln -sfn {command}"
            ))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|arg| !arg.is_empty())
            .map(<[u8]>::to_vec)
            .collect()
    }

    assert!(before["text"].as_str().unwrap().contains("\\\n  \""));
    assert!(!after["text"].as_str().unwrap().contains("\\\n  \""));
    assert_eq!(arguments(before), arguments(after));
    let arguments = arguments(after);
    assert_eq!(arguments.len(), 3);
    assert_eq!(arguments[0], b"-sfn");
    assert!(arguments[1].ends_with(b"-recovery.conf"));
    assert_eq!(arguments[2], b"/var/lib/postgresql/16/recovery.conf");
    assert_eq!(before["context"], after["context"]);
}

macro_rules! equivalent {
    ($($case:ident),* $(,)?) => { $(
        #[test]
        fn $case() {
            compare(stringify!($case));
        }
    )* };
}

equivalent!(
    custom_package,
    jit_enabled,
    jit_disabled,
    empty_extensions,
    extensions,
    extension_list,
    custom_data_dir,
    legacy_data_dir,
    basic_settings,
    mixed_settings,
    preload_libraries,
    authentication_addition,
    authentication_replacement,
    ident_map,
    custom_port,
    initdb_arguments,
    initial_script,
    recovery,
    databases,
    users,
    ownership,
    clauses_preserve,
    clauses_enable,
    clauses_disable,
    multiple_roles,
    invalid_ownership,
    tcpip,
    port_priorities,
    jit_setting_override,
    hardening_override,
    check_disabled,
    state_21,
    state_22,
    state_23,
    renamed_options,
    disabled_lazy,
    rust_model,
    cross_compiled,
    legacy_package_metadata,
);

fn evaluate(
    session: &NixSession,
    artifact: &rusix::nixos::NixosArtifact,
    case: &str,
    rewritten: bool,
    check_assertions: bool,
) -> Result<rusix::Evaluation, Box<rusix::Diagnostic>> {
    fs::copy(
        root().join("tests/fixtures/postgresql-equivalence.nix"),
        session.root().join("postgresql-driver.nix"),
    )
    .unwrap();

    session.evaluate_nixos_with_driver(artifact, &Generated {
        source: format!("import ./postgresql-driver.nix {{ nixpkgs = ./nixpkgs-full; generated = ./module.nix; caseName = {}; rewritten = {rewritten}; checkAssertions = {check_assertions}; downstream = ./downstream.nix; }}", serde_json::to_string(case).unwrap()),
        ..Generated::default()
    })
}

#[test]
fn ordinary_nix_overrides_change_six_output_classes_without_rust_relowering() {
    let session = session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let artifact = compile_module(&candidate()).unwrap();
    let original_source = artifact.module.source.clone();
    fs::write(session.root().join("downstream.nix"), "{}\n").unwrap();

    let upstream = evaluate(&session, &artifact, "downstream_base", false, true)
        .unwrap()
        .value;
    let base = evaluate(&session, &artifact, "downstream_base", true, true)
        .unwrap()
        .value;
    assert_eq!(upstream, base);

    fs::copy(
        root().join("tests/fixtures/postgresql-downstream.nix"),
        session.root().join("downstream.nix"),
    )
    .unwrap();

    let upstream = evaluate(&session, &artifact, "downstream_base", false, true)
        .unwrap()
        .value;
    let changed = evaluate(&session, &artifact, "downstream_base", true, true)
        .unwrap()
        .value;

    assert_eq!(upstream, changed);
    assert_eq!(changed["settings"]["port"], 6432);
    assert_eq!(changed["settings"]["max_connections"], 80);
    assert_eq!(
        changed["service"]["environment"]["PGDATA"]["text"],
        "/srv/downstream-postgresql"
    );
    assert!(
        changed["service"]["postStart"]["text"]
            .as_str()
            .unwrap()
            .contains("ALTER DATABASE \"downstream\" OWNER TO \"downstream\"")
    );
    assert!(
        changed["packageMetadata"]["version"]
            .as_str()
            .unwrap()
            .starts_with("15.")
    );
    assert_eq!(
        changed["authentication"]["text"],
        "local downstream downstream peer"
    );
    assert_ne!(base["generatedFiles"], changed["generatedFiles"]);
    assert_eq!(original_source, artifact.module.source);
    fs::write(session.root().join("downstream.nix"), "{}\n").unwrap();

    let out = root().join("target/postgresql/downstream");
    fs::create_dir_all(&out).unwrap();
    fs::write(
        out.join("base.json"),
        serde_json::to_string_pretty(&base).unwrap(),
    )
    .unwrap();
    fs::write(
        out.join("changed.json"),
        serde_json::to_string_pretty(&changed).unwrap(),
    )
    .unwrap();
}

macro_rules! rejected {
    ($($name:ident: $reason:literal),* $(,)?) => { $(
        #[test]
        fn $name() {
            let session = session().lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let artifact = compile_module(&candidate()).unwrap();
            fs::write(session.root().join("downstream.nix"), "{}\n").unwrap();

            let upstream = evaluate(&session, &artifact, stringify!($name), false, true).unwrap_err();
            let rewritten = evaluate(&session, &artifact, stringify!($name), true, true).unwrap_err();

            assert!(upstream.reason.contains($reason), "{}", upstream.reason);
            assert!(rewritten.reason.contains($reason), "{}", rewritten.reason);
            assert!(!rewritten.raw_nix.is_empty());

            let out = root().join("target/postgresql").join(stringify!($name));
            fs::create_dir_all(&out).unwrap();
            fs::write(out.join("diagnostic.txt"), rewritten.render(&root())).unwrap();
            fs::write(out.join("nix.stderr"), &rewritten.raw_nix).unwrap();
        }
    )* };
}

rejected!(
    removed_11: "postgresql_11 was removed",
    removed_96: "postgresql_9_6 was removed",
    removed_95: "postgresql_9_5 was removed",
    invalid_port: "services.postgresql.settings.port",
    invalid_setting: "services.postgresql.settings.max_connections",
    invalid_clause: "ensureClauses.login",
    invalid_null: "services.postgresql.settings.non_nullable",
    removed_option: "extraConfig",
);

#[test]
fn foreign_invalid_ownership_retains_the_nixos_assertion_reason() {
    let session = session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let artifact = compile_module(&candidate()).unwrap();
    fs::write(session.root().join("downstream.nix"), "{}\n").unwrap();

    for rewritten in [false, true] {
        let error =
            evaluate(&session, &artifact, "invalid_ownership", rewritten, true).unwrap_err();
        assert_eq!(error.kind, rusix::DiagnosticKind::NixosAssertion);
        assert!(
            error
                .reason
                .contains("Offender: orphan has not been found among databases")
        );
    }
}

#[test]
fn provisioning_contribution_conflicts_keep_both_rust_origins() {
    use example::model::Postgresql;

    let session = session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    fs::write(session.root().join("downstream.nix"), "{}\n").unwrap();

    let first_line = line!() + 1;
    let module = candidate().add(Postgresql {
        enable: true,
        data_dir: Some("/srv/one".into()),
        ..Postgresql::default()
    });

    let second_line = line!() + 1;
    let module = module.add(Postgresql {
        enable: true,
        data_dir: Some("/srv/two".into()),
        ..Postgresql::default()
    });

    let artifact = compile_module(&module).unwrap();
    let error = evaluate(&session, &artifact, "minimal", true, false).unwrap_err();
    assert_eq!(error.kind, rusix::DiagnosticKind::NixosMerge);
    assert_eq!(error.origins.len(), 2);
    assert_eq!(
        error.option_path.as_deref(),
        Some("services.postgresql.dataDir")
    );
    let lines: Vec<_> = error
        .origins
        .iter()
        .filter_map(|origin| origin.origin.as_ref().map(|origin| origin.line))
        .collect();
    assert!(lines.contains(&first_line));
    assert!(lines.contains(&second_line));
}

#[test]
fn symbolic_generated_file_errors_keep_the_rust_operation_origin() {
    use example::model::Postgresql;
    use rusix::Expr;

    let session = session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    fs::write(session.root().join("downstream.nix"), "{}\n").unwrap();
    let operation_line = line!() + 1;
    let failure = Expr::int(44).divide(Expr::int(0));
    let module = candidate().add(Postgresql {
        enable: true,
        settings: std::collections::BTreeMap::from([("max_connections".into(), failure.into())]),
        ..Postgresql::default()
    });

    let artifact = compile_module(&module).unwrap();
    let error = evaluate(&session, &artifact, "minimal", true, false).unwrap_err();
    assert_eq!(error.reason, "division by zero");
    assert_eq!(error.primary.as_ref().unwrap().line, operation_line);
    assert_eq!(error.primary.as_ref().unwrap().file, file!());
    assert!(!error.raw_nix.is_empty());
}

#[test]
fn extension_lookup_failures_retain_a_rust_boundary_origin() {
    let session = session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    fs::write(session.root().join("downstream.nix"), "{}\n").unwrap();
    let module = candidate().add(example::model::Postgresql {
        enable: true,
        extensions: vec!["rusixMissingExtension".into()],
        ..example::model::Postgresql::default()
    });

    let artifact = compile_module(&module).unwrap();
    let error = evaluate(&session, &artifact, "minimal", true, false).unwrap_err();
    assert!(error.reason.contains("rusixMissingExtension"));
    assert!(
        error
            .primary
            .as_ref()
            .unwrap()
            .file
            .ends_with("examples/postgresql-nixos-module/lowering.rs")
    );
    let lookup_line = include_str!("../../../examples/postgresql-nixos-module/lowering.rs")
        .lines()
        .position(|line| line.contains("packages.clone().select(&name)"))
        .unwrap() as u32
        + 1;
    assert_eq!(error.primary.as_ref().unwrap().line, lookup_line);
    assert!(!error.raw_nix.is_empty());
}

#[test]
fn typed_role_clauses_match_all_upstream_three_state_cases() {
    use example::model::{Clause, Postgresql, Role, RoleClauses};

    let session = session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    fs::write(session.root().join("downstream.nix"), "{}\n").unwrap();

    for case in ["clauses_preserve", "clauses_enable", "clauses_disable"] {
        let clause = || {
            Some(match case {
                "clauses_preserve" => Clause::Preserve,
                "clauses_enable" => Clause::Enable,
                _ => Clause::Disable,
            })
        };

        let module = candidate().add(Postgresql {
            enable: true,
            roles: vec![Role {
                name: "alice".into(),
                clauses: RoleClauses {
                    superuser: clause(),
                    createrole: clause(),
                    createdb: clause(),
                    inherit: clause(),
                    login: clause(),
                    replication: clause(),
                    bypassrls: clause(),
                },
            }],
            ..Postgresql::default()
        });
        let artifact = compile_module(&module).unwrap();
        let upstream = evaluate(&session, &artifact, case, false, false)
            .unwrap()
            .value;
        // Inputs come from the Rust model, not the matching Nix case fixture.
        let rewritten = evaluate(&session, &artifact, "minimal", true, false)
            .unwrap()
            .value;

        assert_eq!(upstream, rewritten, "typed role clauses: {case}");
    }
}

#[test]
fn unset_role_clauses_and_explicit_preserve_keep_distinct_definitions() {
    use example::model::{Clause, Postgresql, Role, RoleClauses};
    use rusix::IntoConfig;

    let input = Postgresql {
        roles: vec![
            Role {
                name: "unset".into(),
                clauses: RoleClauses::default(),
            },
            Role {
                name: "preserve".into(),
                clauses: RoleClauses {
                    login: Some(Clause::Preserve),
                    ..RoleClauses::default()
                },
            },
        ],
        ..Postgresql::default()
    }
    .into_config();

    let session = session()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let evaluated = session
        .evaluate(&rusix::compile(&input).unwrap())
        .unwrap()
        .value;

    let users = &evaluated["services"]["postgresql"]["ensureUsers"];
    assert_eq!(users[0]["ensureClauses"], serde_json::json!({}));
    assert_eq!(
        users[1]["ensureClauses"],
        serde_json::json!({"login": null})
    );
}
