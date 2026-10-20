#![cfg(feature = "evaluation")]

//! Compares complete recipes and deferred package behavior without fetching or building.
use rusix::interop::raw::{NixFunctionExt, NixpkgsExt};

#[path = "../../../examples/git-nixpkg/inputs.rs"]
pub mod inputs;

#[path = "../../../examples/git-nixpkg/lowering.rs"]
mod lowering;

#[path = "../../../examples/git-nixpkg/model.rs"]
mod model;

use rusix::interop::raw::NixRepresentation;
use rusix::{
    Config, Expr,
    interop::{InputRef, Nixpkgs, raw::NixValue},
};
use rusix::{Generated, NixSession, RenderOptions, compile, compile_with_options};
use std::{
    fs,
    path::Path,
    sync::{Mutex, OnceLock},
};

#[track_caller]
fn artifact(fields: impl IntoIterator<Item = (&'static str, NixValue)>) -> Generated {
    artifact_with_options(fields, RenderOptions::default())
}

#[track_caller]
fn artifact_with_options(
    fields: impl IntoIterator<Item = (&'static str, NixValue)>,
    options: RenderOptions,
) -> Generated {
    let mut args = vec![
        ("factory", lowering::factory().into()),
        ("nixpkgs", Nixpkgs::new().value("path")),
    ];
    args.extend(fields);
    let comparison = InputRef::local(
        "git-comparison",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/git-equivalence.nix"),
    )
    .function("compare")
    .call(NixValue::record(args));
    compile_with_options(Config::new().set_dynamic("result", comparison), options).unwrap()
}

fn session() -> &'static Mutex<NixSession> {
    static SESSION: OnceLock<Mutex<NixSession>> = OnceLock::new();

    SESSION.get_or_init(|| Mutex::new(NixSession::new().unwrap()))
}

fn compare(name: &str, fields: impl IntoIterator<Item = (&'static str, NixValue)>) {
    let artifact = artifact(fields);
    let value = session()
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .evaluate_interop(&artifact)
        .unwrap_or_else(|e| panic!("{name}: {}", e.reason))
        .value;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/git-equivalence")
        .join(name);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("generated.nix"), &artifact.source).unwrap();
    fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
    let result = &value["result"];
    if result["upstream"] != result["candidate"] {
        let a = result["upstream"].as_object().unwrap();
        let b = result["candidate"].as_object().unwrap();
        let differences: Vec<_> = a.keys().filter(|k| a[*k] != b[*k]).collect();
        panic!(
            "{name}: differing projections {differences:?}; inspect {}",
            root.display()
        );
    }
    assert!(
        result["upstream"]["version"]
            .as_str()
            .unwrap()
            .starts_with("2.47.0")
    );
    assert!(
        result["upstream"]["recipe"]
            .as_str()
            .unwrap()
            .starts_with("Derive(")
    );
}

#[test]
fn default_git_has_identical_derivation_and_passthru() {
    compare("default", []);
}

#[test]
fn normal_and_inspection_git_factories_have_identical_complete_projections() {
    let artifacts = [
        RenderOptions::default(),
        RenderOptions {
            origin_comments: true,
            ..Default::default()
        },
    ]
    .map(|options| artifact_with_options([], options));
    assert!(!artifacts[0].source.contains("# rn-"));
    assert_eq!(
        artifacts[1].source.matches("# rn-").count(),
        artifacts[1].spans.len()
    );
    assert_eq!(
        artifacts[0].source.matches("addErrorContext").count(),
        artifacts[1].source.matches("addErrorContext").count()
    );
    let session = session().lock().unwrap_or_else(|p| p.into_inner());
    let [normal, debug] =
        artifacts.map(|generated| session.evaluate_interop(&generated).unwrap().value);

    assert_eq!(normal, debug);
    assert_eq!(normal["result"]["upstream"], normal["result"]["candidate"]);
}

#[test]
fn generated_git_factory_does_not_reconstruct_native_argument_records() {
    let generated = compile(Config::new().set_dynamic("factory", lowering::factory())).unwrap();
    let compact: String = generated.source.split_whitespace().collect();

    // Check the entire factory, including all dependent defaults and callbacks.
    // Ignore whitespace rather than snapshotting the renderer's layout.
    assert!(!compact.contains("let__rusix_arg_"));
    for name in [
        "fetchurl",
        "fetchpatch",
        "stdenv",
        "buildPackages",
        "curl",
        "tests",
    ] {
        assert!(!compact.contains(&format!("\"{name}\"={name};")));
    }
    assert!(compact.contains(".hostPlatform).isDarwin"));
    assert!(compact.contains(".buildPlatform"));
    assert!(compact.contains(".perl.libPrefix"));
}

#[test]
fn generated_git_contexts_cover_boundaries_not_routine_expression_structure() {
    let generated = compile(Config::new().set_dynamic("factory", lowering::factory())).unwrap();
    let boundaries: Vec<_> = generated
        .spans
        .iter()
        .filter(|span| {
            generated.source[span.start..span.end]
                .trim_start_matches('(')
                .strip_prefix("builtins.addErrorContext")
                .is_some_and(|arguments| {
                    arguments
                        .trim_start()
                        .starts_with(&format!("{:?}", span.origin.id))
                })
        })
        .collect();

    assert!(!boundaries.is_empty());
    for span in boundaries {
        let arguments = generated.source[span.start..span.end]
            .trim_start_matches('(')
            .strip_prefix("builtins.addErrorContext")
            .unwrap()
            .trim_start();
        let value = arguments
            .strip_prefix(&format!("{:?}", span.origin.id))
            .unwrap();
        assert!(value.starts_with(char::is_whitespace));
        assert!(!value.trim_start().is_empty());
        let purpose = &span.origin.purpose;
        assert!(
            purpose == "opaque Nix function call"
                // Native union can reject an opaque operand supplied by the caller.
                || purpose == "Nix attribute-set union"
                || purpose.starts_with("nixpkgs lib function lookup ")
                || purpose.starts_with("nixpkgs function lookup ")
                || purpose.starts_with("nixpkgs package lookup "),
            "unexpected structural runtime context: {purpose}",
        );
    }
}

#[test]
fn caller_library_all_override_does_not_change_native_boolean_decisions() {
    // Upstream uses && for the name and linker decisions, not lib.all.
    // These decisions must remain independent of a replacement library function.
    let library = Nixpkgs::new().function("recursiveUpdate").apply([
        Nixpkgs::new().value("lib"),
        NixValue::record([(
            "all",
            NixValue::function(|_| NixValue::function(|_| false.into())),
        )]),
    ]);
    let mut fields: Vec<_> = [
        "svnSupport",
        "guiSupport",
        "sendEmailSupport",
        "withManual",
        "pythonSupport",
        "withpcre2",
    ]
    .map(|name| (name, false.into()))
    .into();
    fields.push(("lib", library));

    compare(
        "caller-library-override",
        [("features", NixValue::record(fields))],
    );
}

#[test]
fn caller_library_any_and_merge_overrides_do_not_change_native_operations() {
    let library = Nixpkgs::new().function("recursiveUpdate").apply([
        Nixpkgs::new().value("lib"),
        rusix::nix_record! {
            "any": NixValue::function(|_| NixValue::function(|_| false.into())),
            "mergeAttrs": NixValue::function(|_| NixValue::function(|_| rusix::nix_record! {})),
        },
    ]);

    for system in ["x86_64-linux", "aarch64-darwin"] {
        compare(
            &format!("caller-native-operations-{system}"),
            [
                ("localSystem", system.into()),
                ("features", rusix::nix_record! { "lib": library.clone() }),
            ],
        );
    }
}

#[test]
fn native_assertions_do_not_use_an_overridden_throw_if_not() {
    let bypass = NixValue::function(|_| NixValue::function(|_| NixValue::function(|value| value)));
    let library = Nixpkgs::new().function("recursiveUpdate").apply([
        Nixpkgs::new().value("lib"),
        rusix::nix_record! { "throwIfNot": bypass },
    ]);
    let session = session().lock().unwrap_or_else(|p| p.into_inner());

    for (fields, feature) in [
        (vec![("osxkeychainSupport", true)], "osxkeychainSupport"),
        (
            vec![("perlSupport", false), ("sendEmailSupport", true)],
            "sendEmailSupport",
        ),
        (
            vec![("perlSupport", false), ("svnSupport", true)],
            "svnSupport",
        ),
    ] {
        let features = features(fields).merge_attrs(rusix::nix_record! { "lib": library.clone() });

        for side in ["upstream", "candidate"] {
            let generated = artifact([("features", features.clone()), ("project", side.into())]);
            let error = session.evaluate_interop(&generated).unwrap_err();
            assert!(error.reason.contains("assertion"), "{}", error.reason);
            assert!(error.reason.contains(feature), "{}", error.reason);
            assert!(!error.raw_nix.is_empty());
            if side == "candidate" {
                assert!(error.origins.iter().any(|origin| {
                    origin.origin.as_ref().is_some_and(|origin| {
                        origin.file.ends_with("examples/git-nixpkg/lowering.rs")
                            && origin.purpose == "Nix expression assertion"
                    })
                }));
            }
        }
    }
}

fn features(fields: impl IntoIterator<Item = (&'static str, bool)>) -> NixValue {
    NixValue::record(fields.into_iter().map(|(k, v)| (k, v.into())))
}

macro_rules! feature_case {
    ($name:ident, $($key:literal = $value:expr),+ $(,)?) => {
        #[test]
        fn $name() {
            compare(stringify!($name), [("features", features([$(($key, $value)),+]))]);
        }
    };
}

feature_case!(
    minimal_git,
    "withManual" = false,
    "pythonSupport" = false,
    "perlSupport" = false,
    "withpcre2" = false
);

feature_case!(perl_disabled, "perlSupport" = false);

feature_case!(
    perl_enabled_email_disabled,
    "perlSupport" = true,
    "sendEmailSupport" = false
);

feature_case!(svn_enabled, "svnSupport" = true);

feature_case!(
    svn_enabled_email_disabled,
    "svnSupport" = true,
    "sendEmailSupport" = false
);

feature_case!(email_enabled, "sendEmailSupport" = true);

feature_case!(email_disabled, "sendEmailSupport" = false);

feature_case!(pcre_disabled, "withpcre2" = false);

feature_case!(manual_disabled, "withManual" = false);

feature_case!(python_disabled, "pythonSupport" = false);

feature_case!(translations_disabled, "nlsSupport" = false);

feature_case!(gui_enabled_linux, "guiSupport" = true);

feature_case!(secret_enabled, "withLibsecret" = true);

feature_case!(ssh_enabled, "withSsh" = true);

feature_case!(
    full_git,
    "svnSupport" = true,
    "guiSupport" = true,
    "sendEmailSupport" = true,
    "withSsh" = true,
    "withLibsecret" = true
);

feature_case!(install_checks_disabled, "doInstallCheck" = false);

feature_case!(install_checks_enabled, "doInstallCheck" = true);

#[test]
fn caller_supplied_optional_text_override_controls_both_factories() {
    let library = Nixpkgs::new().function("recursiveUpdate").apply([
        Nixpkgs::new().value("lib"),
        NixValue::record([(
            "optionalString",
            NixValue::function(|condition| {
                NixValue::function(move |text| {
                    NixValue::if_else(
                        condition,
                        rusix::nix_text!("caller-{text}", text = text),
                        "",
                    )
                })
            }),
        )]),
    ]);

    compare(
        "caller-optional-text-override",
        [("features", NixValue::record([("lib", library)]))],
    );
}

#[test]
fn darwin_intel_default() {
    compare("darwin-intel", [("localSystem", "x86_64-darwin".into())]);
}

#[test]
fn darwin_arm_default() {
    compare("darwin-arm", [("localSystem", "aarch64-darwin".into())]);
}

#[test]
fn darwin_gui_patch_and_tk() {
    compare(
        "darwin-gui",
        [
            ("localSystem", "aarch64-darwin".into()),
            ("features", features([("guiSupport", true)])),
        ],
    );
}

#[test]
fn darwin_keychain_disabled() {
    compare(
        "darwin-no-keychain",
        [
            ("localSystem", "x86_64-darwin".into()),
            ("features", features([("osxkeychainSupport", false)])),
        ],
    );
}

#[test]
fn linux_arm_native() {
    compare("linux-arm", [("localSystem", "aarch64-linux".into())]);
}

#[test]
fn linux_to_arm_cross() {
    compare(
        "cross-arm",
        [(
            "crossSystem",
            NixValue::record([("config", "aarch64-unknown-linux-gnu".into())]),
        )],
    );
}

#[test]
fn linux_to_musl_cross() {
    compare(
        "cross-musl",
        [(
            "crossSystem",
            NixValue::record([("config", "x86_64-unknown-linux-musl".into())]),
        )],
    );
}

#[test]
fn linux_to_mingw_cross() {
    compare(
        "cross-mingw",
        [(
            "crossSystem",
            NixValue::record([("config", "x86_64-w64-mingw32".into())]),
        )],
    );
}

#[test]
fn ordinary_nix_function_override_keeps_dependent_defaults() {
    compare(
        "override-perl",
        [(
            "override",
            features([("perlSupport", false), ("withManual", false)]),
        )],
    );
}

#[test]
fn ordinary_override_attrs_and_final_package_passthru() {
    compare(
        "override-attrs",
        [(
            "attrOverride",
            NixValue::record([
                ("version", "2.47.0-overridden".into()),
                ("doInstallCheck", false.into()),
                ("makeFlags", NixValue::list(["CUSTOM_FLAG=1".into()])),
                ("postInstall", "echo overridden\n".into()),
            ]),
        )],
    );
}

#[test]
fn rust_native_model_matches_ordinary_nix_consumers() {
    // This invokes the model's normal callPackage path, not the comparison fixture.
    let value = Nixpkgs::new()
        .try_call_package(&lowering::factory(), inputs::arguments(model::model()))
        .expect("fixed authoring arguments")
        .field::<Expr<String>>("drvPath");
    let artifact = compile(Config::new().set_dynamic("result", value)).unwrap();
    let session = session().lock().unwrap_or_else(|p| p.into_inner());
    let rust = session.evaluate_interop(&artifact).unwrap().value;
    let reference = artifact_for_model();
    let comparison = session.evaluate_interop(&reference).unwrap().value;
    assert_eq!(
        rust["result"],
        comparison["result"]["upstream"]["derivationPath"]
    );

    // The stronger enum can still select both Perl-dependent features explicitly.
    let full_model = model::Git {
        perl: model::Perl::Enabled {
            svn: true,
            send_email: true,
        },
        ..model::Git::defaults()
    };
    let full = Nixpkgs::new()
        .try_call_package(&lowering::factory(), inputs::arguments(full_model))
        .expect("fixed authoring arguments")
        .field::<Expr<String>>("pname");
    assert_eq!(
        session
            .evaluate_interop(&compile(Config::new().set_dynamic("result", full)).unwrap())
            .unwrap()
            .value["result"],
        "git-with-svn"
    );
}

fn artifact_for_model() -> Generated {
    artifact([(
        "features",
        features([
            ("withManual", false),
            ("pythonSupport", false),
            ("perlSupport", false),
            ("withpcre2", false),
            ("doInstallCheck", false),
        ]),
    )])
}

#[test]
fn linux_to_freebsd_cross() {
    compare(
        "cross-freebsd",
        [(
            "crossSystem",
            NixValue::record([("config", "x86_64-unknown-freebsd".into())]),
        )],
    );
}

#[test]
fn solaris_rejection_is_shared_nixpkgs_limitation() {
    let session = session().lock().unwrap_or_else(|p| p.into_inner());

    for side in ["upstream", "candidate"] {
        let generated = artifact([
            (
                "crossSystem",
                NixValue::record([("config", "x86_64-pc-solaris-gnu".into())]),
            ),
            ("project", side.into()),
        ]);
        let error = session.evaluate_interop(&generated).unwrap_err();
        assert!(
            error.reason.contains("Unknown libc native/impure"),
            "{}",
            error.reason
        );
        assert!(!error.raw_nix.is_empty());
    }
}

#[test]
fn sunos_make_flags_branch_with_explicit_host_flag_probe() {
    compare(
        "sunos-branch-probe",
        [("hostFlags", NixValue::record([("isSunOS", true.into())]))],
    );
}

#[test]
fn cross_build_with_explicit_perl_features() {
    compare(
        "cross-perl",
        [
            (
                "crossSystem",
                NixValue::record([("config", "aarch64-unknown-linux-gnu".into())]),
            ),
            (
                "features",
                features([("perlSupport", true), ("svnSupport", true)]),
            ),
        ],
    );
}

#[test]
fn invalid_public_feature_combinations_reject_and_retain_rust_provenance() {
    let session = session().lock().unwrap_or_else(|p| p.into_inner());

    for (name, fields, reason) in [
        (
            "keychain-on-linux",
            vec![("osxkeychainSupport", true)],
            "osxkeychainSupport",
        ),
        (
            "email-without-perl",
            vec![("perlSupport", false), ("sendEmailSupport", true)],
            "sendEmailSupport",
        ),
        (
            "svn-without-perl",
            vec![("perlSupport", false), ("svnSupport", true)],
            "svnSupport",
        ),
        (
            "all-feature-checks-fail-first-wins",
            vec![
                ("osxkeychainSupport", true),
                ("perlSupport", false),
                ("sendEmailSupport", true),
                ("svnSupport", true),
            ],
            "osxkeychainSupport",
        ),
        (
            "email-and-svn-checks-fail-second-wins",
            vec![
                ("perlSupport", false),
                ("sendEmailSupport", true),
                ("svnSupport", true),
            ],
            "sendEmailSupport",
        ),
    ] {
        let mut errors = Vec::new();

        for side in ["upstream", "candidate"] {
            let artifact = artifact([
                ("features", features(fields.clone())),
                ("project", side.into()),
            ]);
            let error = session.evaluate_interop(&artifact).unwrap_err();
            assert!(!error.raw_nix.is_empty(), "{name}: missing upstream trace");
            assert!(error.reason.contains(reason), "{name}: {}", error.reason);
            errors.push(error);
        }

        // Upstream's assertion belongs to Nix; candidate's boundary retains its Rust source.
        let candidate = &errors[1];
        assert!(
            candidate.origins.iter().any(|o| o
                .origin
                .as_ref()
                .is_some_and(|o| o.file.ends_with("examples/git-nixpkg/lowering.rs"))),
            "{name}: {candidate:?}"
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/git-equivalence")
            .join(name);
        fs::create_dir_all(&root).unwrap();
        for (side, error) in ["upstream", "candidate"].into_iter().zip(errors) {
            fs::write(
                root.join(format!("{side}-error.json")),
                serde_json::to_vec_pretty(&error).unwrap(),
            )
            .unwrap();
        }
    }
}

#[test]
fn invalid_structured_dependency_maps_to_rust_lookup_or_call() {
    let session = session().lock().unwrap_or_else(|p| p.into_inner());
    let fields = NixValue::record([("curl", NixValue::record([("notAPackage", true.into())]))]);

    for side in ["upstream", "candidate"] {
        let boundary_line = line!() + 1;
        let artifact = artifact([("features", fields.clone()), ("project", side.into())]);
        let error = session.evaluate_interop(&artifact).unwrap_err();
        assert!(!error.raw_nix.is_empty());
        assert!(
            error.reason.contains("notAPackage")
                || error.reason.contains("string")
                || (error.reason.contains("Dependency") && error.reason.contains("buildInputs")),
            "{}",
            error.reason
        );
        if side == "candidate" {
            // stdenv rejects a dependency while later forcing the returned derivation.
            // Its trace can recover the Rust crossing boundary, not the internal cause.
            let origin = error.primary.as_ref().expect("Rust boundary origin");
            assert!(origin.file.ends_with("tests/git.rs"));
            assert_eq!(origin.line, boundary_line);
        }
    }
}

#[test]
fn unused_package_and_unused_dependency_remain_lazy() {
    let unused = lowering::factory()
        .as_expression()
        .call(NixValue::record([] as [(&str, NixValue); 0]));
    let generated = compile(
        Config::new()
            .set_dynamic("good", true)
            .set_dynamic("unused", unused),
    )
    .unwrap();
    let session = session().lock().unwrap_or_else(|p| p.into_inner());
    session
        .evaluate_interop(&compile(Config::new().set_dynamic("staged", true)).unwrap())
        .unwrap();
    assert_eq!(
        session
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        true
    );

    // SVN and GUI inputs need not be valid values when those features are disabled.
    let bad = rusix::Expr::int(1).divide(rusix::Expr::int(0));
    compare_without_lock(
        &session,
        artifact([(
            "features",
            NixValue::record([("subversionClient", bad.clone().into()), ("tk", bad.into())]),
        )]),
    );
}

fn compare_without_lock(session: &NixSession, generated: Generated) {
    let value = session.evaluate_interop(&generated).unwrap().value;
    assert_eq!(value["result"]["upstream"], value["result"]["candidate"]);
}

#[test]
fn rust_call_package_preserves_the_exact_default_derivation() {
    let factory = lowering::factory();
    let git = Nixpkgs::new()
        .try_call_package(&factory, inputs::arguments(model::Git::defaults()))
        .expect("fixed authoring arguments");
    let generated =
        compile(Config::new().set_dynamic("result", git.field::<Expr<String>>("drvPath"))).unwrap();
    let reference = artifact([]);
    let session = session().lock().unwrap_or_else(|p| p.into_inner());
    let value = session.evaluate_interop(&generated).unwrap().value;
    let comparison = session.evaluate_interop(&reference).unwrap().value;
    assert_eq!(
        value["result"],
        comparison["result"]["upstream"]["derivationPath"]
    );
    assert_eq!(
        value["result"],
        comparison["result"]["candidate"]["derivationPath"]
    );
}
