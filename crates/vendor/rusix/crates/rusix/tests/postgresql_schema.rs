#![cfg(feature = "evaluation")]

//! Compare the actual public declarations and ordinary Nix consumers with pinned upstream.
use rusix::{Generated, NixSession, nixos::compile_module};
use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

#[allow(dead_code)]
#[path = "../../../examples/postgresql-nixos-module/main.rs"]
mod example;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn session() -> &'static Mutex<NixSession> {
    static SESSION: OnceLock<Mutex<NixSession>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(NixSession::new().unwrap()))
}

fn compare(case: &str, mode: &str, accepts: bool) -> Option<serde_json::Value> {
    let session = session().lock().unwrap_or_else(|p| p.into_inner());
    let module = example::schema::module().module(example::lowering::implementation());
    let artifact = compile_module(&module).unwrap();
    fs::copy(
        root().join("tests/fixtures/postgresql-schema.nix"),
        session.root().join("schema-driver.nix"),
    )
    .unwrap();

    let evaluate = |rewritten| {
        session.evaluate_nixos_with_driver(&artifact, &Generated {
        source: format!("import ./schema-driver.nix {{ nixpkgs = ./nixpkgs-full; generated = ./module.nix; rewritten = {rewritten}; caseName = {}; mode = {}; }}", serde_json::to_string(case).unwrap(), serde_json::to_string(mode).unwrap()),
        ..Generated::default()
    })
    };
    let upstream = evaluate(false);
    let candidate = evaluate(true);
    let output = root()
        .join("target/postgresql-schema")
        .join(mode)
        .join(case);
    fs::create_dir_all(&output).unwrap();

    if accepts {
        let upstream = upstream
            .unwrap_or_else(|e| panic!("upstream {mode}/{case}: {}\n{}", e.reason, e.raw_nix))
            .value;
        let candidate = candidate
            .unwrap_or_else(|e| panic!("candidate {mode}/{case}: {}\n{}", e.reason, e.raw_nix))
            .value;
        fs::write(
            output.join("upstream.json"),
            serde_json::to_string_pretty(&upstream).unwrap(),
        )
        .unwrap();
        fs::write(
            output.join("rusix.json"),
            serde_json::to_string_pretty(&candidate).unwrap(),
        )
        .unwrap();
        assert_eq!(
            upstream,
            candidate,
            "{mode}/{case}; see {}",
            output.display()
        );
        Some(candidate)
    } else {
        let upstream = upstream.unwrap_err();
        let candidate = candidate.unwrap_err();
        fs::write(output.join("upstream.stderr"), &upstream.raw_nix).unwrap();
        fs::write(output.join("rusix.stderr"), &candidate.raw_nix).unwrap();
        assert_eq!(
            upstream.kind, candidate.kind,
            "{case}: {}\n{}",
            upstream.reason, candidate.reason
        );
        assert_eq!(
            upstream.option_path, candidate.option_path,
            "{case}: affected option differs"
        );
        assert_eq!(
            upstream.reason.lines().next(),
            candidate.reason.lines().next(),
            "{case}: underlying NixOS rejection differs"
        );
        assert!(
            candidate.primary.is_none(),
            "foreign invalid definition must not be blamed on schema Rust: {candidate:?}"
        );
        assert!(!candidate.raw_nix.is_empty());
        None
    }
}

#[test]
fn metadata_types_docs_and_nested_declarations_match() {
    let metadata = compare("disabled", "metadata", true).unwrap();
    compare("enabled", "metadata", true);
    fn count(value: &serde_json::Value) -> usize {
        value
            .as_object()
            .unwrap()
            .values()
            .map(|v| {
                if v.get("type").is_some() {
                    1 + count(&v["nested"])
                } else {
                    count(v)
                }
            })
            .sum()
    }
    assert_eq!(metadata.as_object().unwrap().len(), 19); // 16 options plus three migrations.
    assert_eq!(count(&metadata), 32); // 29 ordinary/nested declarations plus three migrations.
}

#[test]
fn evaluated_defaults_match_across_state_versions_and_enablement() {
    for case in [
        "disabled",
        "enabled",
        "state_21",
        "state_22",
        "state_23",
        "jit_enabled",
        "jit_disabled",
    ] {
        compare(case, "defaults", true);
    }
}

#[test]
fn ordinary_nix_valid_definitions_merges_coercions_and_aliases_match() {
    for case in [
        "disabled",
        "enabled",
        "bool_false",
        "package_valid",
        "port_valid",
        "path_valid",
        "nullable_null",
        "nullable_file",
        "recovery_lines",
        "database_list_merge",
        "initdb_list_merge",
        "settings_merge",
        "settings_mixed",
        "preload_list",
        "preload_null",
        "users_valid",
        "users_list_merge",
        "plugins_function",
        "plugins_list",
        "priorities",
        "alias_port",
        "alias_prefix",
        "ordinary_consumer",
    ] {
        compare(case, "values", true);
    }
}

#[test]
fn invalid_ordinary_nix_definitions_fail_without_false_rust_blame() {
    for case in [
        "bool_invalid",
        "package_invalid",
        "port_invalid",
        "port_null",
        "path_invalid",
        "nullable_invalid",
        "database_list_invalid",
        "settings_null_invalid",
        "settings_nested_invalid",
        "preload_invalid",
        "users_missing_name",
        "users_unknown_field",
        "users_invalid_clause",
        "users_invalid_ownership",
        "users_invalid_shape",
        "plugins_invalid_list",
        "plugins_invalid_result",
        "superuser_readonly",
        "removed",
    ] {
        compare(case, "values", false);
    }
}
