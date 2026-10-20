#![cfg(feature = "evaluation")]

//! Delayed backend clues and out-of-band Rust supplier correlation.
use rusix::{Generated, NixSession};
use std::{fs, path::Path};

#[test]
fn audit_additional_delayed_backend_clues() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/backend-correlation/clues");
    fs::create_dir_all(&root).unwrap();
    let session = NixSession::new().unwrap();
    let mut observations = Vec::new();
    for (name, attrs) in [
        ("propagated", "propagatedBuildInputs = [ pkgs.zlib 1 ];"),
        ("nested", "buildInputs = [ pkgs.zlib [ pkgs.openssl 1 ] ];"),
        (
            "check-input",
            "doCheck = true; buildInputs = [ pkgs.zlib ]; checkInputs = [ 1 ];",
        ),
        (
            "native-check-input",
            "doCheck = true; nativeBuildInputs = []; nativeCheckInputs = [ 1 ];",
        ),
        (
            "dependency-output",
            "buildInputs = [ { type = \"derivation\"; outPath = { broken = true; }; } ];",
        ),
        ("duplicate-outputs", "outputs = [ \"out\" \"out\" ];"),
        ("invalid-output-name", "outputs = [ \"out\" \"bad/name\" ];"),
        ("output-type", "outputs = [ \"out\" {} ];"),
        ("reference-type", "allowedReferences = [ {} ];"),
        ("environment-key", "env = { BAD = {}; };"),
        ("configure-flags", "configureFlags = [ \"good\" {} ];"),
    ] {
        let generated = Generated {
            source: format!(
                "let pkgs = import ./nixpkgs-full {{ system = \"x86_64-linux\"; config = {{}}; }}; in (pkgs.stdenv.mkDerivation {{ name = \"clue-audit\"; dontUnpack = true; {attrs} }}).drvPath"
            ),
            ..Generated::default()
        };
        let error = session.evaluate_interop(&generated).unwrap_err();
        assert_eq!(
            error.kind,
            rusix::DiagnosticKind::NixEval,
            "{name}: {error:?}"
        );
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&error).unwrap(),
        )
        .unwrap();
        observations.push(serde_json::json!({"case":name, "reason":error.reason}));
    }
    fs::write(
        root.join("observations.json"),
        serde_json::to_vec_pretty(&observations).unwrap(),
    )
    .unwrap();
}

#[path = "../../../examples/composed-packages/composition.rs"]
pub mod composition;

use rusix::{
    Config,
    interop::{
        NixAttrs, Nixpkgs, Package,
        raw::{NixFunctionExt, NixRepresentation, NixValue, NixpkgsExt},
    },
};

fn real_case(case: &str, supplied: NixValue) -> (rusix::Generated, String) {
    let pkgs = Nixpkgs::new();
    let mut openssl_args = composition::arguments();
    if case.starts_with("openssl-build") {
        openssl_args = NixAttrs::new([
            ("withCryptodev", true.into()),
            ("cryptodev", supplied.clone()),
        ]);
    }
    let openssl = if case == "git-openssl-int" || case == "curl-openssl-int" {
        Package::from_expression(supplied.clone())
    } else {
        pkgs.call_package(
            &composition::openssl::factory(composition::openssl::model::Release::Preview),
            openssl_args,
        )
    };
    let graph = composition::compose(
        openssl,
        |pkgs, openssl| {
            if case.starts_with("mariadb-curl") {
                return Package::from_expression(supplied.clone());
            }
            let args = composition::curl_arguments(pkgs, openssl.clone());
            let args = if case.starts_with("curl-native") {
                args.with_overrides(NixAttrs::new([("pkg-config", supplied.clone())]))
            } else {
                args
            };
            pkgs.try_call_package(&composition::curl::factory(), args)
                .unwrap()
        },
        composition::mariadb::model::Release::V1011.arguments(),
    );
    let path = if case == "git-openssl-int" || case.ends_with("git") {
        "git.drvPath"
    } else if case == "curl-openssl-int" {
        "curl.drvPath"
    } else {
        "mariadb.drvPath"
    };
    (
        rusix::compile(Config::new().set_dynamic("result", graph.as_expression().select(path)))
            .unwrap(),
        path.into(),
    )
}

#[test]
fn real_dependency_suppliers_recover_after_failure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/backend-correlation/real");
    let session = NixSession::new().unwrap();
    fs::create_dir_all(&root).unwrap();
    for case in [
        "mariadb-curl-int",
        "mariadb-curl-attrs",
        "git-openssl-int",
        "curl-openssl-int",
        "curl-native-through-mariadb",
        "openssl-build-through-mariadb",
        "openssl-build-through-git",
    ] {
        let supplied = if case.ends_with("attrs") {
            NixValue::record([("notAPackage", true.into())])
        } else {
            rusix::Expr::int(1).into()
        };
        let (generated, _) = real_case(case, supplied);
        fs::write(
            root.join(format!("{case}-map.json")),
            serde_json::to_vec_pretty(&generated).unwrap(),
        )
        .unwrap();
        assert!(generated.backend_metadata.is_some(), "{case}");
        let after = session.evaluate_interop(&generated).unwrap_err();
        let mut before_map = generated.clone();
        before_map.backend_metadata = None;
        // Correlate the identical failure, without a second evaluation.
        let file = after
            .raw_nix
            .lines()
            .filter_map(|l| l.strip_prefix("@nix "))
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .flat_map(|e| e["trace"].as_array().cloned().unwrap_or_default())
            .find_map(|f| {
                f["file"]
                    .as_str()
                    .filter(|s| s.contains("/generated.nix"))
                    .map(|s| s.split("generated.nix").next().unwrap().to_owned() + "generated.nix")
            })
            .unwrap();
        let before =
            rusix::Diagnostic::from_nix(after.kind, &after.raw_nix, &before_map, Path::new(&file));
        fs::write(
            root.join(format!("{case}-before.json")),
            serde_json::to_vec_pretty(&before).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join(format!("{case}-after.json")),
            serde_json::to_vec_pretty(&after).unwrap(),
        )
        .unwrap();
        assert_eq!(after.reason, before.reason);
        assert_eq!(after.raw_nix, before.raw_nix);
        if case == "curl-openssl-int" {
            assert_eq!(after.primary, before.primary);
            assert_eq!(after.provenance, before.provenance);
        } else {
            assert_eq!(
                after.provenance,
                rusix::Provenance::BackendCorrelation,
                "{case}: {}",
                after.summary()
            );
            assert_ne!(after.primary, before.primary, "{case}");
            assert!(
                after
                    .primary
                    .as_ref()
                    .unwrap()
                    .file
                    .ends_with("backend_correlation.rs"),
                "{case}: {}",
                after.summary()
            );
            assert_eq!(after.option_path, before.option_path);
        }
        // Text-only older evaluator diagnostics must recover the same supplier.
        let rendered = after
            .raw_nix
            .lines()
            .filter_map(|l| l.strip_prefix("@nix "))
            .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
            .find(|e| e["raw_msg"].is_string() && e["level"] == 0)
            .unwrap()["msg"]
            .as_str()
            .unwrap()
            .to_owned();
        let text = rusix::Diagnostic::from_nix(after.kind, &rendered, &generated, Path::new(&file));
        assert_eq!(text.primary, after.primary, "{case}: {}", text.summary());
        assert_eq!(text.provenance, after.provenance, "{case}");
        assert_eq!(generated.source, before_map.source);
    }
}

fn dependency_recipe(name: &str, field: &str, inputs: NixValue) -> NixValue {
    NixValue::record([
        ("name", name.into()),
        ("dontUnpack", true.into()),
        (field, inputs),
    ])
}

fn demand(recipe: NixValue) -> Generated {
    let package = Nixpkgs::new().value("stdenv.mkDerivation").call(recipe);
    rusix::compile(Config::new().set_dynamic("result", package.select("drvPath"))).unwrap()
}

fn value_origin(value: NixValue) -> rusix::ir::Origin {
    Config::new().set_dynamic("probe", value).assignments[0]
        .value
        .origin
        .clone()
}

#[test]
fn indexed_nested_propagated_and_final_attrs_inputs_recover_the_supplier() {
    let session = NixSession::new().unwrap();
    let bad: NixValue = rusix::Expr::int(23).into();
    let expected = value_origin(bad.clone());
    for field in [
        "buildInputs",
        "nativeBuildInputs",
        "propagatedBuildInputs",
        "propagatedNativeBuildInputs",
    ] {
        let deps = NixValue::list([
            Nixpkgs::new().value("zlib"),
            NixValue::list([Nixpkgs::new().value("openssl"), bad.clone()]),
        ]);
        let recipe = dependency_recipe("nested-probe", field, deps);
        let recipe = NixValue::function(|final_attrs| {
            recipe.merge_attrs(NixValue::record([("unused", final_attrs.select("unused"))]))
        });
        let generated = demand(recipe);
        let error = session.evaluate_interop(&generated).unwrap_err();
        assert!(
            error
                .reason
                .contains(&format!("element 2 of element 2 of {field}"))
        );
        assert_eq!(error.primary, Some(expected.clone()), "{}", error.summary());
        assert_eq!(error.provenance, rusix::Provenance::BackendCorrelation);
    }
}

#[test]
fn unknown_list_prefix_and_backend_appended_check_inputs_do_not_invent_a_child() {
    let session = NixSession::new().unwrap();
    let bad: NixValue = rusix::Expr::int(23).into();
    // An opaque helper determines an unknown prefix length. The child after it
    // cannot inherit the literal index it would have before the transformation.
    let prefix = Nixpkgs::new()
        .value("lib.flatten")
        .call(NixValue::list([NixValue::list([
            Nixpkgs::new().value("zlib")
        ])]));
    let unknown = demand(dependency_recipe(
        "unknown-prefix",
        "buildInputs",
        NixValue::concat_lists([prefix, NixValue::list([bad.clone()])]),
    ));
    let appended = demand(
        dependency_recipe(
            "appended-check",
            "buildInputs",
            NixValue::list([Nixpkgs::new().value("zlib")]),
        )
        .merge_attrs(NixValue::record([
            ("doCheck", true.into()),
            ("checkInputs", NixValue::list([bad])),
        ])),
    );
    for generated in [unknown, appended] {
        let error = session.evaluate_interop(&generated).unwrap_err();
        assert!(error.reason.contains("element 2 of buildInputs"));
        assert_ne!(error.provenance, rusix::Provenance::BackendCorrelation);
    }
}

#[test]
fn unused_defaults_optional_fields_and_partial_graph_stay_lazy() {
    let broken: NixValue = rusix::Expr::int(1).divide(rusix::Expr::int(0)).into();
    let factory = NixValue::function_attrs(["unused"], |args| {
        let inputs = NixValue::concat_lists([
            NixValue::list([Nixpkgs::new().value("zlib")]),
            Nixpkgs::new()
                .value("lib.optional")
                .call(false)
                .call(args.select("unused")),
        ]);
        (
            vec![("unused", broken.clone())],
            Nixpkgs::new().value("stdenv.mkDerivation").call(
                dependency_recipe("lazy-correlation", "buildInputs", inputs).merge_attrs(
                    NixValue::record([(
                        "passthru",
                        NixValue::record([("unused", broken.clone())]),
                    )]),
                ),
            ),
        )
    });
    let good = factory.call(composition::arguments());
    let partial = NixValue::record([("good", good), ("unused", broken)]);
    let generated =
        rusix::compile(Config::new().set_dynamic("result", partial.select("good.drvPath")))
            .unwrap();
    assert!(generated.backend_metadata.is_some());
    let mut baseline = generated.clone();
    baseline.backend_metadata = None;
    let session = NixSession::new().unwrap();
    assert_eq!(
        session.evaluate_interop(&generated).unwrap().value,
        session.evaluate_interop(&baseline).unwrap().value
    );
}

#[test]
fn old_and_unknown_metadata_remain_readable_without_correlating() {
    let generated = demand(dependency_recipe(
        "compatibility",
        "buildInputs",
        NixValue::list([1_i64.into()]),
    ));
    let mut value = serde_json::to_value(&generated).unwrap();
    value.as_object_mut().unwrap().remove("backend_metadata");
    let old: Generated = serde_json::from_value(value).unwrap();
    assert!(old.backend_metadata.is_none());
    assert_eq!(generated.source, old.source);
    let session = NixSession::new().unwrap();
    for key in ["version", "revision"] {
        let mut unsupported = generated.clone();
        unsupported.backend_metadata.as_mut().unwrap()[key] = serde_json::json!("unsupported");
        assert_ne!(
            session
                .evaluate_interop(&unsupported)
                .unwrap_err()
                .provenance,
            rusix::Provenance::BackendCorrelation
        );
    }
}

#[test]
fn identical_owner_names_report_multiple_possible_suppliers() {
    let a: NixValue = rusix::Expr::int(1).into();
    let b: NixValue = rusix::Expr::int(2).into();
    let expected = [value_origin(a.clone()), value_origin(b.clone())];
    let pkgs = Nixpkgs::new();
    let left = pkgs.value("stdenv.mkDerivation").call(dependency_recipe(
        "same-child",
        "buildInputs",
        NixValue::list([a]),
    ));
    let right = pkgs.value("stdenv.mkDerivation").call(dependency_recipe(
        "same-child",
        "buildInputs",
        NixValue::list([b]),
    ));
    let generated = demand(dependency_recipe(
        "owner",
        "buildInputs",
        NixValue::list([left, right]),
    ));
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert_eq!(
        error.provenance,
        rusix::Provenance::BackendCorrelation,
        "{}",
        error.summary()
    );
    assert!(error.primary.is_none(), "{}", error.summary());
    assert_eq!(error.origins.len(), 2);
    for origin in &error.origins {
        assert_eq!(origin.role, rusix::OriginRole::BackendCandidate);
        assert!(expected.contains(origin.origin.as_ref().unwrap()));
    }
    assert!(
        error
            .render(Path::new("."))
            .contains("possible backend supplier")
    );
}

#[test]
fn unused_same_name_assignment_does_not_contaminate_the_active_scope() {
    let a: NixValue = rusix::Expr::int(1).into();
    let b: NixValue = rusix::Expr::int(2).into();
    let expected = value_origin(b.clone());
    let pkgs = Nixpkgs::new();
    let left = pkgs.value("stdenv.mkDerivation").call(dependency_recipe(
        "same-entry",
        "buildInputs",
        NixValue::list([a]),
    ));
    let right = pkgs.value("stdenv.mkDerivation").call(dependency_recipe(
        "same-entry",
        "buildInputs",
        NixValue::list([b]),
    ));
    let generated = rusix::compile(
        Config::new()
            .set_dynamic("unused", left.select("drvPath"))
            .set_dynamic("result", right.select("drvPath")),
    )
    .unwrap();
    // Stage pinned inputs using the isolated interop helper, then use its
    // escaped selection API. Unselected assignments remain lazy.
    let session = NixSession::new().unwrap();
    session
        .evaluate_interop(&Generated {
            source: "null".into(),
            ..Generated::default()
        })
        .unwrap();
    let error = session
        .evaluate_attribute(&generated, "result")
        .unwrap_err();
    assert_eq!(error.primary, Some(expected), "{}", error.summary());
    assert_eq!(error.origins.len(), 1);
}

#[test]
fn opaque_overrides_do_not_reuse_the_original_recipe_table() {
    let package = Package::from_expression(Nixpkgs::new().value("stdenv.mkDerivation").call(
        dependency_recipe("overridden", "buildInputs", NixValue::list([1_i64.into()])),
    ));
    let package = package
        .override_attrs(|_| NixAttrs::new([("buildInputs", NixValue::list([2_i64.into()]))]));
    let generated = rusix::compile(
        Config::new().set_dynamic("result", package.as_expression().select("drvPath")),
    )
    .unwrap();
    assert!(generated.backend_metadata.is_none());
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(error.reason.contains("Dependency is not of a valid type"));
    assert_ne!(error.provenance, rusix::Provenance::BackendCorrelation);
}

#[test]
fn unverified_validator_text_never_overrides_a_direct_origin() {
    let generated = demand(dependency_recipe(
        "parser-guard",
        "buildInputs",
        NixValue::list([1_i64.into()]),
    ));
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert_eq!(error.provenance, rusix::Provenance::BackendCorrelation);
    let event = error
        .raw_nix
        .lines()
        .filter_map(|l| l.strip_prefix("@nix "))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|e| e["raw_msg"].is_string() && e["level"] == 0)
        .unwrap();
    let file = event["trace"]
        .as_array()
        .unwrap()
        .iter()
        .find_map(|f| {
            f["file"]
                .as_str()
                .filter(|s| s.contains("/generated.nix"))
                .map(|s| s.split("generated.nix").next().unwrap().to_owned() + "generated.nix")
        })
        .unwrap();
    for variant in [
        "foreign",
        "new-validator-line",
        "no-trace",
        "direct-position",
    ] {
        let mut event = event.clone();
        match variant {
            "foreign" => {
                event["trace"][0]["file"] =
                    serde_json::json!("/untrusted/make-derivation.nix:284:14")
            }
            "new-validator-line" => event["trace"][0]["line"] = serde_json::json!(285),
            "no-trace" => event["trace"] = serde_json::json!([]),
            "direct-position" => {
                // Strong direct generated evidence must win even if a backend
                // clue is also present in the same event.
                event["file"] = serde_json::json!(file);
                let span = generated.spans.iter().find(|s| s.diagnostic_site).unwrap();
                let prefix = &generated.source[..span.start];
                event["line"] =
                    serde_json::json!(prefix.bytes().filter(|b| *b == b'\n').count() + 1);
                event["column"] =
                    serde_json::json!(prefix.rsplit('\n').next().unwrap().chars().count() + 1);
            }
            _ => unreachable!(),
        }
        let raw = format!("@nix {event}");
        let diagnostic =
            rusix::Diagnostic::from_nix(error.kind, &raw, &generated, Path::new(&file));
        assert_ne!(
            diagnostic.provenance,
            rusix::Provenance::BackendCorrelation,
            "{variant}"
        );
        assert_eq!(diagnostic.raw_nix, raw);
        let compiler = rusix::Diagnostic::from_nix(
            rusix::DiagnosticKind::Compiler,
            &raw,
            &generated,
            Path::new(&file),
        );
        assert!(compiler.primary.is_none());
        assert!(compiler.origins.is_empty());
    }
}

#[test]
fn foreign_same_name_dependency_prevents_a_false_unique_match() {
    let a: NixValue = rusix::Expr::int(1).into();
    let b: NixValue = rusix::Expr::int(2).into();
    let pkgs = Nixpkgs::new();
    let known = pkgs.value("stdenv.mkDerivation").call(dependency_recipe(
        "collision",
        "buildInputs",
        NixValue::list([a]),
    ));
    let foreign = rusix::interop::InputRef::local(
        "foreign-correlation",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/backend-correlation.nix"),
    )
    .function("package")
    .call(pkgs.as_value())
    .call(dependency_recipe(
        "collision",
        "buildInputs",
        NixValue::list([b]),
    ));
    let generated = demand(dependency_recipe(
        "collision-owner",
        "buildInputs",
        NixValue::list([foreign, known]),
    ));
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(
        error
            .reason
            .contains("element 1 of buildInputs for collision"),
        "{}",
        error.summary()
    );
    assert_ne!(error.provenance, rusix::Provenance::BackendCorrelation);
}

#[test]
fn identical_parent_and_child_names_preserve_owner_depth() {
    let bad: NixValue = rusix::Expr::int(4).into();
    let expected = value_origin(bad.clone());
    let child = Nixpkgs::new()
        .value("stdenv.mkDerivation")
        .call(dependency_recipe(
            "same-depth",
            "buildInputs",
            NixValue::list([bad]),
        ));
    let generated = demand(dependency_recipe(
        "same-depth",
        "buildInputs",
        NixValue::list([child]),
    ));
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert_eq!(error.primary, Some(expected), "{}", error.summary());
    assert_eq!(error.provenance, rusix::Provenance::BackendCorrelation);
}

#[test]
fn call_package_auto_arguments_are_not_mistaken_for_authored_defaults() {
    let bad: NixValue = rusix::Expr::int(3).into();
    let default_origin = value_origin(bad.clone());
    let factory = NixValue::function_attrs(["stdenv", "openssl"], |args| {
        let deps = NixValue::list([args.clone().select("openssl")]);
        (
            vec![("openssl", bad)],
            args.select("stdenv.mkDerivation").call(dependency_recipe(
                "auto-args",
                "buildInputs",
                deps,
            )),
        )
    });
    let pkgs = Nixpkgs::new();
    let automatic = pkgs
        .value("callPackage")
        .call(factory.clone())
        .call(composition::arguments());
    let automatic =
        rusix::compile(Config::new().set_dynamic("result", automatic.select("drvPath"))).unwrap();
    let metadata = automatic.backend_metadata.as_ref().unwrap();
    let recorded: rusix::ir::Origin = serde_json::from_value(
        metadata["boundaries"][0]["fields"][0]["children"][0]["origin"].clone(),
    )
    .unwrap();
    assert_ne!(recorded, default_origin);
    let direct = factory.call(NixAttrs::new([("stdenv", pkgs.value("stdenv"))]));
    let direct =
        rusix::compile(Config::new().set_dynamic("result", direct.select("drvPath"))).unwrap();
    let session = NixSession::new().unwrap();
    session.evaluate_interop(&automatic).unwrap();
    let error = session.evaluate_interop(&direct).unwrap_err();
    assert_eq!(error.primary, Some(default_origin), "{}", error.summary());
    assert_eq!(error.provenance, rusix::Provenance::BackendCorrelation);
}

#[test]
fn unresolved_automatic_default_cannot_hide_a_competing_package_owner() {
    let pkgs = Nixpkgs::new();
    let a = pkgs.value("stdenv.mkDerivation").call(dependency_recipe(
        "default-collision",
        "buildInputs",
        NixValue::list([rusix::Expr::int(1).into()]),
    ));
    let b = pkgs.value("stdenv.mkDerivation").call(dependency_recipe(
        "default-collision",
        "buildInputs",
        NixValue::list([rusix::Expr::int(2).into()]),
    ));
    let factory = NixValue::function_attrs(["stdenv", "rusixUnknownPackage"], |args| {
        (
            vec![("rusixUnknownPackage", a)],
            args.clone()
                .select("stdenv.mkDerivation")
                .call(dependency_recipe(
                    "default-owner",
                    "buildInputs",
                    NixValue::list([args.select("rusixUnknownPackage"), b]),
                )),
        )
    });
    let package = pkgs
        .value("callPackage")
        .call(factory)
        .call(composition::arguments());
    let generated =
        rusix::compile(Config::new().set_dynamic("result", package.select("drvPath"))).unwrap();
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(
        error.reason.contains("for default-collision"),
        "{}",
        error.summary()
    );
    assert_ne!(error.provenance, rusix::Provenance::BackendCorrelation);
}

#[test]
fn backend_failure_in_an_assertion_guard_does_not_blame_the_unforced_body() {
    let pkgs = Nixpkgs::new();
    let a = pkgs.value("stdenv.mkDerivation").call(dependency_recipe(
        "guard-collision",
        "buildInputs",
        NixValue::list([rusix::Expr::int(1).into()]),
    ));
    let b = pkgs.value("stdenv.mkDerivation").call(dependency_recipe(
        "guard-collision",
        "buildInputs",
        NixValue::list([rusix::Expr::int(2).into()]),
    ));
    let guarded = NixValue::assert(a.select("drvPath").equals("unused"), b.select("drvPath"));
    let generated = rusix::compile(Config::new().set_dynamic("result", guarded)).unwrap();
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(error.reason.contains("for guard-collision"));
    assert_ne!(
        error.provenance,
        rusix::Provenance::BackendCorrelation,
        "{}",
        error.summary()
    );
}
