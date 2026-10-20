#![cfg(feature = "evaluation")]

//! Compare the pinned curl factory's exact recipes, public interface and recursive tests.
use rusix::interop::raw::{NixFunctionExt, NixpkgsExt};

#[path = "../../../examples/curl-nixpkg/inputs.rs"]
mod inputs;

#[path = "../../../examples/curl-nixpkg/lowering.rs"]
mod lowering;

#[path = "../../../examples/curl-nixpkg/model.rs"]
mod model;

use rusix::interop::raw::NixRepresentation;
use rusix::{
    Config, Expr, IntoRusixValue,
    interop::{InputRef, Nixpkgs, raw::NixValue},
};
use rusix::{Diagnostic, Generated, NixSession, compile};
use std::{
    fs,
    path::Path,
    sync::{Mutex, OnceLock},
};

#[track_caller]
fn artifact(fields: impl IntoIterator<Item = (&'static str, NixValue)>) -> Generated {
    let mut arguments = vec![
        ("factory", lowering::factory().into()),
        ("nixpkgs", Nixpkgs::new().value("path")),
    ];
    arguments.extend(fields);
    let comparison = InputRef::local(
        "curl-comparison",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/curl-equivalence.nix"),
    )
    .function("compare")
    .call(NixValue::record(arguments));

    compile(Config::new().set_dynamic("result", comparison)).unwrap()
}

fn session() -> &'static Mutex<NixSession> {
    static SESSION: OnceLock<Mutex<NixSession>> = OnceLock::new();
    SESSION.get_or_init(|| Mutex::new(NixSession::new().unwrap()))
}

fn compare(
    name: &str,
    fields: impl IntoIterator<Item = (&'static str, NixValue)>,
) -> serde_json::Value {
    let generated = artifact(fields);
    let value = session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(&generated)
        .unwrap_or_else(|e| panic!("{name}: {}", e.reason))
        .value;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/curl-equivalence")
        .join(name);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("generated.nix"), generated.source).unwrap();
    fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();

    let result = &value["result"];
    if result["upstream"] != result["candidate"] {
        let upstream = result["upstream"].as_object().unwrap();
        let candidate = result["candidate"].as_object().unwrap();
        let differences: Vec<_> = upstream
            .keys()
            .filter(|key| upstream[*key] != candidate[*key])
            .collect();
        panic!(
            "{name}: differing projections {differences:?}; inspect {}",
            root.display()
        );
    }

    result["candidate"].clone()
}

fn features(fields: impl IntoIterator<Item = (&'static str, bool)>) -> NixValue {
    NixValue::record(fields.into_iter().map(|(name, value)| (name, value.into())))
}

#[test]
fn default_recipe_scripts_contexts_and_with_check_match() {
    let value = compare("default", []);
    assert_eq!(value["version"], "8.11.0");
    assert_eq!(
        value["derivationPath"],
        "/nix/store/cb2y179hgas7837a8wnx08gxawn61p1m-curl-8.11.0.drv"
    );
    assert!(value["recipe"].as_str().unwrap().starts_with("Derive("));
    assert_eq!(value["passthru"]["withCheckEnabled"], true);
}

#[test]
fn all_real_recursive_passthru_test_recipes_match() {
    compare("recursive-passthru", [("passthruDetail", true.into())]);
}

#[test]
fn public_argument_names_and_required_defaults_match() {
    let arguments = compare("arguments", [("probe", "arguments".into())]);
    let arguments = arguments.as_object().unwrap();
    assert_eq!(arguments.len(), inputs::args::argument_names().len());
    assert_eq!(
        arguments.values().filter(|value| **value == true).count(),
        18
    );
}

const TLS_FLAGS: [&str; 4] = [
    "opensslSupport",
    "gnutlsSupport",
    "wolfsslSupport",
    "rustlsSupport",
];

#[test]
fn each_independent_feature_switch_matches_in_both_directions() {
    for feature in [
        "brotliSupport",
        "c-aresSupport",
        "gsaslSupport",
        "gssSupport",
        "http2Support",
        "http3Support",
        "websocketSupport",
        "idnSupport",
        "ldapSupport",
        "pslSupport",
        "rtmpSupport",
        "scpSupport",
        "zlibSupport",
        "zstdSupport",
    ] {
        for enabled in [false, true] {
            compare(
                &format!("feature-{feature}-{enabled}"),
                [("features", features([(feature, enabled)]))],
            );
        }
    }
}

#[test]
fn every_valid_tls_selection_and_rust_model_match() {
    for (index, backend) in [
        model::TlsBackend::Disabled,
        model::TlsBackend::OpenSsl,
        model::TlsBackend::GnuTls,
        model::TlsBackend::WolfSsl,
        model::TlsBackend::Rustls,
    ]
    .into_iter()
    .enumerate()
    {
        let value = compare(
            &format!("tls-{index}"),
            [(
                "model",
                model::Curl {
                    tls: Some(backend),
                    http3: false,
                    websocket: false,
                }
                .arguments()
                .try_into_nix_value()
                .unwrap(),
            )],
        );
        assert_eq!(value["passthru"]["opensslSupport"], index == 1);
    }
    compare(
        "rust-model",
        [(
            "model",
            model::model().arguments().try_into_nix_value().unwrap(),
        )],
    );
    compare(
        "rust-model-default-tls",
        [(
            "model",
            model::Curl {
                tls: None,
                http3: false,
                websocket: false,
            }
            .arguments()
            .try_into_nix_value()
            .unwrap(),
        )],
    );
}

fn rejected(
    name: &str,
    fields: impl IntoIterator<Item = (&'static str, NixValue)> + Clone,
    reason: &str,
) -> Vec<Diagnostic> {
    let session = session().lock().unwrap_or_else(|p| p.into_inner());
    let mut errors = Vec::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/curl-equivalence")
        .join(name);
    fs::create_dir_all(&root).unwrap();

    for side in ["upstream", "candidate"] {
        let mut arguments: Vec<_> = fields.clone().into_iter().collect();
        arguments.push(("project", side.into()));
        let error = session.evaluate_interop(&artifact(arguments)).unwrap_err();
        assert!(
            error.reason.contains(reason),
            "{name}/{side}: {}",
            error.reason
        );
        assert!(!error.raw_nix.is_empty());
        fs::write(
            root.join(format!("{side}-error.json")),
            serde_json::to_vec_pretty(&error).unwrap(),
        )
        .unwrap();
        errors.push(*error);
    }
    errors
}

#[test]
fn every_invalid_tls_combination_rejects_at_the_rust_native_assertion() {
    for mask in 0_u32..16 {
        if mask.count_ones() <= 1 {
            continue;
        }
        let flags = features(
            TLS_FLAGS
                .into_iter()
                .enumerate()
                .map(|(index, key)| (key, mask & (1 << index) != 0)),
        );
        let errors = rejected(
            &format!("invalid-tls-{mask}"),
            [("features", flags)],
            "assertion",
        );
        assert!(
            errors[1]
                .origins
                .iter()
                .any(|origin| origin.origin.as_ref().is_some_and(|o| {
                    o.file.ends_with("examples/curl-nixpkg/lowering.rs")
                        && o.purpose == "Nix expression assertion"
                })),
            "{:?}",
            errors[1]
        );
    }
}

#[test]
fn reduced_full_and_dependent_default_configurations_match() {
    compare(
        "minimal",
        [(
            "features",
            features(
                [
                    "brotliSupport",
                    "c-aresSupport",
                    "gnutlsSupport",
                    "gsaslSupport",
                    "gssSupport",
                    "http2Support",
                    "http3Support",
                    "websocketSupport",
                    "idnSupport",
                    "ldapSupport",
                    "opensslSupport",
                    "pslSupport",
                    "rtmpSupport",
                    "scpSupport",
                    "wolfsslSupport",
                    "rustlsSupport",
                    "zlibSupport",
                    "zstdSupport",
                ]
                .map(|name| (name, false)),
            ),
        )],
    );
    compare(
        "full-openssl",
        [(
            "features",
            features(
                [
                    "brotliSupport",
                    "c-aresSupport",
                    "gsaslSupport",
                    "gssSupport",
                    "http2Support",
                    "http3Support",
                    "websocketSupport",
                    "idnSupport",
                    "ldapSupport",
                    "opensslSupport",
                    "pslSupport",
                    "rtmpSupport",
                    "scpSupport",
                    "zlibSupport",
                    "zstdSupport",
                ]
                .map(|name| (name, true)),
            ),
        )],
    );
    let disabled = compare(
        "dependent-zlib-disabled",
        [("override", features([("zlibSupport", false)]))],
    );
    assert_eq!(disabled["passthru"]["opensslSupport"], false);
    assert!(
        disabled["configureFlags"]
            .as_array()
            .unwrap()
            .contains(&"--without-ssl".into())
    );
    compare(
        "explicit-openssl-without-zlib",
        [(
            "features",
            features([("zlibSupport", false), ("opensslSupport", true)]),
        )],
    );
}

#[test]
fn native_cross_and_platform_specific_recipes_match() {
    for platform in ["aarch64-linux", "x86_64-darwin", "aarch64-darwin"] {
        compare(
            &format!("native-{platform}"),
            [("localSystem", platform.into())],
        );
    }
    for target in [
        "aarch64-unknown-linux-gnu",
        "x86_64-unknown-linux-musl",
        "x86_64-w64-mingw32",
        "x86_64-unknown-freebsd",
    ] {
        compare(
            &format!("cross-{target}"),
            [("crossSystem", NixValue::record([("config", target.into())]))],
        );
    }
}

#[test]
fn explicit_platform_branch_probes_keep_full_record_default_semantics() {
    for flag in ["isSunOS", "isCygwin", "isWindows", "isStatic"] {
        let value = compare(
            &format!("branch-{flag}"),
            [("hostFlags", features([(flag, true)]))],
        );
        if flag == "isSunOS" || flag == "isCygwin" {
            assert!(
                value["configureFlags"]
                    .as_array()
                    .unwrap()
                    .contains(&"--without-libssh2".into())
            );
        }
    }
    compare(
        "branch-static-psl",
        [
            ("hostFlags", features([("isStatic", true)])),
            ("features", features([("pslSupport", true)])),
        ],
    );
    compare(
        "branch-darwin-cross-default-gss",
        [
            ("localSystem", "x86_64-darwin".into()),
            (
                "hostFlags",
                NixValue::record([("rusixBranchProbe", true.into())]),
            ),
        ],
    );
    compare(
        "branch-darwin-static",
        [
            ("localSystem", "aarch64-darwin".into()),
            ("hostFlags", features([("isStatic", true)])),
        ],
    );
}

#[test]
fn final_attrs_version_and_recursive_consumer_overrides_match() {
    let value = compare(
        "override-version",
        [
            (
                "attrOverride",
                NixValue::record([
                    ("version", "8.11.0-custom".into()),
                    ("doCheck", true.into()),
                ]),
            ),
            ("passthruDetail", true.into()),
        ],
    );
    assert!(
        value["sourceUrls"][0]
            .as_str()
            .unwrap()
            .contains("8.11.0-custom")
    );
    assert_eq!(
        value["meta"]["changelog"],
        "https://curl.se/ch/8.11.0-custom.html"
    );
    assert_eq!(value["doCheck"], true);
    compare(
        "override-feature",
        [(
            "override",
            features([("http3Support", true), ("zlibSupport", false)]),
        )],
    );
    compare(
        "override-shell",
        [(
            "attrOverride",
            NixValue::record([
                ("postInstall", "echo custom\n".into()),
                ("configureFlags", NixValue::list(["--custom".into()])),
            ]),
        )],
    );
}

#[test]
fn excluded_dependencies_passthru_and_unused_factory_remain_lazy() {
    let bad: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    compare(
        "unused-dependencies",
        [(
            "features",
            NixValue::record([
                ("brotli", bad.clone()),
                ("gnutls", bad.clone()),
                ("wolfssl", bad.clone()),
                ("rustls-ffi", bad.clone()),
                ("fetchpatch", bad.clone()),
                ("coeurl", bad),
            ]),
        )],
    );
    let generated = compile(
        Config::new().set_dynamic("good", true).set_dynamic(
            "unused",
            lowering::factory()
                .as_expression()
                .call(NixValue::record([] as [(&str, NixValue); 0])),
        ),
    )
    .unwrap();
    assert_eq!(
        session()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        true
    );
}

#[test]
fn generated_defaults_are_lexical_and_contexts_remain_selective() {
    let generated = compile(Config::new().set_dynamic("factory", lowering::factory())).unwrap();
    let compact: String = generated.source.split_whitespace().collect();
    assert!(!compact.contains("let__rusix_arg_"));
    assert!(!compact.contains("deepSeq"));
    // The reused view retains the lexical platform expression as its source.
    assert!(compact.contains("(stdenv.hostPlatform).isSunOS"));
    assert!(compact.contains("stdenv.buildPlatform"));
    for span in &generated.spans {
        if generated.source[span.start..span.end]
            .trim_start_matches('(')
            .strip_prefix("builtins.addErrorContext")
            .is_some_and(|arguments| {
                arguments
                    .trim_start()
                    .starts_with(&format!("{:?}", span.origin.id))
            })
        {
            assert!(
                span.origin.purpose == "opaque Nix function call"
                    || span.origin.purpose == "Nix attribute-set union"
                    || span
                        .origin
                        .purpose
                        .starts_with("nixpkgs lib function lookup")
            );
        }
    }
}

#[test]
fn real_static_build_and_static_psl_workaround_match() {
    compare("static", [("staticBuild", true.into())]);
    let psl = compare(
        "static-psl",
        [
            ("staticBuild", true.into()),
            ("features", features([("pslSupport", true)])),
        ],
    );
    assert!(
        psl["scripts"]["preConfigure"]
            .as_str()
            .unwrap()
            .contains("LIBS=-lidn2 -lunistring")
    );
    let broken = compare(
        "static-brotli-broken-metadata",
        [
            ("staticBuild", true.into()),
            ("features", features([("brotliSupport", true)])),
        ],
    );
    assert_eq!(broken["meta"]["broken"], true);
}

#[test]
fn actual_darwin_cross_default_disables_gss() {
    let value = compare(
        "cross-darwin-arm",
        [
            ("localSystem", "x86_64-darwin".into()),
            (
                "crossSystem",
                NixValue::record([("config", "aarch64-apple-darwin".into())]),
            ),
        ],
    );
    assert!(
        !value["configureFlags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag.as_str().unwrap().starts_with("--with-gssapi="))
    );
}

#[test]
fn solaris_rejection_is_an_existing_nixpkgs_backend_limit() {
    rejected(
        "unsupported-solaris",
        [(
            "crossSystem",
            NixValue::record([("config", "x86_64-pc-solaris-gnu".into())]),
        )],
        "Unknown libc native/impure",
    );
}

fn constant_function(arity: usize, value: NixValue) -> NixValue {
    if arity == 0 {
        value
    } else {
        NixValue::function(move |_| constant_function(arity - 1, value))
    }
}

#[test]
fn supplied_library_configure_output_and_environment_helpers_remain_authoritative() {
    for (name, arity, value) in [
        ("enableFeature", 2, "--caller-enable".into()),
        ("withFeature", 2, "--caller-with".into()),
        ("withFeatureAs", 3, "--caller-with-as".into()),
        ("getDev", 1, "/caller/dev".into()),
        ("getLib", 1, "/caller/lib".into()),
        (
            "optionalAttrs",
            2,
            NixValue::record([("NIX_LDFLAGS", "-lcaller".into())]),
        ),
    ] {
        compare(
            &format!("caller-lib-{name}"),
            [(
                "libOverrides",
                NixValue::record([(name, constant_function(arity, value))]),
            )],
        );
    }
    let optional_text = NixValue::function(|condition| {
        NixValue::function(move |text| {
            NixValue::if_else(
                condition,
                rusix::nix_text!("caller:{text}", text = text),
                "",
            )
        })
    });
    compare(
        "caller-lib-optionalString",
        [(
            "libOverrides",
            NixValue::record([("optionalString", optional_text)]),
        )],
    );
    compare(
        "caller-lib-count",
        [
            (
                "features",
                features([("gnutlsSupport", true), ("opensslSupport", true)]),
            ),
            (
                "libOverrides",
                NixValue::record([("count", constant_function(2, 0_i64.into()))]),
            ),
        ],
    );
}

#[test]
fn native_assertion_does_not_use_an_overridden_throw_if_not() {
    let bypass = NixValue::function(|_| NixValue::function(|_| NixValue::function(|value| value)));
    rejected(
        "native-assert-not-lib-throw",
        [
            (
                "features",
                features([("gnutlsSupport", true), ("opensslSupport", true)]),
            ),
            ("libOverrides", NixValue::record([("throwIfNot", bypass)])),
        ],
        "assertion",
    );
}

#[test]
fn native_operations_do_not_use_unrelated_caller_library_functions() {
    let throw = NixValue::builtin("throw").call("unrelated caller library operation forced");
    compare(
        "caller-native-operations",
        [(
            "libOverrides",
            NixValue::record([
                ("concatLists", constant_function(1, throw.clone())),
                ("concatStringsSep", constant_function(2, throw.clone())),
                ("replaceStrings", constant_function(3, throw)),
            ]),
        )],
    );
}

#[test]
fn failing_argument_expression_keeps_its_rust_operation_origin() {
    let line = line!() + 1;
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let errors = rejected(
        "failing-feature-operation",
        [("features", NixValue::record([("opensslSupport", failure)]))],
        "division by zero",
    );
    assert_eq!(errors[1].primary.as_ref().unwrap().file, file!());
    assert_eq!(errors[1].primary.as_ref().unwrap().line, line);
}

#[test]
fn fetcher_and_raw_library_failures_keep_the_rust_boundary_and_nix_reason() {
    for (name, field, value, reason) in [
        (
            "fetcher",
            "features",
            NixValue::record([(
                "fetchurl",
                constant_function(1, NixValue::builtin("throw").call("caller fetcher failure")),
            )]),
            "caller fetcher failure",
        ),
        (
            "library",
            "libOverrides",
            NixValue::record([("enableFeature", 1_i64.into())]),
            "call",
        ),
    ] {
        let errors = rejected(&format!("failing-{name}"), [(field, value)], reason);
        assert!(
            errors[1]
                .origins
                .iter()
                .any(|origin| origin.origin.as_ref().is_some_and(|o| {
                    o.file.ends_with("tests/curl.rs")
                        || o.file.ends_with("examples/curl-nixpkg/lowering.rs")
                })),
            "{:?}",
            errors[1]
        );
    }
}

#[test]
fn dependent_default_and_final_attrs_failures_preserve_child_operations() {
    let session = session().lock().unwrap_or_else(|p| p.into_inner());
    let line = line!() + 1;
    let bad: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    for fields in [
        vec![("hostFlags", NixValue::record([("isWindows", bad.clone())]))],
        vec![(
            "attrOverride",
            NixValue::record([("version", bad.clone().to_text())]),
        )],
    ] {
        let mut fields = fields;
        fields.push(("project", "candidate".into()));
        let error = session.evaluate_interop(&artifact(fields)).unwrap_err();
        assert_eq!(error.reason, "division by zero");
        assert_eq!(error.primary.as_ref().unwrap().file, file!());
        assert_eq!(error.primary.as_ref().unwrap().line, line);
        assert!(!error.raw_nix.is_empty());
    }
}

#[test]
fn recursive_passthru_override_failure_keeps_a_rust_boundary() {
    let errors = rejected(
        "invalid-recursive-consumer",
        [
            ("features", NixValue::record([("coeurl", 1_i64.into())])),
            ("passthruDetail", true.into()),
        ],
        "set",
    );
    assert!(
        errors[1].origins.iter().any(|origin| origin
            .origin
            .as_ref()
            .is_some_and(|o| { o.file.ends_with("examples/curl-nixpkg/lowering.rs") })),
        "{:?}",
        errors[1]
    );
}

#[test]
fn rust_call_package_matches_exact_default_and_model_derivations() {
    let default = compare("rust-call-package-default", []);
    let model = compare(
        "rust-call-package-model",
        [(
            "model",
            model::model().arguments().try_into_nix_value().unwrap(),
        )],
    );
    let factory = lowering::factory();
    let pkgs = Nixpkgs::new();
    let default_package =
        pkgs.call_package(&factory, NixValue::record([] as [(&str, NixValue); 0]));
    let model_package = pkgs
        .try_call_package(&factory, model::model().arguments())
        .expect("fixed authoring arguments");
    let generated = compile(
        Config::new()
            .set_dynamic(
                "default",
                default_package.field::<rusix::Expr<String>>("drvPath"),
            )
            .set_dynamic(
                "model",
                model_package.field::<rusix::Expr<String>>("drvPath"),
            ),
    )
    .unwrap();
    let value = session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(&generated)
        .unwrap()
        .value;
    assert_eq!(value["default"], default["derivationPath"]);
    assert_eq!(value["model"], model["derivationPath"]);
}
