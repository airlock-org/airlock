#![cfg(feature = "evaluation")]

use rusix::interop::raw::NixRepresentation;

#[path = "../../../examples/composed-packages/composition.rs"]
pub mod composition;

#[path = "support/packages.rs"]
mod support;

use rusix::{
    Config,
    interop::{NixAttrs, Nixpkgs, Package, raw::NixValue},
};

fn compare(
    name: &str,
    graph: impl Into<NixValue>,
    fields: impl IntoIterator<Item = (&'static str, NixValue)>,
) -> serde_json::Value {
    let mut fields: Vec<_> = fields.into_iter().collect();
    fields.push(("graph", graph.into()));
    let artifact = support::artifact("composed", fields);
    support::compare("composed", name, artifact)
}

#[test]
fn connected_rust_graph_matches_all_default_exact_recipes() {
    let value = compare("openssl-curl-git", composition::packages(), []);
    assert_eq!(
        value["openssl"]["derivationPath"],
        "/nix/store/mqf1h79k4p5y723yvn0869ni24ifpnpn-openssl-3.3.2.drv"
    );
    assert_eq!(
        value["curl"]["derivationPath"],
        "/nix/store/m9lkswh8vqpcpvj43mlp243jqgd3ka7q-curl-8.11.0.drv"
    );
    assert_eq!(
        value["git"]["derivationPath"],
        "/nix/store/qk30rjnnsz9lxdp9b6k5hbjj7p8w17pg-git-2.47.0.drv"
    );
    assert_eq!(
        value["mariadb"]["derivationPath"],
        "/nix/store/ph4f1jlld7627mn1xqb05d357gw0l663-mariadb-server-10.11.10.drv"
    );
    assert_eq!(
        value["mariadbClient"]["derivationPath"],
        "/nix/store/gd3f8yw68jnsh5m09rkylxa093pqa7s2-mariadb-client-10.11.10.drv"
    );
}

fn tagged_graph() -> NixAttrs<Package> {
    let openssl = Nixpkgs::new().call_package(
        &composition::openssl::factory(composition::openssl::model::Release::Preview),
        composition::arguments(),
    );
    let tagged = openssl.override_attrs(|old| {
        NixAttrs::new([(
            "passthru",
            old.get("passthru")
                .merge_attrs(NixValue::record([("rusixAuthor", "Rust OpenSSL".into())])),
        )])
    });
    composition::compose(
        tagged,
        |pkgs, openssl| {
            pkgs.try_call_package(
                &composition::curl::factory(),
                composition::curl_arguments(pkgs, openssl.clone()),
            )
            .expect("fixed authoring arguments")
            .override_attrs(|old| {
                NixAttrs::new([(
                    "passthru",
                    old.get("passthru")
                        .merge_attrs(NixValue::record([("rusixAuthor", "Rust curl".into())])),
                )])
            })
        },
        composition::mariadb::model::Release::V1011.arguments(),
    )
}

#[test]
fn tagged_factory_values_prove_all_three_rust_edges() {
    let generated = support::artifact(
        "composed",
        [("graph", tagged_graph().into()), ("probe", true.into())],
    );
    let value = evaluate("tagged-edges", &generated);
    assert_eq!(value["result"]["edges"]["curlOpenSSL"], "Rust OpenSSL");
    assert_eq!(value["result"]["edges"]["gitOpenSSL"], "Rust OpenSSL");
    assert_eq!(value["result"]["edges"]["sameOpenSSL"], true);
    assert_eq!(value["result"]["edges"]["mariadbCurl"], "Rust curl");
    assert_eq!(value["result"]["upstream"], value["result"]["candidate"]);
}

#[test]
fn live_openssl_argument_override_propagates_to_the_whole_rewritten_region() {
    let pkgs = Nixpkgs::new();
    let base = compare("base", composition::packages(), []);
    let openssl = pkgs
        .call_package(
            &composition::openssl::factory(composition::openssl::model::Release::Preview),
            composition::arguments(),
        )
        .override_arguments(NixValue::record([("withZlib", true.into())]));
    let changed = compare(
        "openssl-zlib",
        composition::with_openssl(openssl),
        [("opensslArgs", NixValue::record([("withZlib", true.into())]))],
    );
    for name in ["openssl", "curl", "git", "mariadb", "mariadbClient"] {
        assert_ne!(
            base[name]["derivationPath"],
            changed[name]["derivationPath"]
        );
    }
}

#[test]
fn ordinary_openssl_also_composes_and_unused_graph_stays_lazy() {
    compare(
        "ordinary-openssl",
        composition::with_openssl(Nixpkgs::new().get("openssl").into()),
        [],
    );
    let bad = NixValue::builtin("throw").call("unused OpenSSL");
    let generated = rusix::compile(Config::new().set_dynamic("good", true).set_dynamic(
        "graph",
        composition::with_openssl(Package::from_expression(bad)),
    ))
    .unwrap();
    let session = support::session().lock().unwrap_or_else(|p| p.into_inner());
    assert_eq!(
        session
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        true
    );
}

#[test]
fn generated_graph_shares_factories_and_dependencies_lexically() {
    let generated =
        rusix::compile(Config::new().set_dynamic("graph", composition::packages())).unwrap();
    assert_eq!(generated.source.matches("openssl-3.3.2.tar.gz").count(), 0); // version is lexical interpolation
    assert_eq!(
        generated
            .source
            .matches("sha256-LopAsBl5r+i+C7+z3l3BxnCf7bRtbInBDaEUq1/D0oE=")
            .count(),
        1
    );
    assert_eq!(
        generated
            .source
            .matches("sha256-21nPDWccpuf1wsXsF3CEozp54EyX5xzxg6XN6iNQVOs=")
            .count(),
        1
    );
    assert_eq!(
        generated
            .source
            .matches("sha256-sGp0ZQuDoWqpqwmJhEgrAo51sABnSxH/KIdyxhmm8CI=")
            .count(),
        1
    );
    assert!(!generated.source.contains("deepSeq"));
    assert!(!generated.source.contains("let __rusix_arg_"));
}

#[test]
fn curl_consuming_invalid_supplied_openssl_keeps_child_boundary() {
    let generated = rusix::compile(
        Config::new().set_dynamic(
            "result",
            composition::with_openssl(Package::from_expression(1_i64.into()))
                .as_expression()
                .select("curl.drvPath"),
        ),
    )
    .unwrap();
    let error = failure(
        "curl_consuming_invalid_supplied_openssl_keeps_child_boundary",
        &generated,
    );
    assert!(!error.raw_nix.is_empty());
    assert!(
        error.origins.iter().any(|o| o
            .origin
            .as_ref()
            .is_some_and(|o| o.file.ends_with("curl-nixpkg/lowering.rs"))),
        "{error:?}"
    );
}

#[test]
fn disabled_openssl_is_not_forced_by_composition_or_overrides() {
    let bad = NixValue::builtin("throw").call("excluded supplied OpenSSL");
    let curl = composition::with_openssl(Package::from_expression(bad))
        .as_expression()
        .select("curl.override")
        .call(NixValue::record([("opensslSupport", false.into())]));
    let generated =
        rusix::compile(Config::new().set_dynamic("result", curl.select("drvPath"))).unwrap();
    assert!(
        support::session()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .evaluate_interop(&generated)
            .unwrap()
            .value["result"]
            .as_str()
            .unwrap()
            .ends_with("-curl-8.11.0.drv")
    );
}

#[test]
fn child_openssl_definition_failure_survives_the_full_graph() {
    let openssl = Nixpkgs::new().call_package(
        &composition::openssl::factory(composition::openssl::model::Release::Preview),
        NixValue::record([("fetchurl", 1_i64.into())]),
    );
    let generated = rusix::compile(
        Config::new().set_dynamic(
            "result",
            composition::with_openssl(openssl)
                .as_expression()
                .select("mariadb.drvPath"),
        ),
    )
    .unwrap();
    let error = failure(
        "child_openssl_definition_failure_survives_the_full_graph",
        &generated,
    );
    assert!(!error.raw_nix.is_empty());
    assert!(
        error.origins.iter().any(|o| o
            .origin
            .as_ref()
            .is_some_and(|o| o.file.ends_with("openssl-nixpkg/lowering.rs"))),
        "{error:?}"
    );
}

#[test]
fn mariadb_consuming_failing_rust_curl_keeps_child_definition() {
    let graph = composition::compose(
        Nixpkgs::new().get("openssl").into(),
        |pkgs, openssl| {
            pkgs.try_call_package(
                &composition::curl::factory(),
                composition::curl_arguments(pkgs, openssl.clone())
                    .with_overrides(NixAttrs::new([("fetchurl", 1_i64.into())])),
            )
            .expect("fixed authoring arguments")
        },
        composition::mariadb::model::Release::V1011.arguments(),
    );
    let generated = rusix::compile(
        Config::new().set_dynamic("result", graph.as_expression().select("mariadb.drvPath")),
    )
    .unwrap();
    let error = failure("mariadb-consuming-curl", &generated);
    assert!(error.reason.contains("call"));
    assert!(
        error.origins.iter().any(|o| o
            .origin
            .as_ref()
            .is_some_and(|o| o.file.ends_with("curl-nixpkg/lowering.rs"))),
        "{:?}",
        error.origins
    );
}

/// A pinned backend index identifies the supplied Rust dependency.
#[test]
fn delayed_stdenv_dependency_validation_recovers_supplied_child() {
    let graph = composition::compose(
        Nixpkgs::new().get("openssl").into(),
        |_, _| Package::from_expression(1_i64.into()),
        composition::mariadb::model::Release::V1011.arguments(),
    );
    let generated = rusix::compile(
        Config::new().set_dynamic("result", graph.as_expression().select("mariadb.drvPath")),
    )
    .unwrap();
    let error = failure("backend-invalid-dependency", &generated);
    assert_eq!(error.provenance, rusix::Provenance::BackendCorrelation);
    assert!(
        error
            .reason
            .contains("element 6 of buildInputs for mariadb-server")
    );
    assert!(
        error
            .primary
            .as_ref()
            .unwrap()
            .purpose
            .contains("opaque call argument")
    );
}

fn failure(name: &str, generated: &rusix::Generated) -> rusix::Diagnostic {
    let error = support::session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(generated)
        .unwrap_err();
    assert!(!error.raw_nix.is_empty());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/composed-equivalence/{name}"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("generated.nix"), &generated.source).unwrap();
    std::fs::write(
        root.join("error.json"),
        serde_json::to_vec_pretty(&error).unwrap(),
    )
    .unwrap();
    *error
}

#[test]
fn ordinary_curl_feeds_rust_mariadb_without_rewriting_its_closure() {
    let graph = composition::compose(
        Nixpkgs::new().get("openssl").into(),
        |pkgs, _| pkgs.get("curl").into(),
        composition::mariadb::model::Release::V1011.arguments(),
    );
    compare("ordinary-curl-rust-mariadb", graph, []);
}

#[test]
fn rust_openssl_feeds_git_while_curl_and_mariadb_are_unforced() {
    let openssl = Nixpkgs::new().call_package(
        &composition::openssl::factory(composition::openssl::model::Release::Preview),
        composition::arguments(),
    );
    let graph = composition::compose(
        openssl,
        |_, _| {
            Package::from_expression(NixValue::builtin("throw").call("unrequested curl/MariaDB"))
        },
        composition::mariadb::model::Release::V1011.arguments(),
    );
    compare(
        "openssl-git-only",
        graph,
        [("nodes", NixValue::list(["openssl".into(), "git".into()]))],
    );
}

#[test]
fn ordinary_nix_definitions_consume_the_tagged_rust_dependency_values() {
    let generated = support::artifact(
        "composed",
        [("graph", tagged_graph().into()), ("reverse", true.into())],
    );
    let value = evaluate("reverse-consumers", &generated);
    for name in ["curl", "git", "mariadb"] {
        assert_eq!(
            value["result"]["reverseConsumers"][name], value["result"]["candidate"][name],
            "reverse consumer {name}"
        );
    }
    assert_eq!(
        value["result"]["reverseEdges"]["curlOpenSSL"],
        "Rust OpenSSL"
    );
    assert_eq!(
        value["result"]["reverseEdges"]["gitOpenSSL"],
        "Rust OpenSSL"
    );
    assert_eq!(value["result"]["reverseEdges"]["mariadbCurl"], "Rust curl");
    assert_eq!(value["result"]["upstream"], value["result"]["candidate"]);
}

fn evaluate(name: &str, generated: &rusix::Generated) -> serde_json::Value {
    let value = support::session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(generated)
        .unwrap()
        .value;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../../target/composed-equivalence/{name}"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("generated.nix"), &generated.source).unwrap();
    std::fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
    value
}

#[test]
fn two_openssl_releases_compose_with_all_four_mariadb_releases() {
    for openssl_release in [
        composition::openssl::model::Release::Lts,
        composition::openssl::model::Release::Preview,
    ] {
        for mariadb_release in composition::mariadb::model::Release::ALL {
            let openssl = Nixpkgs::new().call_package(
                &composition::openssl::factory(openssl_release),
                composition::arguments(),
            );
            let graph = composition::compose(
                openssl,
                |pkgs, openssl| {
                    pkgs.try_call_package(
                        &composition::curl::factory(),
                        composition::curl_arguments(pkgs, openssl.clone()),
                    )
                    .expect("fixed authoring arguments")
                },
                mariadb_release.arguments(),
            );
            compare(
                &format!(
                    "{}-{}",
                    openssl_release.attribute(),
                    mariadb_release.attribute()
                ),
                graph,
                [
                    ("opensslRelease", openssl_release.attribute().into()),
                    ("mariadbRelease", mariadb_release.attribute().into()),
                ],
            );
        }
    }
}

#[test]
fn normal_mariadb_argument_override_remains_live_in_the_connected_graph() {
    let flags = NixValue::record([
        ("withEmbedded", true.into()),
        ("withStorageMroonga", false.into()),
    ]);
    let graph = NixValue::function(|g| {
        let mariadb = g.clone().select("mariadb.override").call(flags.clone());
        g.merge_attrs(NixValue::record([("mariadb", mariadb)]))
    })
    .call(composition::packages());
    compare("live-mariadb-override", graph, [("mariadbArgs", flags)]);
}

#[test]
fn excluded_openssl_stays_lazy_through_both_curl_and_mariadb() {
    let graph = composition::compose(
        Package::from_expression(NixValue::builtin("throw").call("excluded OpenSSL graph node")),
        |pkgs, openssl| {
            pkgs.try_call_package(
                &composition::curl::factory(),
                composition::curl_arguments(pkgs, openssl.clone())
                    .with_overrides(NixAttrs::new([("opensslSupport", false.into())])),
            )
            .expect("fixed authoring arguments")
        },
        composition::mariadb::model::Release::V1011.arguments(),
    );
    compare(
        "excluded-openssl-through-mariadb",
        graph,
        [
            (
                "curlArgs",
                NixValue::record([("opensslSupport", false.into())]),
            ),
            ("nodes", NixValue::list(["curl".into(), "mariadb".into()])),
        ],
    );
}
