//! Compilation must change only artifact metadata, never generated Nix.
#[allow(dead_code, unused_imports)] // This audit uses only a subset of the shared package models.
#[path = "../../../../examples/composed-packages/composition.rs"]
pub mod composition;

use super::*;
use crate::interop::{
    Nixpkgs,
    raw::{NixRepresentation, NixpkgsExt},
};
use std::{fs, path::Path};

#[test]
fn backend_metadata_has_zero_generated_cost() {
    let pkgs = Nixpkgs::new();
    let cases = [
        (
            "git",
            Config::new().set_dynamic(
                "package",
                pkgs.try_call_package(&composition::git::factory(), composition::git::arguments())
                    .unwrap(),
            ),
        ),
        (
            "curl",
            Config::new().set_dynamic(
                "package",
                pkgs.try_call_package(
                    &composition::curl::factory(),
                    composition::curl::model::model().arguments(),
                )
                .unwrap(),
            ),
        ),
        (
            "openssl",
            Config::new().set_dynamic(
                "package",
                pkgs.call_package(
                    &composition::openssl::factory(composition::openssl::model::Release::Preview),
                    composition::arguments(),
                ),
            ),
        ),
        (
            "mariadb",
            Config::new().set_dynamic(
                "package",
                pkgs.try_call_package(
                    &composition::mariadb::factory(),
                    composition::mariadb::model::Release::V1011.arguments(),
                )
                .unwrap(),
            ),
        ),
        (
            "graph",
            Config::new().set_dynamic(
                "result",
                composition::packages()
                    .as_expression()
                    .select("mariadb.drvPath"),
            ),
        ),
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/backend-correlation/cost");
    fs::create_dir_all(&root).unwrap();
    let mut observations = Vec::new();
    for (name, config) in cases {
        for comments in [false, true] {
            let options = RenderOptions {
                origin_comments: comments,
                ..Default::default()
            };
            let before = render_with_options(&lower(&config), options);
            let after = compile_with_options(&config, options).unwrap();
            assert_eq!(before.source, after.source, "{name}/{comments}");
            assert_eq!(
                serde_json::to_value(&before.spans).unwrap(),
                serde_json::to_value(&after.spans).unwrap()
            );
            assert!(after.backend_metadata.is_some(), "{name}");
            if !comments {
                let metadata = provenance::read(&after.backend_metadata).unwrap();
                observations.push(serde_json::json!({
                    "case": name,
                    "before_bytes": before.source.len(), "after_bytes": after.source.len(),
                    "before_lines": before.source.lines().count(), "after_lines": after.source.lines().count(),
                    "before_contexts": before.source.matches("addErrorContext").count(),
                    "after_contexts": after.source.matches("addErrorContext").count(),
                    "backend_argument_wrappers": 0,
                    "metadata_bytes": serde_json::to_vec(&metadata).unwrap().len(),
                    "boundaries": metadata.boundaries.len(),
                }));
                fs::write(root.join(format!("{name}.nix")), &after.source).unwrap();
            }
        }
    }
    fs::write(
        root.join("observations.json"),
        serde_json::to_vec_pretty(&observations).unwrap(),
    )
    .unwrap();
}

#[test]
fn compact_lexical_lists_cannot_expand_metadata_without_bound() {
    use crate::interop::raw::NixValue;

    // Nix's lexical graph is small while the represented list is large.
    let mut inputs = NixValue::list([1_i64.into()]);
    for _ in 0..16 {
        inputs =
            NixValue::function(|list| NixValue::concat_lists([list.clone(), list])).call(inputs);
    }
    let pkg = Nixpkgs::new()
        .value("stdenv.mkDerivation")
        .call(NixValue::record([
            ("name", "bounded-list".into()),
            ("buildInputs", inputs),
        ]));
    let config = Config::new().set_dynamic("result", pkg.select("drvPath"));
    let generated = compile(&config).unwrap();
    let metadata = provenance::read(&generated.backend_metadata).unwrap();
    let field = &metadata.boundaries[0].fields[0];
    assert!(!field.complete);
    assert!(!field.children.is_empty());
    assert!(field.children.len() < 65_536);
    assert_eq!(generated.source, render(&lower(&config)).source);
}
