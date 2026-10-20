#![cfg(feature = "evaluation")]

use rusix::interop::raw::NixpkgsExt;

#[path = "../../../examples/openssl-nixpkg/inputs.rs"]
mod inputs;

#[path = "../../../examples/openssl-nixpkg/lowering.rs"]
mod lowering;

#[path = "../../../examples/openssl-nixpkg/model.rs"]
mod model;

#[path = "../../../examples/openssl-nixpkg/scripts.rs"]
mod scripts;

#[path = "support/packages.rs"]
mod support;

use model::Release;
use rusix::{Config, Expr, interop::raw::NixValue};

fn artifact(
    release: Release,
    fields: impl IntoIterator<Item = (&'static str, NixValue)>,
) -> rusix::Generated {
    let mut fields: Vec<_> = fields.into_iter().collect();
    fields.extend([
        ("factory", lowering::factory(release).into()),
        ("family", lowering::family_factory().into()),
        ("release", release.attribute().into()),
    ]);
    support::artifact("openssl", fields)
}

fn compare(
    name: &str,
    release: Release,
    fields: impl IntoIterator<Item = (&'static str, NixValue)>,
) -> serde_json::Value {
    support::compare(
        "openssl",
        &format!("{}-{name}", release.attribute()),
        artifact(release, fields),
    )
}

#[test]
fn releases_recipes_outputs_sources_metadata_tests_and_family_match() {
    for release in Release::ALL {
        let value = compare("default", release, [("passthruDetail", true.into())]);
        assert_eq!(value["version"], release.version());
        compare("family", release, [("probe", "family".into())]);
        let args = compare("arguments", release, [("probe", "arguments".into())]);
        assert_eq!(
            args.as_object().unwrap().len(),
            inputs::args::argument_names().len()
        );
        assert_eq!(
            args.as_object()
                .unwrap()
                .values()
                .filter(|x| **x == true)
                .count(),
            8
        );
    }
}

#[test]
fn each_feature_and_conf_overrides_match_for_every_release() {
    for release in Release::ALL {
        for flag in [
            "withCryptodev",
            "withZlib",
            "enableSSL2",
            "enableSSL3",
            "enableMD2",
            "enableKTLS",
            "static",
        ] {
            for enabled in [false, true] {
                compare(
                    &format!("{flag}-{enabled}"),
                    release,
                    [("features", NixValue::record([(flag, enabled.into())]))],
                );
            }
        }
        compare(
            "conf",
            release,
            [(
                "override",
                NixValue::record([("conf", "/caller/openssl.cnf".into())]),
            )],
        );
        compare(
            "override-attrs",
            release,
            [
                (
                    "attrOverride",
                    NixValue::record([
                        ("version", "99.0".into()),
                        ("postInstall", "echo override\n".into()),
                    ]),
                ),
                ("passthruDetail", true.into()),
            ],
        );
    }
}

#[test]
fn native_static_cross_platforms_and_branch_targets_match() {
    for release in Release::ALL {
        for system in [
            "aarch64-linux",
            "x86_64-darwin",
            "aarch64-darwin",
            "armv7l-linux",
            "powerpc64-linux",
            "riscv64-linux",
        ] {
            compare(system, release, [("localSystem", system.into())]);
        }
        compare("static", release, [("staticBuild", true.into())]);
        for config in [
            "aarch64-unknown-linux-gnu",
            "x86_64-unknown-linux-musl",
            "x86_64-w64-mingw32",
            "x86_64-unknown-freebsd",
            "riscv32-unknown-linux-gnu",
            "mips64-unknown-linux-gnuabi64",
        ] {
            compare(
                config,
                release,
                [("crossSystem", NixValue::record([("config", config.into())]))],
            );
        }
    }
}

#[test]
fn lazy_unused_dependencies_and_final_attrs_keep_original_semantics() {
    let bad: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    compare(
        "excluded",
        Release::Preview,
        [(
            "features",
            NixValue::record([
                ("cryptodev", bad.clone()),
                ("zlib", bad.clone()),
                ("coreutils", bad.clone()),
                ("writeShellScript", bad),
            ]),
        )],
    );
    let value = compare(
        "lexical-version",
        Release::Preview,
        [(
            "attrOverride",
            NixValue::record([("version", "99.0".into())]),
        )],
    );
    assert!(value["sourceUrls"][0].as_str().unwrap().contains("3.3.2"));
    let generated =
        rusix::compile(Config::new().set_dynamic("factory", lowering::factory(Release::Preview)))
            .unwrap();
    assert!(!generated.source.contains("deepSeq"));
    assert!(!generated.source.contains("let __rusix_arg_"));
}

#[test]
fn source_fetcher_failure_maps_to_openssl_definition() {
    let fetcher: NixValue = 1_i64.into();
    let generated = artifact(
        Release::Preview,
        [
            ("features", NixValue::record([("fetchurl", fetcher)])),
            ("project", "candidate".into()),
        ],
    );
    let error = support::session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(error.reason.contains("call"));
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
fn unsupported_configuration_rejects_with_definition_provenance() {
    let fields = NixValue::record([
        ("system", "rusix-unsupported".into()),
        ("isBSD", false.into()),
        ("isMinGW", false.into()),
        ("isLinux", false.into()),
        ("isiOS", false.into()),
    ]);
    for side in ["upstream", "candidate"] {
        let generated = artifact(
            Release::Preview,
            [("hostFlags", fields.clone()), ("project", side.into())],
        );
        let error = support::session()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .evaluate_interop(&generated)
            .unwrap_err();
        assert!(error.reason.contains("Not sure what configuration to use"));
        assert!(!error.raw_nix.is_empty());
        if side == "candidate" {
            assert!(
                error
                    .primary
                    .as_ref()
                    .unwrap()
                    .file
                    .ends_with("openssl-nixpkg/lowering.rs")
            );
        }
    }
}

#[test]
fn native_lookups_ignore_unrelated_library_overrides() {
    let bad = NixValue::builtin("throw").call("native lookup must not use library helper");
    compare(
        "native-lookups",
        Release::Preview,
        [(
            "features",
            NixValue::record([(
                "lib",
                rusix::interop::Nixpkgs::new()
                    .value("lib")
                    .merge_attrs(NixValue::record([
                        ("attrByPath", bad.clone()),
                        ("hasAttrByPath", bad),
                    ])),
            )]),
        )],
    );
}

#[test]
fn legacy_unused_ktls_default_remains_lazy() {
    let bad = NixValue::builtin("throw").call("irrelevant KTLS platform default");
    compare(
        "legacy-lazy-ktls",
        Release::Legacy,
        [("hostFlags", NixValue::record([("isLinux", bad)]))],
    );
}
