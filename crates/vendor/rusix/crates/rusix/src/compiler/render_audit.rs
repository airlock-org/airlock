//! Prove inspection comments are independent of machine provenance and lazy evaluation.
use super::*;
use crate::compiler::context_audit::equivalent_diagnostics;
use crate::nixos;
use crate::{
    Expr,
    interop::{Nixpkgs, raw::NixValue},
    nix_text,
    nixos::{DefinitionPriority, NixosModule, OptionDecl, OptionRef, OptionType},
};

const INSPECT: RenderOptions = RenderOptions {
    origin_comments: true,
    layout: RenderLayout::Readable,
};

#[test]
fn compact_output_preserves_literal_bytes_laziness_and_failure_origins() {
    let literal = "line one\n  λ with \"quotes\" and \\slashes\n";
    let config = Config::new()
        .set_dynamic("literal", literal)
        .set_dynamic("bad", Expr::int(44).divide(Expr::int(0)));
    let readable = compile(&config).unwrap();
    let compact = compile_with_options(
        &config,
        RenderOptions {
            layout: RenderLayout::Compact,
            ..Default::default()
        },
    )
    .unwrap();
    let session = NixSession::new().unwrap();

    for generated in [&readable, &compact] {
        assert_eq!(
            session
                .evaluate_attribute(generated, "literal")
                .unwrap()
                .value,
            literal
        );
        for span in &generated.spans {
            assert!(
                generated
                    .source
                    .get(span.start..span.end)
                    .is_some_and(|text| !text.is_empty())
            );
        }
    }

    assert_eq!(readable.spans.len(), compact.spans.len());
    for (before, after) in readable.spans.iter().zip(&compact.spans) {
        assert_eq!(before.origin, after.origin);
        assert_eq!(before.enclosing, after.enclosing);
    }

    equivalent_diagnostics(
        &session.evaluate_attribute(&readable, "bad").unwrap_err(),
        &session.evaluate_attribute(&compact, "bad").unwrap_err(),
    );
}

fn module_pair(module: &NixosModule) -> [nixos::NixosArtifact; 2] {
    [
        nixos::compile_module(module).unwrap(),
        nixos::compile_module_with_options(module, INSPECT).unwrap(),
    ]
}

fn base() -> NixosModule {
    NixosModule::empty().import("nixos/modules/services/networking/ssh/sshd.nix")
}

#[test]
fn both_renderings_persist_the_same_origins_with_their_own_correct_spans() {
    let number = Expr::int(22);
    let config = Config::new()
        .set_dynamic("primitive", true)
        .set_dynamic(
            "list",
            NixValue::list([number.clone().into(), 42_i64.into()]),
        )
        .set_dynamic(
            "nested",
            NixValue::record([(
                "record",
                NixValue::record([("number", number.clone().into())]),
            )]),
        )
        .set_dynamic("arithmetic", number.clone().divide(Expr::int(2)))
        .set_dynamic("conditional", NixValue::if_else(true, 1_i64, 2_i64))
        .set_dynamic(
            "selection",
            NixValue::record([("key", true.into())]).select("key"),
        )
        .set_dynamic("application", NixValue::function(|value| value).call(true))
        .set_dynamic("package", Nixpkgs::new().get("hello"))
        .set_dynamic("text", nix_text!("number={number}", number = number));
    let normal = compile(&config).unwrap();
    let debug = compile_with_options(&config, INSPECT).unwrap();

    assert!(!normal.source.contains("# rn-"));
    assert_eq!(debug.source.matches("# rn-").count(), debug.spans.len());
    assert_eq!(normal.spans.len(), debug.spans.len());
    assert_eq!(
        normal.source.matches("addErrorContext").count(),
        debug.source.matches("addErrorContext").count()
    );

    for (normal, debug) in normal.spans.iter().zip(&debug.spans) {
        assert_eq!(normal.origin, debug.origin);
        assert_eq!(normal.enclosing, debug.enclosing);
        assert_eq!(normal.diagnostic_site, debug.diagnostic_site);
    }

    for generated in [normal, debug] {
        let restored: Generated =
            serde_json::from_str(&serde_json::to_string(&generated).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&generated).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );

        for span in &restored.spans {
            assert!(
                restored
                    .source
                    .get(span.start..span.end)
                    .is_some_and(|text| !text.is_empty())
            );
            let before = &restored.source[..span.start];
            let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
            let column = before.rsplit('\n').next().unwrap().len() + 1;
            // Without comments, several nested spans can start at the same byte.
            // The most specific containing expression must still win.
            let narrowest = restored
                .spans
                .iter()
                .filter(|candidate| candidate.start <= span.start && span.start < candidate.end)
                .min_by_key(|candidate| candidate.end - candidate.start)
                .unwrap();
            assert_eq!(restored.at_position(line, column), Some(&narrowest.origin));
        }
    }
}

#[test]
fn source_map_only_failures_are_equivalent_without_any_runtime_contexts() {
    let config = Config::new().set_dynamic(
        "nested",
        vec![Expr::int(1), Expr::int(44).divide(Expr::int(0))],
    );
    let mut ast = lower(&config);
    crate::compiler::context_audit::contexts(&mut ast, false);
    let normal = render(&ast);
    let debug = render_with_options(&ast, INSPECT);
    let session = NixSession::new().unwrap();

    for generated in [&normal, &debug] {
        assert!(!generated.source.contains("addErrorContext"));
    }
    let normal = session.evaluate(&normal).unwrap_err();
    let debug = session.evaluate(&debug).unwrap_err();
    equivalent_diagnostics(&normal, &debug);
    assert_eq!(normal.provenance, Provenance::SourceMap);
    assert_eq!(normal.primary.as_ref().unwrap().purpose, "integer division");
}

#[test]
fn cloned_origins_keep_selected_occurrence_ancestry_and_lazy_siblings_in_both_modes() {
    let failure = Expr::int(44).divide(Expr::int(0));
    let config = Config::new()
        .set_dynamic("good", 42_i64)
        .set_dynamic("first", failure.clone())
        .set_dynamic("second", failure);
    let normal = compile(&config).unwrap();
    let debug = compile_with_options(&config, INSPECT).unwrap();
    let session = NixSession::new().unwrap();

    for generated in [&normal, &debug] {
        assert_eq!(
            session.evaluate_attribute(generated, "good").unwrap().value,
            42
        );
    }
    let normal = session.evaluate_attribute(&normal, "second").unwrap_err();
    let debug = session.evaluate_attribute(&debug, "second").unwrap_err();
    equivalent_diagnostics(&normal, &debug);
    assert!(
        normal
            .related
            .iter()
            .any(|origin| origin.purpose == "set second")
    );
    assert!(
        !normal
            .related
            .iter()
            .any(|origin| origin.purpose == "set first")
    );
}

/// A small public option declaration used to test schema-default attribution.
#[derive(rusix::IntoConfig)]
struct NumberSchema {
    /// NixOS checks this integer default after combining the module declarations.
    number: OptionDecl,
}

/// One module failure with the option and causal count expected from real NixOS.
struct ModuleCase {
    /// Identifies the diagnostic mechanism being compared.
    name: &'static str,
    /// Rust contributions, declarations and imports compiled once in each mode.
    module: NixosModule,
    /// Final option demanded by the isolated evaluator.
    selection: &'static [&'static str],
    /// NixOS failure category independent of generated source layout.
    kind: DiagnosticKind,
    /// Number of contributing definitions/rules that Nix actually reports.
    causes: usize,
}

#[test]
fn module_metadata_and_final_option_diagnostics_are_identical_in_both_modes() {
    let owner = "services.openssh.authorizedKeysCommandUser";
    let ports = "services.openssh.ports";
    let cases = [
        ModuleCase {
            name: "unknown",
            module: base().add(Config::new().set_dynamic("services.openssh.rusixMissing", true)),
            selection: &["services", "openssh", "ports"],
            kind: DiagnosticKind::NixosModule,
            causes: 1,
        },
        ModuleCase {
            name: "type",
            module: base().add(Config::new().set_dynamic(ports, vec!["wrong"])),
            selection: &["services", "openssh", "ports"],
            kind: DiagnosticKind::NixosType,
            causes: 1,
        },
        ModuleCase {
            name: "merge",
            module: base()
                .add(Config::new().set_dynamic(owner, "root"))
                .add(Config::new().set_dynamic(owner, "nobody")),
            selection: &["services", "openssh", "authorizedKeysCommandUser"],
            kind: DiagnosticKind::NixosMerge,
            causes: 2,
        },
        ModuleCase {
            name: "three-origin type",
            module: base()
                .add(Config::new().set_dynamic(ports, "wrong-a"))
                .add(Config::new().set_dynamic(ports, "wrong-b"))
                .add(Config::new().set_dynamic(ports, "wrong-c")),
            selection: &["services", "openssh", "ports"],
            kind: DiagnosticKind::NixosType,
            causes: 3,
        },
        ModuleCase {
            name: "assertion",
            module: base().assertion("port-policy", Expr::boolean(false), "port rejected"),
            selection: &["services", "openssh", "ports"],
            kind: DiagnosticKind::NixosAssertion,
            causes: 1,
        },
        ModuleCase {
            name: "import",
            module: base().import("nixos/modules/misc/label.nix"),
            selection: &["system", "nixos", "label"],
            kind: DiagnosticKind::ExternalNix,
            causes: 1,
        },
        ModuleCase {
            name: "final option",
            module: base().add(Config::new().set_dynamic(
                ports,
                vec![OptionRef::<i64>::new("services.openssh.rusixMissing").into_expr()],
            )),
            selection: &["services", "openssh", "ports"],
            kind: DiagnosticKind::NixEval,
            causes: 1,
        },
        ModuleCase {
            name: "schema default",
            module: NixosModule::empty().declare(NumberSchema {
                number: OptionDecl::new(OptionType::named("int")).default("wrong"),
            }),
            selection: &["number"],
            kind: DiagnosticKind::NixosType,
            causes: 1,
        },
    ];
    let session = NixSession::new().unwrap();

    for case in cases {
        let [normal, debug] = module_pair(&case.module);
        let metadata = |artifact: &nixos::NixosArtifact| {
            let mut value = serde_json::to_value(artifact).unwrap();
            value.as_object_mut().unwrap().remove("module");
            value
        };
        assert_eq!(metadata(&normal), metadata(&debug));
        let normal = session
            .evaluate_nixos(&normal, case.selection, case.name == "assertion")
            .unwrap_err();
        let debug = session
            .evaluate_nixos(&debug, case.selection, case.name == "assertion")
            .unwrap_err();
        equivalent_diagnostics(&normal, &debug);
        assert_eq!(normal.kind, case.kind, "{}: {normal:?}", case.name);
        assert_eq!(normal.origins.len(), case.causes, "{}", case.name);
    }
}

#[test]
fn discarded_definitions_and_lazy_guard_branches_remain_unforced_in_both_modes() {
    let failure: NixValue = Expr::int(44).divide(Expr::int(0)).into();
    let module = base()
        .module(
            NixosModule::new(Config::new().set_dynamic("services.openssh.ports", failure.clone()))
                .priority(DefinitionPriority::Default),
        )
        .add(Config::new().set_dynamic("services.openssh.ports", vec![22]));
    let session = NixSession::new().unwrap();

    for artifact in module_pair(&module) {
        assert_eq!(
            session
                .evaluate_nixos(&artifact, &["services", "openssh", "ports"], false)
                .unwrap()
                .value,
            serde_json::json!([22])
        );
    }

    let lib = Nixpkgs::new().library();
    let guarded = lib.throw_if_not(
        false,
        "first failure",
        lib.throw_if_not(
            failure.clone().into_expr::<bool>(),
            failure.clone().into_expr::<String>(),
            failure.clone(),
        ),
    );
    let config = Config::new()
        .set_dynamic(
            "good",
            lib.throw_if_not(true, failure.into_expr::<String>(), 42_i64),
        )
        .set_dynamic("bad", guarded);
    let normal = compile(&config).unwrap();
    let debug = compile_with_options(&config, INSPECT).unwrap();

    // Stage interop without forcing the rejected guard or the unused message.
    for generated in [&normal, &debug] {
        session.stage_interop().unwrap();
        assert_eq!(
            session.evaluate_attribute(generated, "good").unwrap().value,
            42
        );
    }
    let normal = session.evaluate_attribute(&normal, "bad").unwrap_err();
    let debug = session.evaluate_attribute(&debug, "bad").unwrap_err();
    equivalent_diagnostics(&normal, &debug);
    assert_eq!(normal.reason, "first failure");
}
