//! Generate Nix source from Rust configuration and evaluate it in isolation.
//!
//! Nix evaluates configuration expressions and describes software builds. This
//! crate compiles values authored with `rusix` into that language. Compilation
//! happens in Rust; evaluation is a separate step using `crate::NixSession`.
//!
//! [`compile`] generates ordinary Nix configuration, while
//! [`crate::nixos::compile_module`] generates a module that NixOS combines with other
//! modules. Generated artifacts retain Rust locations for error reporting through
//! [`Diagnostic`]. Evaluations use temporary, disposable Nix stores, without
//! building packages or activating a system. [`crate::compiler::ast`] is an advanced code-generation
//! API; normal authors use `rusix` instead.
//!
//! Describe a value, compile it, then ask Nix to evaluate it:
//!
//! ```
//! use rusix::{Expr, IntoConfig};
//! use rusix::compile;
//! # #[cfg(feature = "evaluation")]
//! # {
//! use rusix::NixSession;
//! use std::path::Path;
//!
//! #[derive(IntoConfig)]
//! struct Output {
//!     // Integer computation deferred until Nix evaluates the answer field.
//!     answer: Expr<i64>,
//! }
//!
//! let generated = compile(Output {
//!     answer: Expr::from(84_i64).divide(2.into()),
//! }).expect("valid Rust configuration");
//! // generated.source contains Nix; no Nix process has run yet.
//!
//! let session = NixSession::new().expect("temporary evaluation workspace");
//! match session.evaluate(&generated) {
//!     Ok(evaluation) => println!("answer: {}", evaluation.value["answer"]),
//!     Err(diagnostic) => {
//!         eprintln!("{}", diagnostic.render(Path::new(".")));
//!         eprintln!("Original Nix output: {}", diagnostic.raw_nix);
//!     }
//! }
//! # }
//! ```
//!
//! `Evaluation::value` holds the selected result as JSON. JSON conversion makes
//! Nix compute the data being returned, including nested fields; selecting a small
//! result can leave unrelated expressions unevaluated. Evaluation failures and
//! compilation failures both return [`Diagnostic`]. Its [`Diagnostic::reason`]
//! explains the error, [`Diagnostic::origins`] lists recovered causes, and
//! [`Diagnostic::raw_nix`] preserves the original Nix output. Some errors have
//! several causes or no known Rust location; [`Diagnostic::render`] displays the
//! available information without requiring one culprit.
//!
//! With the default `evaluation` feature, `NixSession` evaluates generated values
//! in an offline disposable store. See the crate README for evaluator setup.
pub mod ast;

pub(crate) mod interop;

pub(crate) mod provenance;

pub(crate) mod render;

pub(crate) mod sharing;

#[cfg(all(test, feature = "evaluation"))]
mod backend_audit;

#[cfg(all(test, feature = "evaluation"))]
pub(crate) mod context_audit;

#[cfg(all(test, feature = "evaluation"))]
mod render_audit;

use crate::Diagnostic;

#[cfg(all(test, feature = "evaluation"))]
use crate::NixSession;

use crate::{Config, IntoConfig, ir::Node, ir::ValueKind};

#[cfg(all(test, feature = "evaluation"))]
use crate::{DiagnosticKind, Provenance};

use ast::{BinaryOp, Builtin, NixExpr, NixKind};
pub use render::{Generated, RenderLayout, RenderOptions, SourceSpan, render, render_with_options};
use std::{cell::Cell, rc::Rc};

/// Generate Nix source directly from a typed root implementing [`IntoConfig`].
/// Prefer a struct deriving `IntoConfig` or a root in a `#[rusix::config]`
/// module. An already lowered [`Config`] or `&Config` is also accepted.
/// This validates the Rust description and returns source plus the information
/// needed to map generated error locations back to Rust. It does not run Nix.
/// References to final NixOS options or the system’s supplied package set require
/// [`crate::nixos::compile_module`] and are rejected by this standalone compiler.
/// Repeated substantial expressions with identical attribution share lazy local
/// bindings. Shared bindings preserve lexical dependencies and authored error paths.
/// Immediate lambda calls expose lazy bindings and literal arguments, allowing
/// equivalent work in separate scopes to share one enclosing binding.
#[track_caller]
pub fn compile(config: impl IntoConfig) -> Result<Generated, Box<Diagnostic>> {
    compile_with_options(config, RenderOptions::default())
}

/// Generate validated Nix with optional origin comments for manual inspection.
/// Normal [`compile`] output omits these comments. Both forms retain the same
/// origins, ancestry and runtime boundaries, with spans for their own text.
///
/// ```
/// use rusix::IntoConfig;
/// use rusix::{RenderOptions, compile_with_options};
///
/// #[derive(IntoConfig)]
/// struct Output {
///     // Literal enable flag emitted as the Nix field enabled.
///     enabled: bool,
/// }
///
/// let annotated = compile_with_options(
///     Output { enabled: true },
///     RenderOptions { origin_comments: true, ..Default::default() },
/// ).unwrap();
/// assert!(annotated.source.contains("# rn-"));
/// ```
#[track_caller]
pub fn compile_with_options(
    config: impl IntoConfig,
    options: RenderOptions,
) -> Result<Generated, Box<Diagnostic>> {
    let config = config.into_config();
    validate_standalone(&config)?;

    let mut generated = render_with_options(&sharing::optimize(lower(&config)), options);
    generated.backend_metadata = provenance::collect(&config);
    Ok(generated)
}

/// Generate a standalone Nix expression, retaining validation and diagnostic source mappings.
///
/// Use this for a function, scalar, inspection query or flake attribute set that must
/// be the file's root value. Unlike [`compile`], this does not place the value inside
/// a configuration record. Rust validates the description; Nix evaluation remains
/// a separate operation. NixOS option and module package references still require
/// [`crate::nixos::compile_module`].
///
/// ```
/// use rusix::{compile_value, interop::raw::NixValue};
/// let identity = NixValue::function(|value| value);
/// let generated = compile_value(identity).expect("valid standalone function");
/// // The emitted file evaluates to a function, ready for a caller to apply.
/// ```
#[track_caller]
pub fn compile_value(value: impl crate::ConfigValue) -> Result<Generated, Box<Diagnostic>> {
    let config = Config::new().set_dynamic("value", value);
    validate_standalone(&config)?;

    let mut generated = render(&sharing::optimize(lower_value(
        &config.assignments[0].value,
    )));
    generated.backend_metadata = provenance::collect(&config);
    Ok(generated)
}

/// Apply the same scope and value checks to record roots and standalone expressions.
fn validate_standalone(config: &Config) -> Result<(), Box<Diagnostic>> {
    config
        .validate()
        .map_err(|error| Box::new(Diagnostic::validation(error.origin, error.message)))?;

    for assignment in &config.assignments {
        if let Some(reference) = option_reference(&assignment.value) {
            return Err(Diagnostic::validation(
                reference.origin.clone(),
                "symbolic NixOS option references require NixosModule lowering".into(),
            )
            .into());
        }
    }

    Ok(())
}

/// Find scoped dependencies without evaluating any values.
pub(crate) fn option_reference(node: &Node) -> Option<&Node> {
    scoped_reference(node, false)
}

pub(crate) fn module_package_reference(node: &Node) -> Option<&Node> {
    scoped_reference(node, true)
}

fn scoped_reference(node: &Node, packages_only: bool) -> Option<&Node> {
    let option_reference = |node| scoped_reference(node, packages_only);
    match &node.kind {
        ValueKind::OptionReference(_) => {
            if packages_only {
                None
            } else {
                Some(node)
            }
        }
        ValueKind::List(items) => items.iter().find_map(option_reference),
        ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => {
            fields.iter().find_map(|(_, node)| option_reference(node))
        }
        ValueKind::Apply(left, right)
        | ValueKind::AttrMerge(left, right)
        | ValueKind::Assert(left, right)
        | ValueKind::Divide(left, right)
        | ValueKind::Equal(left, right) => {
            option_reference(left).or_else(|| option_reference(right))
        }
        ValueKind::Select(value, _)
        | ValueKind::ToText(value)
        | ValueKind::FilesystemPath(value)
        | ValueKind::StringPrefix { value, .. }
        | ValueKind::PathAppend { value, .. }
        | ValueKind::InRange { value, .. } => option_reference(value),
        ValueKind::Function { body, .. } => option_reference(body),
        ValueKind::FunctionAttrs { defaults, body, .. } => defaults
            .iter()
            .find_map(|(_, v)| option_reference(v))
            .or_else(|| option_reference(body)),
        ValueKind::If(condition, yes, no) => option_reference(condition)
            .or_else(|| option_reference(yes))
            .or_else(|| option_reference(no)),
        ValueKind::Reference(reference) => match &reference.source {
            rusix::ir::Source::NixosPackages { .. } => Some(node),
            rusix::ir::Source::Packages { overlays } => overlays.iter().find_map(option_reference),
            _ => None,
        },
        ValueKind::Parameter(_) => None,
        ValueKind::Bool(_)
        | ValueKind::Int(_)
        | ValueKind::Float(_)
        | ValueKind::Null
        | ValueKind::String(_) => None,
    }
}

fn lower(config: &Config) -> NixExpr {
    let assignments = config
        .assignments
        .iter()
        .map(|assignment| {
            let value = lower_value(&assignment.value);
            // Assignment and value keep separate identities, even for primitives.
            let annotated =
                NixExpr::attributed(NixKind::Group(Box::new(value)), assignment.origin.clone());
            (assignment.path_segments().to_vec(), annotated)
        })
        .collect();

    NixExpr::attributed(NixKind::AttrSet(assignments), config.origin.clone())
}

fn variable(name: &str) -> NixExpr {
    NixExpr::plain(NixKind::Variable(name.into()))
}

fn integer(value: i64) -> NixExpr {
    NixExpr::plain(NixKind::Int(value))
}

fn binary(op: BinaryOp, left: NixExpr, right: NixExpr) -> NixExpr {
    NixExpr::plain(NixKind::Binary(op, Box::new(left), Box::new(right)))
}

pub(crate) fn lower_value(node: &Node) -> NixExpr {
    lower_scoped(node, &[])
}

/// Runtime markers are reserved for failures that lose their operation position.
#[derive(Clone, Copy)]
enum DiagnosticBoundary {
    /// Nix may report only the external implementation of the called function.
    OpaqueCall,
    /// Explicit constraints must identify the validating operation.
    Validation,
    /// Nix string addition can blame the operand rather than its consuming operation.
    Coercion,
    /// A merged option can force a foreign definition without a generated lookup frame.
    FinalOption,
    /// Nix's union type error can report only the enclosing field, not the // operation.
    RecordUnion,
}

/// Ordinary expression failures use generated positions and static span ancestry.
fn runtime_boundary(kind: &ValueKind) -> Option<DiagnosticBoundary> {
    match kind {
        ValueKind::Apply(..) => Some(DiagnosticBoundary::OpaqueCall),
        ValueKind::AttrMerge(..) => Some(DiagnosticBoundary::RecordUnion),
        ValueKind::InRange { .. } => Some(DiagnosticBoundary::Validation),
        ValueKind::StringPrefix { .. }
        | ValueKind::PathAppend { .. }
        | ValueKind::FilesystemPath(_) => Some(DiagnosticBoundary::Coercion),
        ValueKind::OptionReference(_) => Some(DiagnosticBoundary::FinalOption),
        _ => None,
    }
}

/// A callback parameter or a native argument-set scope identified by semantic IR.
#[derive(Clone)]
struct ParameterScope<'a> {
    /// Matches references to this function, independently of argument names.
    binding: u64,
    /// Native named bindings; ordinary callbacks have only a generated parameter.
    arguments: Option<&'a [String]>,
    /// Shared by nested lowering, so an outer record is captured before shadowing.
    record_used: Rc<Cell<bool>>,
}

/// Lower one default or body, materializing resolved arguments only when needed.
fn lower_argument_region(
    node: &Node,
    binding: u64,
    arguments: &[String],
    scope: &[ParameterScope<'_>],
) -> NixExpr {
    let record_used = Rc::new(Cell::new(false));
    let mut nested = scope.to_vec();
    nested.push(ParameterScope {
        binding,
        arguments: Some(arguments),
        record_used: record_used.clone(),
    });
    let value = lower_scoped(node, &nested);

    if !record_used.get() {
        return value;
    }

    // Whole-record use and shadowed outer names need resolved bindings, not an
    // @-pattern's raw caller record (which would omit defaulted arguments).
    let record = NixExpr::plain(NixKind::AttrSet(
        arguments
            .iter()
            .map(|argument| (vec![argument.clone()], variable(argument)))
            .collect(),
    ));
    NixExpr::plain(NixKind::Let(
        format!("__rusix_arg_{}", scope.len()),
        Box::new(record),
        Box::new(value),
    ))
}

/// Identify a selection chain rooted in a native argument-set scope.
fn argument_scope(node: &Node, scope: &[ParameterScope<'_>]) -> Option<usize> {
    match &node.kind {
        ValueKind::Parameter(binding) => scope
            .iter()
            .position(|entry| entry.binding == *binding && entry.arguments.is_some()),
        ValueKind::Select(value, _) => argument_scope(value, scope),
        _ => None,
    }
}

fn lower_scoped(node: &Node, scope: &[ParameterScope<'_>]) -> NixExpr {
    let lower_value = |node| lower_scoped(node, scope);
    let kind = match &node.kind {
        ValueKind::Function { binding, body } => {
            let name = format!("__rusix_arg_{}", scope.len());
            let mut scope = scope.to_vec();
            scope.push(ParameterScope {
                binding: *binding,
                arguments: None,
                record_used: Rc::new(Cell::new(false)),
            });
            NixKind::Lambda(name, Box::new(lower_scoped(body, &scope)))
        }
        ValueKind::FunctionAttrs {
            binding,
            arguments,
            open,
            defaults,
            body,
        } => {
            let parameters = arguments
                .iter()
                .map(|argument| {
                    (
                        argument.clone(),
                        defaults
                            .iter()
                            .find(|(n, _)| n == argument)
                            .map(|(_, value)| {
                                lower_argument_region(value, *binding, arguments, scope)
                            }),
                    )
                })
                .collect();
            let body = Box::new(lower_argument_region(body, *binding, arguments, scope));

            if *open {
                NixKind::OpenArgumentFunction(parameters, body)
            } else {
                NixKind::ArgumentFunction(parameters, body)
            }
        }
        ValueKind::Parameter(binding) => {
            let index = scope
                .iter()
                .position(|entry| entry.binding == *binding)
                .expect("validated callback scope");
            if scope[index].arguments.is_some() {
                scope[index].record_used.set(true);
            }
            NixKind::Variable(format!("__rusix_arg_{index}"))
        }
        ValueKind::If(condition, yes, no) => NixKind::If(
            Box::new(lower_value(condition)),
            Box::new(lower_value(yes)),
            Box::new(lower_value(no)),
        ),
        ValueKind::AttrMerge(left, right) => NixKind::Binary(
            BinaryOp::AttrMerge,
            Box::new(lower_value(left)),
            Box::new(lower_value(right)),
        ),
        ValueKind::Assert(condition, value) => NixKind::Assert(
            Box::new(lower_value(condition)),
            Box::new(lower_value(value)),
        ),
        ValueKind::Equal(left, right) => NixKind::Binary(
            crate::compiler::ast::BinaryOp::Equal,
            Box::new(lower_value(left)),
            Box::new(lower_value(right)),
        ),
        ValueKind::Bool(v) => NixKind::Bool(*v),
        ValueKind::Int(v) => NixKind::Int(*v),
        ValueKind::Float(v) => NixKind::Float(*v),
        ValueKind::Null => NixKind::Null,
        ValueKind::String(v) => NixKind::String(v.clone()),
        ValueKind::List(items) => NixKind::List(items.iter().map(lower_value).collect()),
        ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => NixKind::AttrSet(
            fields
                .iter()
                .map(|(name, node)| (vec![name.clone()], lower_value(node)))
                .collect(),
        ),
        ValueKind::Reference(reference) => {
            return interop::lower_reference_scoped(reference, &lower_value);
        }
        ValueKind::OptionReference(path) => {
            NixKind::Select(Box::new(variable("config")), path.parts().to_vec())
        }
        ValueKind::ToText(value) => NixKind::Call(Builtin::ToString, vec![lower_value(value)]),
        ValueKind::FilesystemPath(value) => NixKind::Binary(
            BinaryOp::Add,
            Box::new(NixExpr::plain(NixKind::Path("/.".into()))),
            Box::new(NixExpr::plain(NixKind::Call(
                Builtin::ToPath,
                vec![lower_value(value)],
            ))),
        ),
        ValueKind::PathAppend { value, suffix } => NixKind::Binary(
            BinaryOp::Add,
            Box::new(lower_value(value)),
            Box::new(NixExpr::plain(NixKind::String(suffix.clone()))),
        ),
        ValueKind::StringPrefix { prefix, value } => NixKind::Binary(
            BinaryOp::Add,
            Box::new(NixExpr::plain(NixKind::String(prefix.clone()))),
            Box::new(lower_value(value)),
        ),
        ValueKind::Apply(function, argument) => {
            let mut function = lower_value(function);
            // A curried .apply(...) records the same caller for every application.
            // The outer call covers evaluation of its partial applications too.
            if matches!(function.kind, NixKind::Apply(..))
                && function
                    .origin
                    .as_ref()
                    .is_some_and(|o| o.id == node.origin.id)
            {
                function.error_context = false;
            }
            NixKind::Apply(Box::new(function), Box::new(lower_value(argument)))
        }
        ValueKind::Select(value, path) => {
            let kind = if let Some(index) = argument_scope(value, scope) {
                let first = &path.parts()[0];
                let known = scope[index].arguments.unwrap().contains(first);
                let shadowed = scope[index + 1..]
                    .iter()
                    .any(|entry| entry.arguments.is_some_and(|names| names.contains(first)));

                // Only a direct parameter selection becomes a bare lexical name.
                // Keep every enclosing selection's provenance and source span.
                if matches!(value.kind, ValueKind::Parameter(_)) && known && !shadowed {
                    let root =
                        NixExpr::attributed(NixKind::Variable(first.clone()), value.origin.clone());
                    NixKind::ArgumentSelect(Box::new(root), path.parts()[1..].to_vec())
                } else {
                    NixKind::ArgumentSelect(Box::new(lower_value(value)), path.parts().to_vec())
                }
            } else {
                interop::select_opaque(lower_value(value), path).kind
            };
            return NixExpr::attributed(kind, node.origin.clone());
        }
        ValueKind::Divide(left, right) => {
            NixKind::Call(Builtin::Div, vec![lower_value(left), lower_value(right)])
        }
        ValueKind::InRange {
            value,
            min,
            max,
            message,
        } => {
            let condition = binary(
                BinaryOp::And,
                binary(
                    BinaryOp::GreaterEqual,
                    variable("__rusix_range"),
                    integer(*min),
                ),
                binary(
                    BinaryOp::LessEqual,
                    variable("__rusix_range"),
                    integer(*max),
                ),
            );

            let body = NixExpr::plain(NixKind::If(
                Box::new(condition),
                Box::new(variable("__rusix_range")),
                Box::new(NixExpr::plain(NixKind::Call(
                    Builtin::Throw,
                    vec![NixExpr::plain(NixKind::String(message.clone()))],
                ))),
            ));

            NixKind::Let(
                "__rusix_range".into(),
                Box::new(lower_value(value)),
                Box::new(body),
            )
        }
    };

    if runtime_boundary(&node.kind).is_some() {
        NixExpr::contextual(kind, node.origin.clone())
    } else {
        NixExpr::attributed(kind, node.origin.clone())
    }
}
