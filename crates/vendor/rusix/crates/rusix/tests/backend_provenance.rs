#![cfg(feature = "evaluation")]

//! Evidence for the limits of lazy contexts at opaque backend argument handoffs.
#[path = "../../../examples/composed-packages/composition.rs"]
pub mod composition;

#[path = "support/packages.rs"]
mod support;

use rusix::{
    Config,
    interop::{
        InputRef, NixAttrs, NixExpression, Nixpkgs, Package,
        raw::{NixFunctionExt, NixRepresentation, NixValue, NixpkgsExt},
    },
};
use rusix::{Generated, NixSession};
use std::{fs, path::Path};

fn artifact_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/backend-provenance")
}

fn fixture(scenario: &str, placement: &str, payload: &str) -> Generated {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/backend-provenance.nix");
    Generated {
        source: format!(
            "import {} {{ pkgs = import ./nixpkgs-full {{ system = \"x86_64-linux\"; config = {{}}; }}; scenario = {scenario:?}; placement = {placement:?}; payload = {payload:?}; }}",
            serde_json::to_string(&path.to_string_lossy()).unwrap()
        ),
        ..Generated::default()
    }
}

// Only trace messages count: source excerpts can contain unused context labels.
fn trace_messages(raw: &str) -> Vec<String> {
    raw.lines()
        .filter_map(|line| line.strip_prefix("@nix "))
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .flat_map(|event| {
            event["trace"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|frame| frame["raw_msg"].as_str().map(str::to_owned))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn delayed_backend_context_matrix() {
    let session = NixSession::new().unwrap();
    let root = artifact_root().join("matrix");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("nix-version.txt"), session.version().unwrap()).unwrap();
    let mut observations = Vec::new();

    for scenario in [
        "dependency",
        "plain",
        "nested-list",
        "propagated",
        "copy",
        "lib-map",
        "lib-flatten",
        "reconstruct",
        "override",
        "overrideAttrs",
        "finalAttrs",
        "composed",
    ] {
        for payload in ["int", "attrs", "throw", "nested-throw"] {
            for placement in ["none", "field", "child", "call", "combined"] {
                let generated = fixture(scenario, placement, payload);
                let name = format!("{scenario}-{payload}-{placement}");
                let result = session.evaluate_interop(&generated);
                let observation = match result {
                    Ok(value) => {
                        assert_eq!((scenario, payload), ("plain", "int"), "{name}");
                        serde_json::json!({ "case": name, "value": value.value, "contexts": [] })
                    }
                    Err(error) => {
                        assert_ne!(error.kind, rusix::DiagnosticKind::Tooling, "{error:?}");
                        assert_ne!(error.kind, rusix::DiagnosticKind::Compiler, "{error:?}");
                        let messages = trace_messages(&error.raw_nix);
                        assert!(!messages.is_empty(), "{name}: missing structured trace");
                        let contexts: Vec<_> = messages
                            .iter()
                            .filter(|message| message.ends_with("-ORIGIN"))
                            .cloned()
                            .collect();
                        fs::write(
                            root.join(format!("{name}.json")),
                            serde_json::to_vec_pretty(&error).unwrap(),
                        )
                        .unwrap();
                        let mut expected = Vec::new();
                        if payload == "throw" {
                            assert_eq!(error.reason, "deferred child failure");
                            if matches!(placement, "child" | "combined") {
                                expected.push("CHILD-ORIGIN".to_owned());
                            }
                            if scenario == "plain" && matches!(placement, "field" | "combined") {
                                expected.push("FIELD-ORIGIN".to_owned());
                            }
                        } else if payload == "nested-throw" {
                            assert_eq!(error.reason, "deferred outPath failure");
                        } else if scenario == "plain" {
                            assert!(error.reason.contains("cannot coerce a set to a string"));
                        } else {
                            assert!(error.reason.contains("Dependency is not of a valid type"));
                        }
                        assert_eq!(contexts, expected, "{name}: {messages:?}");
                        serde_json::json!({ "case": name, "reason": error.reason, "contexts": contexts })
                    }
                };
                observations.push(observation);
            }
        }
    }
    fs::write(
        root.join("observations.json"),
        serde_json::to_vec_pretty(&observations).unwrap(),
    )
    .unwrap();
}

fn handoff_stdenv(placement: &str) -> NixValue {
    InputRef::local(
        "backend-provenance-handoff",
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/backend-provenance-handoff.nix"),
    )
    .function("instrument")
    .call(NixValue::record([
        ("stdenv", Nixpkgs::new().value("stdenv")),
        ("placement", placement.into()),
    ]))
}

fn package_graph(
    placement: &str,
    supplied_openssl: Option<NixValue>,
    supplied_curl: Option<NixValue>,
) -> NixAttrs<Package> {
    let pkgs = Nixpkgs::new();
    let stdenv = if placement == "none" {
        pkgs.value("stdenv")
    } else {
        handoff_stdenv(placement)
    };
    let openssl = supplied_openssl
        .map(Package::from_expression)
        .unwrap_or_else(|| {
            pkgs.call_package(
                &composition::openssl::factory(composition::openssl::model::Release::Preview),
                NixAttrs::new([("stdenv", stdenv.clone())]),
            )
        });
    openssl.bind(|openssl| {
        let curl = supplied_curl
            .map(Package::from_expression)
            .unwrap_or_else(|| {
                pkgs.try_call_package(
                    &composition::curl::factory(),
                    composition::curl_arguments(&pkgs, openssl.clone())
                        .with_overrides(NixAttrs::new([("stdenv", stdenv.clone())])),
                )
                .unwrap()
            });
        let git = pkgs.call_package(
            &composition::git::factory(),
            NixAttrs::try_from_record(composition::git::arguments().with_openssl(openssl.clone()))
                .unwrap()
                .merge(NixAttrs::new([("stdenv", stdenv.clone())])),
        );
        curl.bind(|curl| {
            let mariadb = pkgs.call_package(
                &composition::mariadb::factory(),
                NixAttrs::try_from_record(composition::mariadb::model::Release::V1011.arguments())
                    .unwrap()
                    .merge(NixAttrs::new([
                        ("curl", curl.clone().into()),
                        ("stdenv", stdenv),
                    ])),
            );
            NixAttrs::new([
                ("openssl", openssl),
                ("curl", curl),
                ("git", git),
                ("mariadb", mariadb),
            ])
        })
    })
}

#[test]
fn real_package_handoffs_do_not_recover_delayed_dependency_origins() {
    let session = NixSession::new().unwrap();
    let root = artifact_root().join("real-failures");
    fs::create_dir_all(&root).unwrap();
    let mut observations = Vec::new();

    for case in [
        "mariadb-curl-int",
        "git-openssl-int",
        "curl-openssl-int",
        "mariadb-curl-attrs",
    ] {
        let mut baseline = None;
        for placement in ["none", "field", "child", "call", "combined"] {
            let supplied = if case.ends_with("attrs") {
                NixValue::record([("notAPackage", true.into())])
            } else {
                1_i64.into()
            };
            let graph = if case.starts_with("mariadb") {
                package_graph(placement, None, Some(supplied))
            } else {
                package_graph(placement, Some(supplied), None)
            };
            let path = if case.starts_with("mariadb") {
                "mariadb.drvPath"
            } else if case.starts_with("git") {
                "git.drvPath"
            } else {
                "curl.drvPath"
            };
            let mut generated = rusix::compile(
                Config::new().set_dynamic("result", graph.as_expression().select(path)),
            )
            .unwrap();
            // Historical context-only experiment: correlation is tested separately.
            generated.backend_metadata = None;
            let error = session.evaluate_interop(&generated).unwrap_err();
            assert_ne!(error.kind, rusix::DiagnosticKind::Tooling, "{error:?}");
            assert_ne!(error.kind, rusix::DiagnosticKind::Compiler, "{error:?}");
            let messages = trace_messages(&error.raw_nix);
            if case == "curl-openssl-int" && matches!(placement, "child" | "combined") {
                // This expression throws during evaluation; the existing Rust
                // configureFlags operation already identifies the precise cause.
                assert!(messages.iter().any(|m| m == "HANDOFF-CHILD:configureFlags"));
            } else {
                assert!(
                    !messages.iter().any(|m| m.starts_with("HANDOFF-")),
                    "{case}/{placement}: {messages:?}"
                );
            }
            let attribution = (
                error.reason.clone(),
                error.primary.clone(),
                error.option_path.clone(),
            );
            if let Some(baseline) = &baseline {
                assert_eq!(&attribution, baseline, "{case}/{placement}");
            } else {
                baseline = Some(attribution);
            }
            let name = format!("{case}-{placement}");
            fs::write(root.join(format!("{name}.nix")), &generated.source).unwrap();
            fs::write(
                root.join(format!("{name}-map.json")),
                serde_json::to_vec_pretty(&generated).unwrap(),
            )
            .unwrap();
            fs::write(
                root.join(format!("{name}.json")),
                serde_json::to_vec_pretty(&error).unwrap(),
            )
            .unwrap();
            observations.push(serde_json::json!({
                "case": name, "reason": error.reason, "primary": error.primary,
                "semantic_path": error.option_path,
                "contexts": messages.iter().filter(|m| m.starts_with("HANDOFF-")).collect::<Vec<_>>(),
            }));
        }
    }
    fs::write(
        root.join("observations.json"),
        serde_json::to_vec_pretty(&observations).unwrap(),
    )
    .unwrap();
}

#[test]
fn real_package_handoffs_preserve_exact_recipes_and_output_paths() {
    for placement in ["none", "field", "child", "call", "combined"] {
        let generated = support::artifact(
            "composed",
            [("graph", package_graph(placement, None, None).into())],
        );
        support::compare(
            "composed",
            &format!("backend-provenance-{placement}"),
            generated,
        );
    }
}

#[test]
fn real_child_package_validation_loses_context_across_composed_edges() {
    let session = NixSession::new().unwrap();
    let root = artifact_root().join("child-packages");
    fs::create_dir_all(&root).unwrap();

    for case in [
        "curl-native-through-mariadb",
        "openssl-build-through-mariadb",
        "openssl-build-through-git",
    ] {
        let mut baseline = None;
        for placement in ["none", "field", "child", "call", "combined"] {
            let pkgs = Nixpkgs::new();
            let stdenv = if placement == "none" {
                pkgs.value("stdenv")
            } else {
                handoff_stdenv(placement)
            };
            let mut openssl_args = NixAttrs::new([("stdenv", stdenv.clone())]);
            if case.starts_with("openssl") {
                openssl_args = openssl_args.merge(NixAttrs::new([
                    ("withCryptodev", true.into()),
                    ("cryptodev", 1_i64.into()),
                ]));
            }
            let openssl = pkgs.call_package(
                &composition::openssl::factory(composition::openssl::model::Release::Preview),
                openssl_args,
            );
            let mut curl_args = NixAttrs::new([("stdenv", stdenv)]);
            if case.starts_with("curl") {
                curl_args = curl_args.merge(NixAttrs::new([("pkg-config", 1_i64.into())]));
            }
            let curl = pkgs
                .try_call_package(
                    &composition::curl::factory(),
                    composition::curl_arguments(&pkgs, openssl.clone()).with_overrides(curl_args),
                )
                .unwrap();
            let graph = package_graph(placement, Some(openssl.into()), Some(curl.into()));
            let path = if case.ends_with("git") {
                "git.drvPath"
            } else {
                "mariadb.drvPath"
            };
            let mut generated = rusix::compile(
                Config::new().set_dynamic("result", graph.as_expression().select(path)),
            )
            .unwrap();
            // Historical context-only experiment: correlation is tested separately.
            generated.backend_metadata = None;
            let error = session.evaluate_interop(&generated).unwrap_err();
            let expected = if case.starts_with("curl") {
                "nativeBuildInputs for curl"
            } else {
                "buildInputs for openssl"
            };
            assert!(
                error.reason.contains(expected),
                "{case}/{placement}: {error:?}"
            );
            // A parent's deferred configureFlags operation can remain active,
            // but the offending dependency's backend handoff has already ended.
            assert!(
                !trace_messages(&error.raw_nix).iter().any(|m| {
                    matches!(
                        m.as_str(),
                        "HANDOFF-CHILD:buildInputs"
                            | "HANDOFF-CHILD:nativeBuildInputs"
                            | "HANDOFF-CHILD:propagatedBuildInputs"
                    )
                }),
                "{case}/{placement}"
            );
            let attribution = (
                error.reason.clone(),
                error.primary.clone(),
                error.option_path.clone(),
            );
            if let Some(baseline) = &baseline {
                assert_eq!(&attribution, baseline, "{case}/{placement}");
            } else {
                baseline = Some(attribution);
            }
            let name = format!("{case}-{placement}");
            fs::write(root.join(format!("{name}.nix")), &generated.source).unwrap();
            fs::write(
                root.join(format!("{name}-map.json")),
                serde_json::to_vec_pretty(&generated).unwrap(),
            )
            .unwrap();
            fs::write(
                root.join(format!("{name}.json")),
                serde_json::to_vec_pretty(&error).unwrap(),
            )
            .unwrap();
        }
    }
}

#[test]
fn contexts_cover_evaluation_not_the_lifetime_of_successful_values() {
    let session = NixSession::new().unwrap();
    let root = artifact_root().join("lifetime");
    fs::create_dir_all(&root).unwrap();

    for (name, expression, expected) in [
        (
            "field-head",
            "builtins.addErrorContext \"FIELD-ORIGIN\" (throw \"head failure\")",
            true,
        ),
        (
            "list-head",
            "builtins.head (builtins.addErrorContext \"FIELD-ORIGIN\" [ (throw \"child failure\") ])",
            false,
        ),
        (
            "attrset-head",
            "(builtins.addErrorContext \"FIELD-ORIGIN\" { child = throw \"child failure\"; }).child",
            false,
        ),
        (
            "attrset-child",
            "{ child = builtins.addErrorContext \"FIELD-ORIGIN\" (throw \"child failure\"); }.child",
            true,
        ),
        (
            "successful-scalar",
            "let child = builtins.addErrorContext \"FIELD-ORIGIN\" 1; in builtins.seq child (if builtins.isInt child then throw \"later validation\" else true)",
            false,
        ),
        (
            "successful-container",
            "let child = builtins.addErrorContext \"FIELD-ORIGIN\" { later = throw \"later force\"; }; in builtins.seq child child.later",
            false,
        ),
        (
            "transform-throws",
            "(x: if builtins.isInt x then throw \"helper rejection\" else x) (builtins.addErrorContext \"FIELD-ORIGIN\" 1)",
            false,
        ),
        (
            "operation-covers-transform",
            "builtins.addErrorContext \"FIELD-ORIGIN\" ((x: if builtins.isInt x then throw \"helper rejection\" else x) 1)",
            true,
        ),
    ] {
        let generated = Generated {
            source: expression.into(),
            ..Generated::default()
        };
        let error = session.evaluate(&generated).unwrap_err();
        assert_eq!(
            trace_messages(&error.raw_nix)
                .iter()
                .any(|m| m == "FIELD-ORIGIN"),
            expected,
            "{name}: {error:?}"
        );
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&error).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn real_package_handoffs_leave_excluded_and_partial_graph_branches_lazy() {
    let session = NixSession::new().unwrap();
    for placement in ["none", "field", "child", "call", "combined"] {
        let bad = NixValue::builtin("throw").call("excluded graph node");
        let graph = package_graph(placement, None, Some(bad.clone()));
        let git = graph.as_expression().select("git.drvPath");
        let generated = rusix::compile(Config::new().set_dynamic("result", git)).unwrap();
        session.evaluate_interop(&generated).unwrap();

        let graph = package_graph(placement, Some(bad), None);
        let curl = graph
            .as_expression()
            .select("curl.override")
            .call(NixValue::record([("opensslSupport", false.into())]))
            .select("drvPath");
        let generated = rusix::compile(Config::new().set_dynamic("result", curl)).unwrap();
        session.evaluate_interop(&generated).unwrap();
    }
}

#[test]
fn handoffs_preserve_unused_defaults_fields_validation_order_and_final_attrs() {
    let session = NixSession::new().unwrap();
    let root = artifact_root().join("laziness");
    fs::create_dir_all(&root).unwrap();
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/backend-provenance-lazy.nix");
    let handoff_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/backend-provenance-handoff.nix");
    let mut baseline = None;

    for placement in ["none", "field", "child", "call", "combined"] {
        for early_failure in [false, true] {
            let generated = Generated {
                source: format!(
                    "let pkgs = import ./nixpkgs-full {{ system = \"x86_64-linux\"; config = {{}}; }}; in import {} {{ inherit pkgs; stdenv = (import {}).instrument {{ stdenv = pkgs.stdenv; placement = {placement:?}; }}; earlyFailure = {early_failure}; }}",
                    serde_json::to_string(&fixture_path.to_string_lossy()).unwrap(),
                    serde_json::to_string(&handoff_path.to_string_lossy()).unwrap(),
                ),
                ..Generated::default()
            };
            if early_failure {
                let error = session.evaluate_interop(&generated).unwrap_err();
                assert!(
                    error.reason.contains("unsupported hardening flags"),
                    "{error:?}"
                );
                assert!(
                    !trace_messages(&error.raw_nix)
                        .iter()
                        .any(|m| m == "UNUSED-ORIGIN")
                );
                fs::write(
                    root.join(format!("{placement}-early-error.json")),
                    serde_json::to_vec_pretty(&error).unwrap(),
                )
                .unwrap();
            } else {
                let value = session.evaluate_interop(&generated).unwrap().value;
                assert_eq!(value["finalAttrsOverride"], "2");
                if let Some(baseline) = &baseline {
                    assert_eq!(&value, baseline, "{placement}");
                } else {
                    baseline = Some(value.clone());
                }
                fs::write(
                    root.join(format!("{placement}.json")),
                    serde_json::to_vec_pretty(&value).unwrap(),
                )
                .unwrap();
            }
        }
    }
}
