#![cfg(feature = "evaluation")]

//! Caller-owned Nix projects can use basic authoring without staging Rusix's nixpkgs.
use rusix::interop::{NixPath, raw::NixValue};
use rusix::{Config, IntoRusixValue, NixSession, compile, nix_text};
use std::collections::BTreeMap;

#[test]
fn text_templates_need_only_builtins_and_retain_context_and_laziness() {
    let file = NixValue::builtin("toFile").apply(["dependency".into(), "contents".into()]);
    let text = nix_text!(
        "prefix:{file}/suffix",
        file = file.clone().into_expr::<String>()
    );
    let generated = compile(
        Config::new()
            .set_dynamic("text", text.clone())
            .set_dynamic("context", NixValue::builtin("getContext").call(text))
            .set_dynamic("original", NixValue::builtin("getContext").call(file))
            .set_dynamic("unused", rusix::Expr::int(1).divide(0.into())),
    )
    .unwrap();
    let session = NixSession::new().unwrap();
    let context = session
        .evaluate_attribute(&generated, "context")
        .unwrap()
        .value;
    let original = session
        .evaluate_attribute(&generated, "original")
        .unwrap()
        .value;
    assert!(!context.as_object().unwrap().is_empty());
    assert_eq!(context, original);
    assert!(!session.root().join("nixpkgs").exists());
    assert!(!generated.source.contains("import"));
}

#[derive(IntoRusixValue)]
#[rusix(rename_all = "preserve")]
struct Preferences {
    /// Literal Qubes preference name, with its original underscore.
    provides_network: bool,
    /// A raw Rust identifier loses only the Rust-specific prefix.
    r#type: u32,
    /// Dynamic literal names must not become dotted attribute paths.
    service_flags: BTreeMap<String, bool>,
}

#[test]
fn preserved_names_and_ordered_maps_compose_as_argument_records() {
    let value = Preferences {
        provides_network: true,
        r#type: u32::MAX,
        service_flags: BTreeMap::from([
            ("api.v1".into(), true),
            ("${throw \"injection\"}".into(), false),
        ]),
    }
    .try_into_nix_value()
    .unwrap();
    let generated = compile(Config::new().set_dynamic("prefs", value)).unwrap();
    let result = NixSession::new().unwrap().evaluate(&generated).unwrap();
    assert_eq!(
        result.value,
        serde_json::json!({"prefs": {
            "provides_network": true, "type": 4294967295_u64,
            "service_flags": {"api.v1": true, "${throw \"injection\"}": false}
        }})
    );
}

#[test]
fn relative_paths_remain_paths_and_reject_traversal_before_evaluation() {
    let root: NixPath = rusix::interop::Nixpkgs::new().source_path("nixos");
    for invalid in [
        "",
        "/absolute",
        "../outside",
        "a/../b",
        "a//b",
        "./a",
        "a/",
        "nul\0",
    ] {
        assert!(root.join_relative(invalid).is_err(), "{invalid:?}");
    }
    let path = root
        .join_relative("modules/a space-${literal}.nix")
        .unwrap();
    let generated = compile(
        Config::new()
            .set_dynamic("path", path.clone())
            .set_dynamic("isPath", NixValue::builtin("isPath").call(path)),
    )
    .unwrap();
    let result = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap();
    assert_eq!(result.value["isPath"], true);
    assert!(
        result.value["path"]
            .as_str()
            .unwrap()
            .ends_with("/nixos/modules/a space-${literal}.nix")
    );
}

#[test]
fn saved_module_metadata_translates_external_evaluation_and_keeps_compiler_failures_unmapped() {
    use rusix::{
        DiagnosticKind, Provenance,
        nixos::{NixosArtifact, NixosModule, compile_module},
    };
    let first = NixosModule::new(Config::new().set_dynamic("services.demo.value", "first"));
    let second = NixosModule::new(Config::new().set_dynamic("services.demo.value", "second"));
    let artifact = compile_module(&first.module(second)).unwrap();
    let saved: NixosArtifact =
        serde_json::from_slice(&serde_json::to_vec(&artifact).unwrap()).unwrap();
    let session = NixSession::new().unwrap();
    let driver = rusix::Generated {
        source: r#"let lib = import ./nixpkgs/lib; in (lib.evalModules {
          modules = [ ./module.nix { options.services.demo.value = lib.mkOption { type = lib.types.str; }; } ];
        }).config.services.demo.value"#.into(),
        ..Default::default()
    };
    let failure = session
        .evaluate_nixos_with_driver(&artifact, &driver)
        .unwrap_err();
    let mapped = saved.diagnose(
        DiagnosticKind::NixEval,
        &failure.raw_nix,
        &session.root().join("module.nix"),
        &session.root().join("nixpkgs"),
    );
    assert_eq!(mapped.kind, DiagnosticKind::NixosMerge);
    assert_eq!(mapped.provenance, Provenance::ModuleDefinition);
    assert_eq!(mapped.origins.len(), 2);
    assert_eq!(mapped.raw_nix, failure.raw_nix);
    assert!(mapped.related.iter().all(|origin| origin.file == file!()));

    let syntax = saved.diagnose(
        DiagnosticKind::Compiler,
        &failure.raw_nix,
        &session.root().join("module.nix"),
        &session.root().join("nixpkgs"),
    );
    assert!(syntax.primary.is_none());
}
