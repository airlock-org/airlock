#![cfg(feature = "evaluation")]

//! A genuine overlay modifies upstream curl, using the ordinary nixpkgs fixed point.
#[path = "../../../examples/overlay/authoring.rs"]
mod authoring;

#[path = "../../../examples/overlay/inputs.rs"]
mod inputs;

#[path = "../../../examples/overlay/model.rs"]
mod model;

use rusix::{
    Config, Expr,
    interop::{
        InputRef, NixAttrs, NixCallable, NixList, Nixpkgs, Overlay, Package, PackageFunction,
        ToNixText,
        raw::{AsNixValue, NixFunctionExt, NixValue, NixpkgsExt},
    },
};
use rusix::{DiagnosticKind, Generated, NixSession, compile};
use std::{fs, path::Path};

fn reference() -> InputRef {
    InputRef::local(
        "overlay-reference",
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/overlay-reference.nix"),
    )
}

fn ordinary(overlay: Option<NixValue>) -> NixValue {
    reference().function("reference").call(NixValue::record([
        ("nixpkgs", Nixpkgs::new().value("path")),
        ("overlay", overlay.into()),
    ]))
}

fn inspect(pkgs: NixValue) -> NixValue {
    reference().function("inspect").call(pkgs)
}

fn save(name: &str, generated: &Generated, value: &serde_json::Value) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/overlay-equivalence")
        .join(name);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("generated.nix"), &generated.source).unwrap();
    fs::write(
        root.join("source-map.json"),
        serde_json::to_vec_pretty(generated).unwrap(),
    )
    .unwrap();
    fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(value).unwrap(),
    )
    .unwrap();
}

fn evaluate(name: &str, config: Config) -> serde_json::Value {
    let generated = compile(&config).unwrap();
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_or_else(|error| panic!("{name}: {error:?}"))
        .value;
    save(name, &generated, &value);
    value
}

#[test]
fn ordinary_baseline_and_handwritten_overlay_establish_the_reference() {
    let generated = compile(
        Config::new()
            .set_dynamic("baseline", inspect(Nixpkgs::new().as_value()))
            .set_dynamic("ordinary", inspect(ordinary(None)))
            .set_dynamic(
                "reference",
                inspect(ordinary(Some(reference().overlay("overlay").into()))),
            ),
    )
    .unwrap();
    let value = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap()
        .value;
    save("reference", &generated, &value);
    assert_eq!(value["baseline"], value["ordinary"]);
    let baseline = &value["baseline"];
    let modified = &value["reference"];
    let mut expected_flags = baseline["configureFlags"].as_array().unwrap().clone();
    assert!(!expected_flags.contains(&"--disable-dict".into()));
    expected_flags.push("--disable-dict".into());
    assert_eq!(
        modified["configureFlags"],
        serde_json::json!(expected_flags)
    );
    assert_ne!(baseline["derivationPath"], modified["derivationPath"]);
    assert_ne!(baseline["recipe"], modified["recipe"]);
    assert_eq!(baseline["sourceRecipe"], modified["sourceRecipe"]);
    assert_eq!(baseline["hello"], modified["hello"]);
    assert_eq!(modified["downstream"]["curl"], modified["derivationPath"]);
    assert_ne!(
        baseline["downstream"]["derivationPath"],
        modified["downstream"]["derivationPath"]
    );
}

#[test]
fn rust_overlay_matches_the_handwritten_recipe_and_downstream_dependency_exactly() {
    let value = evaluate(
        "rust",
        Config::new()
            .set_dynamic("baseline", inspect(ordinary(None)))
            .set_dynamic(
                "reference",
                inspect(ordinary(Some(reference().overlay("overlay").into()))),
            )
            .set_dynamic(
                "rustImported",
                inspect(ordinary(Some(authoring::overlay().into()))),
            )
            .set_dynamic("rust", inspect(authoring::package_set().as_value())),
    );
    assert_eq!(value["rust"], value["reference"]);
    assert_eq!(value["rustImported"], value["reference"]);
    let baseline = &value["baseline"];
    let modified = &value["rust"];
    assert_eq!(modified["downstream"]["curl"], modified["derivationPath"]);
    assert_ne!(
        baseline["downstream"]["recipe"],
        modified["downstream"]["recipe"]
    );

    // The complete recipe changes only its flags and the resulting output identities.
    let mut expected_recipe = baseline["recipe"].as_str().unwrap().to_owned();
    let env_field = |value: &serde_json::Value| {
        format!(
            "(\"configureFlags\",{})",
            serde_json::to_string(&value["configureFlagsEnv"]).unwrap()
        )
    };
    assert!(expected_recipe.contains(&env_field(baseline)));
    expected_recipe = expected_recipe.replace(&env_field(baseline), &env_field(modified));
    for (output, path) in baseline["outputPaths"].as_object().unwrap() {
        expected_recipe = expected_recipe.replace(
            path.as_str().unwrap(),
            modified["outputPaths"][output].as_str().unwrap(),
        );
    }
    assert_eq!(expected_recipe, modified["recipe"].as_str().unwrap());
    assert_eq!(baseline["source"], modified["source"]);
    assert_eq!(baseline["hello"], modified["hello"]);
}

#[test]
fn declared_views_and_the_output_model_select_metadata_without_forcing_the_overlay_function() {
    let pkgs = authoring::package_set().view::<inputs::Packages>();
    let output = model::Output {
        overlay: authoring::overlay(),
        curl_derivation: pkgs.curl().view::<inputs::PackageMetadata>().drv_path(),
    };
    let output = NixAttrs::try_from_record(output).unwrap();
    let value = evaluate(
        "declared-views",
        Config::new()
            .set_dynamic("rust", output.field::<Expr<String>>("curlDerivation"))
            .set_dynamic(
                "reference",
                ordinary(Some(reference().overlay("overlay").into())).select("curl.drvPath"),
            ),
    );
    assert_eq!(value["rust"], value["reference"]);
}

#[test]
fn structured_overlay_and_attribute_callbacks_return_conversion_errors_immediately() {
    #[derive(rusix::IntoRusixValue)]
    struct Invalid {
        #[rusix(flatten)]
        fields: bool,
    }

    let mut builds = 0;
    let error = Overlay::try_from_function(|_: inputs::Packages, _: inputs::Packages| {
        builds += 1;
        Invalid { fields: false }
    })
    .unwrap_err();
    assert_eq!(builds, 1);
    assert!(error.message.contains("flatten"));

    let package: Package = Nixpkgs::new().get("rusixMissingPackage").into();
    let error = package
        .try_override_attrs(|_: inputs::BuildAttrs| {
            builds += 1;
            Invalid { fields: false }
        })
        .unwrap_err();
    assert_eq!(builds, 2);
    assert!(error.message.contains("flatten"));
}

#[test]
fn prev_modifies_the_previous_package_once_per_overlay_without_self_recursion() {
    let twice = authoring::package_set().with_overlay(authoring::overlay());
    let reference_twice = ordinary(Some(reference().overlay("overlay").into()))
        .select("extend")
        .call(reference().overlay("overlay").as_value());
    let value = evaluate(
        "prev",
        Config::new()
            .set_dynamic("baseline", inspect(ordinary(None)))
            .set_dynamic("once", inspect(authoring::package_set().as_value()))
            .set_dynamic("twice", inspect(twice.as_value()))
            .set_dynamic(
                "mixed",
                inspect(
                    Nixpkgs::new()
                        .with_overlay(reference().overlay("overlay"))
                        .with_overlay(authoring::overlay())
                        .as_value(),
                ),
            )
            .set_dynamic("referenceTwice", inspect(reference_twice)),
    );
    assert_eq!(value["twice"], value["referenceTwice"]);
    assert_eq!(value["mixed"], value["referenceTwice"]);
    let mut flags = value["baseline"]["configureFlags"]
        .as_array()
        .unwrap()
        .clone();
    flags.push("--disable-dict".into());
    assert_eq!(value["once"]["configureFlags"], serde_json::json!(flags));
    flags.push("--disable-dict".into());
    assert_eq!(value["twice"]["configureFlags"], serde_json::json!(flags));
    assert_ne!(
        value["once"]["derivationPath"],
        value["twice"]["derivationPath"]
    );
}

#[test]
fn final_sees_a_later_overlay_while_prev_keeps_the_preceding_package() {
    let probe = Overlay::from_function(|final_pkgs, prev_pkgs| {
        let final_curl: Package = final_pkgs.field("curl");
        let prev_curl: Package = prev_pkgs.field("curl");
        NixAttrs::new([
            (
                "rusixFinalCurl",
                final_curl.field::<Expr<String>>("drvPath").into(),
            ),
            (
                "rusixPreviousCurl",
                prev_curl.field::<Expr<String>>("drvPath").into(),
            ),
        ])
    });
    let pkgs = Nixpkgs::new()
        .with_overlay(probe)
        .with_overlay(authoring::overlay());
    let value = evaluate(
        "final",
        Config::new()
            .set_dynamic("baseline", Nixpkgs::new().value("curl.drvPath"))
            .set_dynamic("modified", pkgs.value("curl.drvPath"))
            .set_dynamic("final", pkgs.value("rusixFinalCurl"))
            .set_dynamic("prev", pkgs.value("rusixPreviousCurl")),
    );
    assert_eq!(value["final"], value["modified"]);
    assert_eq!(value["prev"], value["baseline"]);
    assert_ne!(value["final"], value["prev"]);
}

#[test]
fn unrelated_failing_package_set_attributes_remain_lazy() {
    let poison = Overlay::from_function(|_, _| {
        NixAttrs::new([(
            "rusixUnusedPackage",
            NixValue::builtin("throw").call("unused overlay attribute evaluated"),
        )])
    });
    let pkgs = Nixpkgs::new()
        .with_overlay(poison)
        .with_overlay(authoring::overlay());
    let generated = compile(
        Config::new()
            .set_dynamic("curl", pkgs.value("curl.drvPath"))
            .set_dynamic("hello", pkgs.value("hello.drvPath"))
            .set_dynamic("ordinaryHello", Nixpkgs::new().value("hello.drvPath")),
    )
    .unwrap();
    assert!(!generated.source.contains("deepSeq"));
    assert!(!generated.source.contains("builtins.seq"));
    let session = NixSession::new().unwrap();
    let value = session.evaluate_interop(&generated).unwrap().value;
    save("lazy", &generated, &value);
    assert_eq!(value["hello"], value["ordinaryHello"]);
    let demanded =
        compile(Config::new().set_dynamic("bad", pkgs.value("rusixUnusedPackage"))).unwrap();
    let error = session.evaluate_interop(&demanded).unwrap_err();
    assert!(error.reason.contains("unused overlay attribute evaluated"));
}

#[test]
fn failure_inside_override_attrs_maps_to_the_rust_operation() {
    let operation_line = std::cell::Cell::new(0);
    let overlay = Overlay::from_function(|_, prev| {
        let curl: Package = prev.field("curl");
        let curl = curl.override_attrs(|old| {
            operation_line.set(line!() + 1);
            let bad = Expr::int(1).divide(Expr::int(0));
            let flags: NixList<Expr<String>> = old.field("configureFlags");
            NixAttrs::new([(
                "configureFlags",
                NixList::concat([flags, NixList::new([bad.to_nix_text()])]).into(),
            )])
        });
        NixAttrs::new([("curl", curl.into())])
    });
    let pkgs = Nixpkgs::new().with_overlay(overlay);
    let generated =
        compile(Config::new().set_dynamic("flags", pkgs.value("curl.configureFlags"))).unwrap();
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::NixEval);
    assert_eq!(error.reason, "division by zero");
    let origin = error.primary.as_ref().unwrap();
    assert_eq!(origin.file, file!());
    assert_eq!(origin.line, operation_line.get());
    assert_eq!(origin.purpose, "integer division");
    assert!(!error.raw_nix.is_empty());
    save(
        "provenance",
        &generated,
        &serde_json::to_value(&error).unwrap(),
    );
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/overlay-equivalence/provenance");
    fs::write(root.join("nix.stderr"), &error.raw_nix).unwrap();
}

#[test]
fn overlays_capture_outer_callback_parameters_and_reject_escaped_ones() {
    let mut builds = 0;
    let label = NixCallable::<Expr<String>, Expr<String>>::from_function(|label| {
        let overlay = Overlay::from_function(|_, _| {
            builds += 1;
            NixAttrs::new([("rusixLabel", label.into())])
        });
        Nixpkgs::new()
            .with_overlay(overlay)
            .value("rusixLabel")
            .into_expr()
    });
    assert_eq!(builds, 1);
    let value = evaluate(
        "captured",
        Config::new().set_dynamic("label", label.call("captured")),
    );
    assert_eq!(value["label"], "captured");

    let named = PackageFunction::<Expr<String>>::from_function_attrs(["label"], |args| {
        let overlay =
            Overlay::from_function(|_, _| NixAttrs::new([("rusixLabel", args.select("label"))]));
        (
            Vec::<(&str, NixValue)>::new(),
            Nixpkgs::new()
                .with_overlay(overlay)
                .value("rusixLabel")
                .into_expr(),
        )
    });
    let value = evaluate(
        "named-capture",
        Config::new().set_dynamic(
            "label",
            named.call(NixAttrs::new([("label", NixValue::from("named"))])),
        ),
    );
    assert_eq!(value["label"], "named");

    let mut escaped = None;
    let _callback = NixCallable::<Expr<String>, Expr<String>>::from_function(|label| {
        escaped = Some(Overlay::from_function(|_, _| {
            NixAttrs::new([("rusixLabel", label.into())])
        }));
        "unused".into()
    });
    let generated = Config::new().set_dynamic(
        "label",
        Nixpkgs::new()
            .with_overlay(escaped.unwrap())
            .value("rusixLabel"),
    );
    let error = compile(&generated).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);
    assert!(error.reason.contains("escaped its function scope"));
}

#[test]
fn overlay_bodies_cannot_hide_invalid_records_or_nixos_scoped_references() {
    let invalid = Overlay::from_function(|_, _| {
        NixAttrs::new([("duplicate", NixValue::from(1)), ("duplicate", 2.into())])
    });
    let error = compile(
        Config::new().set_dynamic("package", Nixpkgs::new().with_overlay(invalid).get("hello")),
    )
    .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);
    assert!(error.reason.contains("duplicate record field"));

    let option = Overlay::from_function(|_, _| {
        NixAttrs::new([(
            "label",
            rusix::nixos::OptionRef::<String>::new("example.label")
                .into_expr()
                .into(),
        )])
    });
    let error = compile(
        Config::new().set_dynamic("label", Nixpkgs::new().with_overlay(option).value("label")),
    )
    .unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Validation);
    assert!(
        error
            .reason
            .contains("option references require NixosModule")
    );
}

#[test]
fn authored_overlays_extend_the_module_package_set_and_can_read_final_options() {
    use rusix::nixos::compile_module;
    use rusix::nixos::{NixosModule, OptionDecl, OptionRef, OptionType};

    #[derive(rusix::IntoConfig)]
    struct Schema {
        #[rusix(rename = "rusixPrefix")]
        prefix: OptionDecl,
    }

    let overlay = Overlay::from_function(|_, prev| {
        let hello: Package = prev.field("hello");
        let label = Expr::concat([
            OptionRef::<String>::new("rusixPrefix").into_expr(),
            hello.field("pname"),
        ]);
        NixAttrs::new([("rusixLabel", label.into())])
    });
    let pkgs = Nixpkgs::from_module().with_overlay(overlay);
    let module = NixosModule::new(
        Config::new()
            .set_dynamic("rusixPrefix", "module-")
            .set_dynamic("environment.rusixResult", pkgs.value("rusixLabel")),
    )
    .declare(Schema {
        prefix: OptionDecl::new(OptionType::named("str")),
    });
    let value = NixSession::new()
        .unwrap()
        .evaluate_nixos_interop(
            &compile_module(&module).unwrap(),
            &["environment", "rusixResult"],
            false,
        )
        .unwrap()
        .value;
    assert_eq!(value, "module-hello");
}
