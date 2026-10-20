#![cfg(feature = "evaluation")]

//! Typed authoring uses the same IR, lazy evaluator and diagnostic boundaries.
use rusix::interop::raw::NixRepresentation;
use rusix::interop::raw::{AsNixValue, NixFunctionExt, NixpkgsExt};
use rusix::{
    Config, Expr, IntoRusixValue,
    interop::{
        NixAttrs, NixCallable, NixExpression, NixList, NixNullable, NixPath, Nixpkgs, Package,
        PackageFunction, raw::NixValue,
    },
};
use rusix::{NixSession, compile};

#[rusix::args]
mod views {
    use rusix::interop::{NixAttrs, NixCallable, NixLibrary, NixList, Package, Stdenv};

    #[rusix(root)]
    struct Inputs {
        package: Package,
        lib: NixLibrary,
        builder: NixCallable<Package>,
        stdenv: Stdenv,
        dependencies: NixList<Package>,
        family: NixAttrs<Package>,
        recipe: Recipe,
    }

    #[rusix(value)]
    struct Recipe {
        name: String,
        enabled: bool,
    }
}

fn evaluate(value: impl rusix::ConfigValue) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", value)).unwrap())
        .unwrap()
        .value["result"]
        .clone()
}

#[test]
fn typed_bindings_preserve_factory_interfaces_defaults_and_family_results() {
    let factory: PackageFunction<Expr<String>> =
        PackageFunction::from_function_attrs(["name", "unused"], |args| {
            (
                vec![("unused", Expr::int(1).divide(Expr::int(0)).into())],
                args.select("name").into_expr(),
            )
        });
    let family = factory.bind(|factory| {
        NixAttrs::new([(
            "member",
            factory.call(NixValue::record([("name", "bound".into())])),
        )])
    });
    assert_eq!(evaluate(family.get("member")), "bound");

    let factory: PackageFunction<NixAttrs<Expr<String>>> =
        PackageFunction::from_function_attrs(["name"], |args| {
            (
                Vec::<(&str, NixValue)>::new(),
                NixAttrs::new([("member", args.select("name").into_expr())]),
            )
        });
    let family =
        Nixpkgs::new().call_package(&factory, NixValue::record([("name", "family".into())]));
    assert_eq!(evaluate(family.get("member")), "family");
}

#[test]
fn bindings_and_callbacks_keep_custom_finite_views() {
    let recipe = views::Recipe::from_expression(NixValue::record([
        ("name", "kept".into()),
        ("enabled", true.into()),
    ]));
    let result: Expr<String> =
        recipe.bind(|recipe| Expr::choose(recipe.enabled(), recipe.name(), "excluded".into()));
    assert_eq!(evaluate(result), "kept");
    let callable = NixCallable::from_function(|recipe: views::Recipe| recipe.name());
    assert_eq!(
        evaluate(
            callable.call(views::Recipe::from_expression(NixValue::record([(
                "name",
                "callback".into()
            )])))
        ),
        "callback"
    );
}

#[test]
fn typed_lists_and_text_leave_excluded_failures_lazy() {
    let lib = Nixpkgs::new().library();
    let bad: Expr<String> = NixValue::builtin("throw").call("excluded").into_expr();
    let list = NixList::concat([
        NixList::new(["first".into()]),
        NixList::optional(&lib, false, bad.clone()),
        NixList::new([bad.clone()]).when(&lib, false),
    ]);
    assert_eq!(evaluate(list), serde_json::json!(["first"]));
    assert_eq!(
        evaluate(Expr::concat(["text".into(), bad.when(&lib, false)])),
        "text"
    );
}

#[derive(IntoRusixValue)]
struct Arguments {
    name: String,
}

#[derive(IntoRusixValue)]
struct InvalidArguments {
    #[rusix(flatten)]
    name: String,
}

#[test]
fn structured_arguments_lower_only_at_calls_and_return_conversion_errors() {
    let callable =
        NixCallable::from_function(|args: NixValue| args.select("name").into_expr::<String>());
    assert_eq!(
        evaluate(
            callable
                .try_call(Arguments {
                    name: "record".into()
                })
                .unwrap()
        ),
        "record"
    );
    assert!(
        callable
            .try_call(InvalidArguments {
                name: "invalid".into()
            })
            .is_err()
    );
    let factory: PackageFunction<Expr<String>> =
        PackageFunction::from_function_attrs(["name"], |args| {
            (
                Vec::<(&str, NixValue)>::new(),
                args.select("name").into_expr(),
            )
        });
    assert_eq!(
        evaluate(
            Nixpkgs::new()
                .try_call_package(
                    &factory,
                    Arguments {
                        name: "factory".into()
                    }
                )
                .unwrap()
        ),
        "factory"
    );
    assert!(
        Nixpkgs::new()
            .try_call_package(
                &factory,
                InvalidArguments {
                    name: "invalid".into()
                }
            )
            .is_err()
    );
}

#[test]
fn package_overrides_outputs_and_existing_references_preserve_package_type() {
    let package: Package = Nixpkgs::new().get("openssl").into();
    let package = package.override_arguments(NixValue::record([("withZlib", true.into())]));
    let package =
        package.override_attrs(|old| old.merge(NixAttrs::new([("version", "typed-probe".into())])));
    assert_eq!(
        evaluate(package.field::<Expr<String>>("version")),
        "typed-probe"
    );
    assert_eq!(
        evaluate(package.output("dev").field::<Expr<String>>("outputName")),
        "dev"
    );
    let output = Nixpkgs::new()
        .pkgs_function("lib.getDev")
        .returning::<Package>()
        .call(package);
    assert_eq!(evaluate(output.field::<Expr<String>>("outputName")), "dev");
}

#[test]
fn typed_binding_shares_source_and_preserves_child_failure_provenance() {
    let bad: Package =
        Package::from_expression(NixValue::builtin("throw").call("typed child failure"));
    let result = bad.bind(|package| package.field::<Expr<String>>("name"));
    let generated = compile(Config::new().set_dynamic("result", result)).unwrap();
    assert_eq!(generated.source.matches("typed child failure").count(), 1);
    let error = NixSession::new()
        .unwrap()
        .evaluate_interop(&generated)
        .unwrap_err();
    assert!(error.reason.contains("typed child failure"));
    assert!(format!("{error:?}").contains("typed_interop.rs"));
}

#[test]
fn typed_argument_accessors_keep_packages_callables_scopes_and_lists() {
    let pkgs = Nixpkgs::new();
    let package: Package = pkgs.get("openssl").into();
    let input = views::from_value(rusix::nix_record! {
        "package": package.clone(),
        "lib": pkgs.library(),
        "builder": pkgs.pkgs_function("stdenv.mkDerivation").returning::<Package>(),
        "stdenv": rusix::nix_record! {
            "buildPlatform": rusix::nix_record! { "system": "native" },
            "hostPlatform": rusix::nix_record! { "system": "native" },
        },
        "dependencies": NixList::new([package.clone()]),
        "family": NixAttrs::new([("openssl", package)]),
        "recipe": rusix::nix_record! { "name": "view", "enabled": true },
    });
    let built = input.builder().call(rusix::nix_record! {
        "name": "typed-view", "buildCommand": "", "version": "probe",
    });
    let result = rusix::nix_record! {
        "package": input.package().field::<Expr<String>>("version"),
        "family": input.family().get("openssl").field::<Expr<String>>("version"),
        "builder": built.field::<Expr<String>>("version"),
        "equal": input.stdenv().build_host_equal(),
        "length": NixValue::builtin("length").call(input.dependencies()),
        "recipe": input.recipe.name().require(&input.lib(), input.recipe.enabled(), "disabled"),
    };
    assert_eq!(
        evaluate(result),
        serde_json::json!({
            "package": "3.3.2", "family": "3.3.2", "builder": "probe", "equal": true,
            "length": 1, "recipe": "view",
        })
    );
}

#[test]
fn structured_callback_results_lower_inside_the_function_boundary() {
    let function: NixCallable<NixAttrs, Expr<String>> =
        NixCallable::try_from_function(|name: Expr<String>| {
            #[derive(IntoRusixValue)]
            struct ResultRecord {
                name: Expr<String>,
            }

            ResultRecord { name }
        })
        .unwrap();
    assert_eq!(
        evaluate(function.call("typed").field::<Expr<String>>("name")),
        "typed"
    );
    assert!(
        NixCallable::<NixAttrs, Expr<String>>::try_from_function(|_: Expr<String>| {
            InvalidArguments {
                name: "invalid".into(),
            }
        })
        .is_err()
    );
}

#[test]
fn typed_text_keeps_package_output_string_context() {
    let source = Nixpkgs::new().get("openssl").as_value().to_text();
    let text = Expr::<String>::concat(["prefix:".into(), source.clone().into_expr()]);
    let result = NixValue::builtin("getContext")
        .call(source)
        .equals(NixValue::builtin("getContext").call(text));
    assert_eq!(evaluate(result), true);
}

#[test]
fn structured_bindings_preserve_views_and_reject_before_building_the_callback() {
    #[derive(IntoRusixValue)]
    struct Recipe {
        name: &'static str,
        enabled: bool,
    }

    let result = views::Recipe::try_bind_record(
        Recipe {
            name: "shared",
            enabled: true,
        },
        |recipe| {
            recipe
                .as_attrs()
                .merge(NixAttrs::new([("extra", "retained".into())]))
        },
    )
    .unwrap();
    assert_eq!(
        evaluate(result),
        serde_json::json!({"name":"shared", "enabled":true, "extra":"retained"})
    );

    let called = std::cell::Cell::new(false);
    let result = views::Recipe::try_bind_record(
        InvalidArguments {
            name: "invalid".into(),
        },
        |recipe| {
            called.set(true);
            recipe.name()
        },
    );
    assert!(result.is_err());
    assert!(!called.get());
}

#[test]
fn typed_library_operations_retain_the_supplied_helpers_and_output_fallbacks() {
    use rusix::interop::NixLibrary;

    let lib = Nixpkgs::new().library();
    assert_eq!(evaluate(lib.version_at_least("3.3.2", "3.0")), true);
    let openssl: Package = Nixpkgs::new().get("openssl").into();
    assert_eq!(
        evaluate(
            lib.get_dev(openssl.clone())
                .field::<Expr<String>>("outputName")
        ),
        "dev"
    );
    assert_eq!(
        evaluate(lib.get_lib(openssl).field::<Expr<String>>("outputName")),
        "out"
    );
    let single = Package::from_expression(NixValue::record([("name", "single-output".into())]));
    assert_eq!(
        evaluate(lib.get_dev(single.clone()).field::<Expr<String>>("name")),
        "single-output"
    );
    assert_eq!(
        evaluate(lib.get_lib(single).field::<Expr<String>>("name")),
        "single-output"
    );

    let replaced =
        NixLibrary::from_expression(lib.as_expression().merge_attrs(NixValue::record([
            (
                "versionAtLeast",
                NixValue::function(|_| NixValue::function(|_| false.into())),
            ),
            (
                "concatLists",
                NixValue::function(|_| NixValue::list(["replacement".into()])),
            ),
            (
                "getDev",
                NixValue::function(|_| NixValue::record([("name", "replaced-dev".into())])),
            ),
            (
                "getLib",
                NixValue::function(|_| NixValue::record([("name", "replaced-lib".into())])),
            ),
        ])));
    assert_eq!(evaluate(replaced.version_at_least("3.3.2", "3.0")), false);
    let ignored: NixList<Expr<String>> = NixList::from_expression(
        NixValue::builtin("throw").call("must remain excluded by supplied concatLists"),
    );
    assert_eq!(
        evaluate(NixList::concat_with(&replaced, [ignored])),
        serde_json::json!(["replacement"])
    );
    let ignored = Package::from_expression(NixValue::builtin("throw").call("excluded package"));
    assert_eq!(
        evaluate(
            replaced
                .get_dev(ignored.clone())
                .field::<Expr<String>>("name")
        ),
        "replaced-dev"
    );
    assert_eq!(
        evaluate(replaced.get_lib(ignored).field::<Expr<String>>("name")),
        "replaced-lib"
    );
}

#[test]
fn explicit_override_capabilities_survive_binding_and_calling_fetchers() {
    use rusix::interop::{NixOverridable, Overridable};

    let pkgs = Nixpkgs::new();
    let curl: Package = pkgs.get("curl").into();
    let curl = curl.override_attrs(|old| {
        NixAttrs::new([(
            "passthru",
            old.field::<NixAttrs>("passthru")
                .merge(NixAttrs::new([("typedDependency", true.into())]))
                .into(),
        )])
    });
    let fetcher: Overridable<NixCallable<Package>> =
        Overridable::from_expression(pkgs.value("fetchurl"));
    let source: Package = fetcher.bind(|fetcher| {
        fetcher
            .override_arguments(NixAttrs::new([("curl", curl)]))
            .extend(NixAttrs::new([("version", 1_i64.into())]))
            .call(NixValue::record([
                ("url", "https://example.invalid/typed-source.tar.xz".into()),
                (
                    "hash",
                    "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
                ),
            ]))
    });
    let first = NixCallable::<Package>::from_expression(NixValue::builtin("head"));
    let dependency = first.call(source.field::<NixList<Package>>("nativeBuildInputs"));
    assert_eq!(
        evaluate(dependency.field::<Expr<bool>>("typedDependency")),
        true
    );
}

#[test]
fn callable_parameter_contracts_survive_binding_currying_and_external_references() {
    let function = NixCallable::from_function(|prefix: Expr<String>| {
        NixCallable::from_function(move |suffix: Expr<String>| Expr::concat([prefix, suffix]))
    });
    let text = function.bind(|function| function.call("left:").call("right"));
    assert_eq!(evaluate(text), "left:right");

    let identity = Nixpkgs::new()
        .function("id")
        .signature::<Expr<String>, Expr<String>>();
    assert_eq!(evaluate(identity.call("external")), "external");
}

#[test]
fn shared_library_operations_preserve_literal_and_expression_interfaces() {
    let lib = Nixpkgs::new().library();
    let list: NixList<Expr<i64>> = lib.optional(true, 42_i64);
    let joined: NixList<Expr<i64>> = lib.concat_lists([list.clone(), lib.optionals(false, list)]);
    let guarded: NixList<Expr<i64>> = lib.throw_if_not(true, "unused", joined);
    assert_eq!(evaluate(guarded), serde_json::json!([42]));
    let package: Package = lib.throw_if_not(true, "unused", Nixpkgs::new().get("openssl"));
    assert_eq!(evaluate(package.field::<Expr<String>>("version")), "3.3.2");
    let text: Expr<String> = lib.replace_text("a.b", [(".", "_")]);
    assert_eq!(evaluate(text), "a_b");
}

#[test]
fn text_templates_return_strings_and_coerce_typed_packages_at_interpolation() {
    use rusix::{interop::ToNixText, nix_text};

    let package: Package = Nixpkgs::new().get("openssl").into();
    let text: Expr<String> = nix_text!("prefix:{package}/bin/openssl", package = package.clone());
    let changed: Expr<String> = text.clone().replace_text([("prefix", "changed")]);
    let contexts = NixValue::builtin("getContext");
    assert_eq!(
        evaluate(
            contexts
                .clone()
                .call(text)
                .equals(contexts.call(package.to_nix_text()))
        ),
        true
    );
    let literal: Expr<String> = nix_text!("left ", "right");
    assert_eq!(evaluate(literal), "left right");
    assert_eq!(
        evaluate(NixValue::builtin("substring").apply([
            0_i64.into(),
            8_i64.into(),
            changed.into()
        ])),
        "changed:"
    );
}

#[test]
fn nullable_values_map_and_default_lazily_while_attributes_distinguish_presence() {
    let bad: Expr<String> = NixValue::builtin("throw")
        .call("excluded nullable branch")
        .into_expr();
    let absent = NixNullable::<Expr<String>>::null();
    let mapped = absent.map(|_| bad.clone()).unwrap_or("fallback");
    assert_eq!(evaluate(mapped), "fallback");
    assert_eq!(
        evaluate(NixNullable::<Expr<String>>::some("kept").unwrap_or(bad)),
        "kept"
    );

    let attrs = NixAttrs::new([
        ("present", NixNullable::<Expr<String>>::some("value")),
        ("null", NixNullable::null()),
    ]);
    assert_eq!(evaluate(attrs.has("null")), true);
    assert_eq!(evaluate(attrs.has("missing")), false);
    assert_eq!(
        evaluate(
            attrs
                .get_or("null", NixNullable::some("missing fallback"))
                .unwrap_or("null fallback")
        ),
        "null fallback"
    );
    assert_eq!(
        evaluate(
            attrs
                .get_or("missing", NixNullable::some("missing fallback"))
                .unwrap_or("null fallback")
        ),
        "missing fallback"
    );

    let strings = NixAttrs::new([("a.b", Expr::<String>::from("literal key"))]);
    assert_eq!(
        evaluate(strings.get_optional("a.b").unwrap_or("missing")),
        "literal key"
    );
    assert_eq!(evaluate(strings.get_optional("missing").is_null()), true);
}

#[test]
fn nullable_transform_failures_keep_child_provenance_and_original_nix_diagnostics() {
    let line = line!() + 1;
    let failure = Expr::int(1).divide(Expr::int(0));
    let result = NixNullable::<Expr<i64>>::some(42_i64)
        .map(|_| failure.to_text())
        .unwrap_or("fallback");
    let diagnostic = NixSession::new()
        .unwrap()
        .evaluate_interop(&compile(Config::new().set_dynamic("result", result)).unwrap())
        .unwrap_err();
    assert_eq!(diagnostic.primary.as_ref().unwrap().file, file!());
    assert_eq!(diagnostic.primary.as_ref().unwrap().line, line);
    assert!(diagnostic.reason.contains("division by zero"));
    assert!(!diagnostic.raw_nix.is_empty());
}

#[test]
fn pinned_paths_remain_paths_until_explicit_text_coercion() {
    use rusix::{interop::ToNixText, nix_text};

    let path: NixPath =
        Nixpkgs::new().source_path("pkgs/applications/version-management/git/ssh-path.patch");
    assert_eq!(
        evaluate(NixValue::builtin("isPath").call(path.clone())),
        true
    );
    let text: Expr<String> = nix_text!("patch={path}", path = path.clone());
    let context = NixValue::builtin("getContext");
    assert_eq!(
        evaluate(
            context
                .clone()
                .call(text)
                .equals(context.call(path.to_nix_text()))
        ),
        true
    );
}

#[rusix::args]
mod reused_views {
    use super::views::Recipe;
    use rusix::interop::FinalAttrs;

    #[rusix(root)]
    struct Inputs {
        #[rusix(expression)]
        recipe: Recipe,
        #[rusix(expression, rename = "platform")]
        host: rusix::interop::Platform,
        #[rusix(expression)]
        final_attrs: FinalAttrs,
    }
}

#[test]
fn separately_declared_record_views_retain_lazy_fields_and_complete_platform_values() {
    let input = reused_views::from_value(rusix::nix_record! {
        "recipe": rusix::nix_record! { "name": "reused", "enabled": true },
        "platform": rusix::nix_record! { "system": "synthetic", "isLinux": true, "unknown": 17_i64 },
        "finalAttrs": rusix::nix_record! { "version": "1.0", "finalPackage": Nixpkgs::new().get("openssl") },
    });
    let text = input.recipe().bind(|recipe| recipe.name());
    assert_eq!(evaluate(text), "reused");
    assert_eq!(evaluate(input.host().is_linux()), true);
    assert_eq!(
        evaluate(input.host().as_attrs().field::<Expr<i64>>("unknown")),
        17
    );
    assert_eq!(evaluate(input.final_attrs().version()), "1.0");
    assert_eq!(
        evaluate(
            input
                .final_attrs()
                .final_package()
                .field::<Expr<String>>("version")
        ),
        "3.3.2"
    );

    let stdenv = rusix::interop::Stdenv::from_expression(rusix::nix_record! {
        "buildPlatform": rusix::nix_record! { "system": "same", "extra": 1_i64 },
        "hostPlatform": rusix::nix_record! { "system": "same", "extra": 2_i64 },
        "targetPlatform": rusix::nix_record! { "config": "target" },
    });
    assert_eq!(evaluate(stdenv.build_host_equal()), false);
    assert_eq!(evaluate(stdenv.build_platform().system()), "same");
    assert_eq!(evaluate(stdenv.host_platform().system()), "same");
    assert_eq!(evaluate(stdenv.target_platform().config()), "target");
}

/// A selected file's snapshot must not retain the containing source tree's string dependency.
#[test]
fn source_snapshots_depend_on_the_selected_file() {
    let root = Nixpkgs::new().source_path("pkgs/applications/version-management/git");
    let stored = NixPath::from_expression(NixValue::concat_text([root.as_expression()]));
    let filesystem =
        NixPath::from_source_text(stored.as_expression().to_text().into_expr::<String>());
    assert_eq!(
        evaluate(NixValue::builtin("typeOf").call(filesystem.clone())),
        "path"
    );
    assert_eq!(
        evaluate(
            filesystem
                .as_expression()
                .to_text()
                .equals(stored.as_expression().to_text())
        ),
        true
    );
    let selected = stored.join_relative("ssh-path.patch").unwrap().snapshot();
    let expected = NixValue::concat_text([root
        .join_relative("ssh-path.patch")
        .unwrap()
        .as_expression()]);

    assert_eq!(
        evaluate(selected.as_expression().equals(expected.clone())),
        true
    );
    assert_eq!(
        evaluate(
            NixValue::builtin("getContext")
                .call(selected)
                .equals(NixValue::builtin("getContext").call(expected))
        ),
        true,
    );
}

/// Snapshotting stays lazy and reports inaccessible sources at the authoring call site.
#[test]
fn missing_source_snapshots_are_lazy_and_attributed() {
    let missing = Nixpkgs::new().source_path("rusix-no-such-source-file");
    let line = line!() + 1;
    let snapshot = missing.snapshot();
    let generated = compile(
        Config::new()
            .set_dynamic("good", 42_i64)
            .set_dynamic("bad", snapshot),
    )
    .unwrap();
    let session = NixSession::new().unwrap();
    assert_eq!(
        session
            .evaluate_attribute(&generated, "good")
            .unwrap()
            .value,
        42
    );
    let error = session.evaluate_interop(&generated).unwrap_err();
    assert_eq!(error.primary.as_ref().unwrap().line, line);
    assert!(!error.raw_nix.is_empty());
}
