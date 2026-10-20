#![cfg(feature = "evaluation")]

//! Exact build/host equality uses supplied Nix values without platform-schema knowledge.
use rusix::interop::raw::NixRepresentation;
use rusix::{
    Config, Expr,
    interop::{Nixpkgs, raw::NixValue},
    ir::ValueKind,
    package::build_host_equal,
};
use rusix::{Generated, NixSession, Provenance, compile};

#[rusix::args]
mod args {
    /// A test-only finite view of a package's external arguments.
    #[rusix(root)]
    struct Inputs {
        /// Retains the caller's entire build environment, including undeclared fields.
        stdenv: Stdenv,
    }

    /// Navigation declares a few fields without reconstructing the supplied record.
    #[rusix(value)]
    struct Stdenv {
        /// Platform running build tools.
        build_platform: Platform,
        /// Platform running the resulting package.
        host_platform: Platform,
    }

    /// Deliberately incomplete view: additional platform fields must still affect equality.
    struct Platform {
        /// Architecture/OS shorthand, insufficient to establish platform equality.
        system: String,
    }
}

fn platform(system: &str) -> NixValue {
    NixValue::record([("system", system.into())])
}

fn stdenv(build: NixValue, host: NixValue) -> NixValue {
    NixValue::record([("buildPlatform", build), ("hostPlatform", host)])
}

fn artifact(value: impl Into<NixValue>) -> Generated {
    compile(Config::new().set_dynamic("result", value.into())).unwrap()
}

fn evaluate(value: impl Into<NixValue>) -> serde_json::Value {
    NixSession::new()
        .unwrap()
        .evaluate_interop(&artifact(value))
        .unwrap()
        .value["result"]
        .clone()
}

#[test]
fn shared_platform_values_with_functions_compare_equal_without_considering_target() {
    // A Nix function argument supplies one shared value to both fields. Merely
    // cloning a Rust record would render two separate Nix record expressions.
    let compare = NixValue::function(|shared| {
        let environment = stdenv(shared.clone(), shared).merge_attrs(NixValue::record([(
            "targetPlatform",
            platform("aarch64-linux"),
        )]));
        build_host_equal(environment).into()
    });
    let shared = platform("x86_64-linux").merge_attrs(NixValue::record([(
        "canExecute",
        NixValue::function(|_| false.into()),
    )]));

    assert_eq!(evaluate(compare.call(shared)), true);
}

#[test]
fn different_platform_values_and_negation_use_existing_boolean_expressions() {
    let environment = stdenv(platform("x86_64-linux"), platform("aarch64-linux"));
    let equal: Expr<bool> = build_host_equal(environment);

    assert_eq!(
        evaluate(NixValue::list([equal.clone().into(), (!equal).into()])),
        serde_json::json!([false, true]),
    );
    assert_eq!(
        evaluate(!build_host_equal(stdenv(
            platform("x86_64-linux"),
            platform("x86_64-linux"),
        ))),
        false,
    );
}

#[test]
fn same_system_does_not_hide_additional_attributes_libc_or_static_differences() {
    let base = platform("x86_64-linux");
    let pairs = [
        (
            base.clone(),
            base.clone()
                .merge_attrs(NixValue::record([("rusixBranchProbe", true.into())])),
        ),
        (
            base.clone()
                .merge_attrs(NixValue::record([("libc", "glibc".into())])),
            base.clone()
                .merge_attrs(NixValue::record([("libc", "musl".into())])),
        ),
        (
            base.clone()
                .merge_attrs(NixValue::record([("isStatic", false.into())])),
            base.merge_attrs(NixValue::record([("isStatic", true.into())])),
        ),
    ];

    for (build, host) in pairs {
        assert_eq!(
            evaluate(NixValue::record([
                (
                    "sameSystem",
                    build
                        .clone()
                        .select("system")
                        .equals(host.clone().select("system")),
                ),
                ("equal", build_host_equal(stdenv(build, host)).into()),
            ])),
            serde_json::json!({"sameSystem": true, "equal": false}),
        );
    }
}

#[test]
fn raw_equality_differs_from_function_filtered_equality_and_executability() {
    let build = platform("x86_64-linux").merge_attrs(NixValue::record([(
        "canExecute",
        NixValue::function(|_| true.into()),
    )]));
    let host = platform("x86_64-linux").merge_attrs(NixValue::record([(
        "canExecute",
        NixValue::function(|_| false.into()),
    )]));
    let filtered_equal = Nixpkgs::new()
        .library()
        .as_expression()
        .clone()
        .select_segments(["systems", "equals"])
        .apply([build.clone(), host.clone()]);

    assert_eq!(
        evaluate(NixValue::record([
            ("filteredEqual", filtered_equal),
            (
                "canExecute",
                build.clone().select("canExecute").call(host.clone())
            ),
            ("equal", build_host_equal(stdenv(build, host)).into()),
        ])),
        serde_json::json!({"filteredEqual": true, "canExecute": true, "equal": false}),
    );
}

fn factory() -> NixValue {
    NixValue::function_attrs(["stdenv"], |arguments| {
        let input = args::from_value(arguments);
        (
            Vec::<(&str, NixValue)>::new(),
            NixValue::record([
                (
                    "sameSystem",
                    NixValue::from(input.stdenv.build_platform.system())
                        .equals(input.stdenv.host_platform.system()),
                ),
                ("equal", build_host_equal(input.stdenv.as_value()).into()),
            ]),
        )
    })
}

#[test]
fn one_factory_observes_supplied_stdenv_and_fields_outside_its_structural_view() {
    let factory = factory();
    let base = platform("x86_64-linux");
    let changed = base.clone().merge_attrs(NixValue::record([(
        "notDeclaredInRust",
        NixValue::record([("nested", 42_i64.into())]),
    )]));

    assert_eq!(
        evaluate(NixValue::list([
            factory.clone().call(NixValue::record([(
                "stdenv",
                stdenv(base.clone(), base.clone()),
            )])),
            factory.call(NixValue::record([("stdenv", stdenv(base, changed))])),
        ])),
        serde_json::json!([
            {"sameSystem": true, "equal": true},
            {"sameSystem": true, "equal": false},
        ]),
    );
}

#[test]
fn generated_comparison_is_lexical_without_record_reconstruction_or_runtime_wrapping() {
    let generated = artifact(factory());

    let compact: String = generated.source.split_whitespace().collect();
    assert!(
        compact.contains("stdenv.buildPlatform==stdenv.hostPlatform"),
        "{}",
        generated.source,
    );
    assert!(!generated.source.contains("let __rusix_arg_"));
    assert!(!generated.source.contains("builtins.import"));
    assert!(!generated.source.contains("builtins.addErrorContext"));
}

#[test]
fn unused_comparisons_and_dependent_defaults_remain_lazy() {
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();
    let comparison = build_host_equal(failure.clone());
    let factory = NixValue::function_attrs(["stdenv", "equal"], |arguments| {
        (
            vec![("equal", build_host_equal(arguments.select("stdenv")).into())],
            42_i64.into(),
        )
    });

    assert_eq!(
        evaluate(NixValue::record([
            (
                "good",
                NixValue::record([("good", 42_i64.into()), ("bad", comparison.clone().into())])
                    .select("good"),
            ),
            ("shortCircuit", Expr::boolean(false).and(comparison).into()),
            (
                "unusedDefault",
                factory.call(NixValue::record([("stdenv", failure)])),
            ),
        ])),
        serde_json::json!({"good": 42, "shortCircuit": false, "unusedDefault": 42}),
    );
}

#[test]
fn missing_platforms_map_to_the_helper_call_through_source_spans() {
    let session = NixSession::new().unwrap();
    let environments = [
        NixValue::record([("hostPlatform", platform("x86_64-linux"))]),
        NixValue::record([("buildPlatform", platform("x86_64-linux"))]),
        "not an attribute set".into(),
    ];

    for environment in environments {
        let call_line = line!() + 1;
        let comparison = build_host_equal(environment);
        let config = Config::new().set_dynamic("result", comparison);
        let node = &config.assignments[0].value;
        assert_eq!(node.origin.file, file!());
        assert_eq!(node.origin.line, call_line);
        let ValueKind::Equal(build, host) = &node.kind else {
            panic!("helper must use ordinary equality IR")
        };
        for (operand, field) in [(build, "buildPlatform"), (host, "hostPlatform")] {
            let ValueKind::Select(_, path) = &operand.kind else {
                panic!("helper must select original platform values")
            };
            assert_eq!(path.parts(), &[field]);
            assert_eq!(operand.origin.line, call_line);
        }

        let error = session
            .evaluate_interop(&compile(&config).unwrap())
            .unwrap_err();
        assert_eq!(error.primary.as_ref().unwrap().file, file!());
        assert_eq!(error.primary.as_ref().unwrap().line, call_line);
        assert_eq!(error.provenance, Provenance::SourceMap);
        assert!(!error.raw_nix.is_empty());
    }
}

#[test]
fn platform_expression_failures_keep_their_original_operation() {
    let session = NixSession::new().unwrap();
    let failure_line = line!() + 1;
    let failure: NixValue = Expr::int(1).divide(Expr::int(0)).into();

    for environment in [
        stdenv(failure.clone(), platform("x86_64-linux")),
        stdenv(platform("x86_64-linux"), failure),
    ] {
        let error = session
            .evaluate_interop(&artifact(build_host_equal(environment)))
            .unwrap_err();
        assert_eq!(error.primary.as_ref().unwrap().file, file!());
        assert_eq!(error.primary.as_ref().unwrap().line, failure_line);
        assert_eq!(error.reason, "division by zero");
        assert!(!error.raw_nix.is_empty());
    }
}
