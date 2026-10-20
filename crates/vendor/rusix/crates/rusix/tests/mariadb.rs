#![cfg(feature = "evaluation")]

use rusix::interop::raw::NixRepresentation;

#[path = "../../../examples/mariadb-nixpkg/inputs.rs"]
mod inputs;

#[path = "../../../examples/mariadb-nixpkg/lowering.rs"]
mod lowering;

#[path = "../../../examples/mariadb-nixpkg/model.rs"]
pub mod model;

#[path = "../../../examples/mariadb-nixpkg/scripts.rs"]
mod scripts;

#[path = "support/packages.rs"]
mod support;

use model::Release;
use rusix::{
    Config, Expr,
    interop::{NixAttrs, raw::NixValue},
};

fn artifact(
    release: Release,
    fields: impl IntoIterator<Item = (&'static str, NixValue)>,
) -> rusix::Generated {
    let mut fields: Vec<_> = fields.into_iter().collect();
    fields.extend([
        ("factory", lowering::factory().into()),
        ("family", lowering::family().into()),
        ("release", release.attribute().into()),
        ("version", release.version().into()),
        ("hash", release.hash().into()),
    ]);
    support::artifact("mariadb", fields)
}

fn compare(
    name: &str,
    release: Release,
    fields: impl IntoIterator<Item = (&'static str, NixValue)>,
) -> serde_json::Value {
    support::compare(
        "mariadb",
        &format!("{}-{name}", release.attribute()),
        artifact(release, fields),
    )
}

#[test]
fn default_client_server_recipes_and_generic_interface_match_every_release() {
    for release in Release::ALL {
        let value = compare("default", release, []);
        assert_eq!(value["server"]["version"], release.version());
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
            4
        );
    }
    compare("family", Release::V1011, [("probe", "family".into())]);
}

#[test]
fn all_sixteen_storage_embedded_numa_combinations_match_every_release() {
    for release in Release::ALL {
        for mask in 0..16 {
            let flags = NixValue::record(
                [
                    "withStorageMroonga",
                    "withStorageRocks",
                    "withEmbedded",
                    "withNuma",
                ]
                .into_iter()
                .enumerate()
                .map(|(n, key)| (key, (mask & (1 << n) != 0).into())),
            );
            compare(&format!("features-{mask}"), release, [("override", flags)]);
        }
    }
}

#[test]
fn native_and_cross_platforms_match_both_client_and_server() {
    for release in Release::ALL {
        for platform in [
            "aarch64-linux",
            "i686-linux",
            "x86_64-darwin",
            "aarch64-darwin",
        ] {
            compare(platform, release, [("localSystem", platform.into())]);
        }
        for target in ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-musl"] {
            compare(
                target,
                release,
                [("crossSystem", NixValue::record([("config", target.into())]))],
            );
        }
    }
}

#[test]
fn overrides_keep_common_lexical_version_and_original_client_server_members() {
    for release in Release::ALL {
        let changed = compare(
            "attr-override",
            release,
            [
                (
                    "attrOverride",
                    NixValue::record([
                        ("version", "99.0".into()),
                        ("postInstall", "echo changed\n".into()),
                    ]),
                ),
                (
                    "clientAttrOverride",
                    NixValue::record([("cmakeFlags", NixValue::list(["-DCALLER=ON".into()]))]),
                ),
            ],
        );
        assert!(
            changed["server"]["sourceUrls"][0]
                .as_str()
                .unwrap()
                .contains(release.version())
        );
        assert_eq!(changed["client"]["version"], release.version());
        compare(
            "version-argument",
            release,
            [(
                "override",
                NixValue::record([("version", "10.7.99".into())]),
            )],
        );
    }
}

#[test]
fn supported_recursive_nixos_passthru_recipes_match() {
    for release in Release::ALL {
        compare("passthru", release, [("passthruDetail", true.into())]);
    }
}

#[test]
fn excluded_dependencies_defaults_and_client_server_siblings_remain_lazy() {
    let bad: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    compare(
        "excluded",
        Release::V1011,
        [(
            "features",
            NixValue::record([
                ("withStorageMroonga", false.into()),
                ("withNuma", false.into()),
                ("kytea", bad.clone()),
                ("libsodium", bad.clone()),
                ("msgpack", bad.clone()),
                ("zeromq", bad.clone()),
                ("numactl", bad.clone()),
                ("libaio", bad.clone()),
                ("fixDarwinDylibNames", bad.clone()),
                ("cctools", bad.clone()),
                ("perl", bad.clone()),
                ("libedit", bad.clone()),
            ]),
        )],
    );
    let package = rusix::interop::Nixpkgs::new()
        .try_call_package(
            &lowering::factory(),
            NixAttrs::try_from_record(Release::V1011.arguments())
                .unwrap()
                .merge(NixAttrs::from_expression(NixValue::record([
                    ("bison", bad),
                    (
                        "withStorageMroonga",
                        NixValue::builtin("throw").call("server-only default"),
                    ),
                ]))),
        )
        .expect("fixed authoring arguments");
    let generated = rusix::compile(Config::new().set_dynamic(
        "client",
        package.field::<rusix::Expr<String>>("client.drvPath"),
    ))
    .unwrap();
    support::session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(&generated)
        .unwrap();
    let source = rusix::compile(Config::new().set_dynamic("factory", lowering::factory()))
        .unwrap()
        .source;
    assert!(!source.contains("deepSeq"));
    assert!(!source.contains("let __rusix_arg_"));
}

#[test]
fn fetcher_failure_and_unsupported_solaris_keep_raw_diagnostics() {
    for side in ["upstream", "candidate"] {
        let generated = artifact(
            Release::V1011,
            [
                ("features", NixValue::record([("fetchurl", 1_i64.into())])),
                ("project", side.into()),
            ],
        );
        let error = support::session()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .evaluate_interop(&generated)
            .unwrap_err();
        assert!(error.reason.contains("call"));
        assert!(!error.raw_nix.is_empty());
        if side == "candidate" {
            assert!(
                error.origins.iter().any(|o| o
                    .origin
                    .as_ref()
                    .is_some_and(|o| o.file.ends_with("mariadb-nixpkg/lowering.rs"))),
                "{error:?}"
            );
        }
        let generated = artifact(
            Release::V1011,
            [
                (
                    "crossSystem",
                    NixValue::record([("config", "x86_64-pc-solaris-gnu".into())]),
                ),
                ("project", side.into()),
            ],
        );
        let error = support::session()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .evaluate_interop(&generated)
            .unwrap_err();
        assert!(error.reason.contains("Unknown libc native/impure"));
    }
}

#[test]
fn pinned_passthru_autobackup_and_freebsd_emulator_limits_reject_on_both_sides() {
    for release in Release::ALL {
        for (name, fields, reason) in [
            (
                "autobackup",
                vec![
                    ("passthruDetail", true.into()),
                    ("testName", "mysql-autobackup".into()),
                ],
                "automysqlbackup",
            ),
            (
                "freebsd",
                vec![(
                    "crossSystem",
                    NixValue::record([("config", "x86_64-unknown-freebsd".into())]),
                )],
                "Don't know how to run x86_64-unknown-freebsd executables",
            ),
        ] {
            for side in ["upstream", "candidate"] {
                let mut fields = fields.clone();
                fields.push(("project", side.into()));
                let generated = artifact(release, fields);
                let error = support::session()
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .evaluate_interop(&generated)
                    .unwrap_err();
                assert!(error.reason.contains(reason), "{error:?}");
                assert!(!error.raw_nix.is_empty());
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../target/mariadb-equivalence/{}-{name}",
                    release.attribute()
                ));
                std::fs::create_dir_all(&path).unwrap();
                std::fs::write(
                    path.join(format!("{side}-error.json")),
                    serde_json::to_vec_pretty(&error).unwrap(),
                )
                .unwrap();
            }
        }
    }
}
