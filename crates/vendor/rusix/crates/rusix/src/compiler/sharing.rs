//! Share repeated and function-invariant computations while preserving lexical dependencies.
//!
//! Nix memoizes a lazy binding, not arbitrary identical source expressions. Keep
//! sufficiently large repeated trees in local `let` bindings and keep invariant
//! work outside function calls. Attribution is part of expression identity:
//! equal values authored at different sites stay distinct.

use super::ast::{BinaryOp, Builtin, NixExpr, NixKind};
use crate::ir::Origin;
use std::collections::{HashMap, HashSet};

mod lifting;

mod normalize;

mod scopes;

/// Lexical names read by rendered syntax without explicit variable nodes in the AST.
const IMPLICIT_NAMES: &[&str] = &[
    "builtins",
    "__lessThan",
    "__sub",
    "true",
    "false",
    "null",
    "__curPos",
];

/// Describe prelude lookups performed by Nix's parser or by a native operation.
fn implicit_operation(expr: &NixExpr) -> Option<&'static str> {
    match &expr.kind {
        NixKind::Call(..) => Some("builtins"),
        NixKind::Bool(true) => Some("true"),
        NixKind::Bool(false) => Some("false"),
        NixKind::Null => Some("null"),
        // Nix parses comparisons and unary minus as calls to lexical helpers.
        NixKind::Binary(BinaryOp::GreaterEqual | BinaryOp::LessEqual, ..) => Some("__lessThan"),
        NixKind::Int(value) if *value < 0 => Some("__sub"),
        NixKind::Float(value) if value.is_sign_negative() => Some("__sub"),
        _ => None,
    }
}

/// Scalar syntax and binding boundaries, with descendants represented separately.
#[derive(PartialEq, Eq, Hash)]
enum Shape<'a> {
    Bool(bool),
    Int(i64),
    Float(u64),
    Null,
    String(&'a str),
    Path(&'a str),
    List,
    Group,
    AttrSet(Vec<&'a [String]>),
    Call(Builtin),
    Apply,
    Binary(BinaryOp),
    If,
    Assert,
    Variable(&'a str),
    Select(&'a [String]),
    ArgumentSelect(&'a [String]),
    /// Every binder is unique in its parent region; its children have their own regions.
    Boundary(usize),
}

/// Exact structural identity, including all original diagnostic attribution.
#[derive(PartialEq, Eq, Hash)]
struct Key<'a> {
    /// Source operation; separate authored origins must never acquire shared blame.
    origin: Option<&'a Origin>,
    /// Authored container path; sharing across distinct paths would lose use-site diagnostics.
    enclosing: Vec<&'a Origin>,
    /// Whether this operation contributes a runtime error frame.
    error_context: bool,
    /// Nonrecursive syntax for this node.
    shape: Shape<'a>,
    /// Interned child identities in source order.
    children: Vec<usize>,
}

/// Reuse counts and approximate token weight for one structurally distinct subtree.
struct Occurrences {
    /// Number of original occurrences in this lexical region.
    count: usize,
    /// Node count plus an allowance for long literals, used to avoid sharing tiny lookups.
    weight: usize,
}

/// Bottom-up interning makes identity checks proportional to the original tree size.
#[derive(Default)]
struct Region<'a> {
    /// Complete keys resolve hash collisions by structural equality.
    keys: HashMap<Key<'a>, usize>,
    /// Addresses identify original nodes only while that region is being rewritten.
    identities: HashMap<*const NixExpr, usize>,
    /// Frequency and weight indexed by structural identity.
    occurrences: Vec<Occurrences>,
}

impl<'a> Region<'a> {
    /// Intern descendants before their parent, stopping at lexical scope boundaries.
    fn index(&mut self, expr: &'a NixExpr, enclosing: &mut Vec<&'a Origin>) -> usize {
        let ancestors = enclosing.clone();
        if let Some(origin) = &expr.origin {
            enclosing.push(origin);
        }
        let descendants = if boundary(expr) {
            Vec::new()
        } else {
            children(expr)
        };
        let children: Vec<_> = descendants
            .into_iter()
            .map(|child| self.index(child, enclosing))
            .collect();
        if expr.origin.is_some() {
            enclosing.pop();
        }
        let weight = children.iter().fold(1_usize, |weight, child| {
            weight.saturating_add(self.occurrences[*child].weight)
        });
        let shape = match &expr.kind {
            NixKind::Bool(value) => Shape::Bool(*value),
            NixKind::Int(value) => Shape::Int(*value),
            NixKind::Float(value) => Shape::Float(value.to_bits()),
            NixKind::Null => Shape::Null,
            NixKind::String(value) => Shape::String(value),
            NixKind::Path(value) => Shape::Path(value),
            NixKind::List(_) => Shape::List,
            NixKind::Group(_) => Shape::Group,
            NixKind::AttrSet(fields) => {
                Shape::AttrSet(fields.iter().map(|(path, _)| path.as_slice()).collect())
            }
            NixKind::Call(builtin, _) => Shape::Call(*builtin),
            NixKind::Apply(..) => Shape::Apply,
            NixKind::Binary(operator, ..) => Shape::Binary(*operator),
            NixKind::If(..) => Shape::If,
            NixKind::Assert(..) => Shape::Assert,
            NixKind::Variable(name) => Shape::Variable(name),
            NixKind::Select(_, path) => Shape::Select(path),
            NixKind::ArgumentSelect(_, path) => Shape::ArgumentSelect(path),
            _ => Shape::Boundary(self.identities.len()),
        };
        let literal_weight = match &shape {
            Shape::String(value) | Shape::Path(value) => value.len() / 64,
            _ => 0,
        };
        let key = Key {
            origin: expr.origin.as_ref(),
            enclosing: ancestors,
            error_context: expr.error_context,
            shape,
            children,
        };
        let next = self.occurrences.len();
        let identity = *self.keys.entry(key).or_insert_with(|| {
            self.occurrences.push(Occurrences {
                count: 0,
                weight: weight.saturating_add(literal_weight),
            });
            next
        });
        self.occurrences[identity].count += 1;
        self.identities.insert(expr, identity);
        identity
    }
}

/// Generated names avoid every existing variable and binder, including advanced AST inputs.
#[derive(Default)]
struct Names {
    /// Reserved lexical identifiers collected before introducing any bindings.
    used: HashSet<String>,
    /// Deterministic traversal counter; hash-map iteration never controls output order.
    next: usize,
}

impl Names {
    /// Reserve authored identifiers even inside scopes the optimizer does not merge.
    fn collect(&mut self, expr: &NixExpr) {
        match &expr.kind {
            NixKind::Variable(name) | NixKind::Lambda(name, _) | NixKind::Let(name, ..) => {
                self.used.insert(name.clone());
            }
            NixKind::Function(names, _) => self.used.extend(names.iter().cloned()),
            NixKind::ArgumentFunction(defaults, _) | NixKind::OpenArgumentFunction(defaults, _) => {
                self.used
                    .extend(defaults.iter().map(|(name, _)| name.clone()));
            }
            _ => {}
        }
        for child in children(expr) {
            self.collect(child);
        }
    }

    /// Allocate one identifier without capturing any existing variable.
    fn fresh(&mut self) -> String {
        loop {
            let name = format!("__rusix_shared_{next}", next = self.next);
            self.next += 1;
            if self.used.insert(name.clone()) {
                return name;
            }
        }
    }
}

/// Optimize the lowered tree in place without duplicating the complete compiler artifact.
pub(crate) fn optimize(mut ast: NixExpr) -> NixExpr {
    let mut names = Names::default();
    names.collect(&ast);
    // This intrinsic observes the generated text itself. Moving even unrelated
    // syntax could change its line or column, so retain the complete tree.
    if names.used.contains("__curPos") {
        return ast;
    }
    normalize::normalize(&mut ast, &mut names);
    // Lifting introduces let boundaries. Preserve repeated-expression sharing
    // first, before moving individual invariants into their enclosing scopes.
    scopes::share(&mut ast, &mut names);
    lifting::lift(&mut ast, &mut names);
    ast
}

/// Treat functions, argument defaults and existing lets as independent lexical regions.
fn boundary(expr: &NixExpr) -> bool {
    matches!(
        expr.kind,
        NixKind::Lambda(..)
            | NixKind::Function(..)
            | NixKind::ArgumentFunction(..)
            | NixKind::OpenArgumentFunction(..)
            | NixKind::Let(..)
    )
}

/// Replace repeated trees with attributed references and retain each definition exactly once.
fn rewrite(
    expr: &mut NixExpr,
    identities: &HashMap<*const NixExpr, usize>,
    candidates: &[bool],
    ancestry: &HashMap<usize, Vec<Origin>>,
    shared: &mut HashMap<usize, String>,
    definitions: &mut Vec<(String, NixExpr)>,
    names: &mut Names,
) {
    let identity = identities[&(expr as *const NixExpr)];
    if candidates[identity] {
        let reference = |name: String, origin| NixExpr {
            origin,
            error_context: false,
            kind: NixKind::Variable(name),
        };
        if let Some(name) = shared.get(&identity) {
            *expr = reference(name.clone(), expr.origin.clone());
            return;
        }
        for child in children_mut(expr) {
            rewrite(
                child,
                identities,
                candidates,
                ancestry,
                shared,
                definitions,
                names,
            );
        }
        let name = names.fresh();
        let value = reference(name.clone(), expr.origin.clone());
        let mut definition = std::mem::replace(expr, value);
        // A definition now lives before the original result expression. Retain
        // its original source-map ancestry without adding runtime error frames.
        for origin in ancestry[&identity].iter().rev() {
            definition = NixExpr::attributed(NixKind::Group(Box::new(definition)), origin.clone());
        }
        definitions.push((name.clone(), definition));
        shared.insert(identity, name);
    } else if !boundary(expr) {
        for child in children_mut(expr) {
            rewrite(
                child,
                identities,
                candidates,
                ancestry,
                shared,
                definitions,
                names,
            );
        }
    }
}

/// Borrow immediate children in a stable order, including argument defaults before bodies.
fn children(expr: &NixExpr) -> Vec<&NixExpr> {
    match &expr.kind {
        NixKind::List(items) | NixKind::Call(_, items) => items.iter().collect(),
        NixKind::AttrSet(fields) => fields.iter().map(|(_, value)| value).collect(),
        NixKind::Group(value)
        | NixKind::Select(value, _)
        | NixKind::ArgumentSelect(value, _)
        | NixKind::Lambda(_, value)
        | NixKind::Function(_, value) => vec![value],
        NixKind::Apply(left, right)
        | NixKind::Binary(_, left, right)
        | NixKind::Assert(left, right)
        | NixKind::Let(_, left, right) => vec![left, right],
        NixKind::If(condition, yes, no) => vec![condition, yes, no],
        NixKind::ArgumentFunction(defaults, body)
        | NixKind::OpenArgumentFunction(defaults, body) => defaults
            .iter()
            .filter_map(|(_, default)| default.as_ref())
            .chain(std::iter::once(body.as_ref()))
            .collect(),
        _ => Vec::new(),
    }
}

/// Borrow children mutably in exactly the same order used for structural indexing.
fn children_mut(expr: &mut NixExpr) -> Vec<&mut NixExpr> {
    match &mut expr.kind {
        NixKind::List(items) | NixKind::Call(_, items) => items.iter_mut().collect(),
        NixKind::AttrSet(fields) => fields.iter_mut().map(|(_, value)| value).collect(),
        NixKind::Group(value)
        | NixKind::Select(value, _)
        | NixKind::ArgumentSelect(value, _)
        | NixKind::Lambda(_, value)
        | NixKind::Function(_, value) => vec![value],
        NixKind::Apply(left, right)
        | NixKind::Binary(_, left, right)
        | NixKind::Assert(left, right)
        | NixKind::Let(_, left, right) => vec![left, right],
        NixKind::If(condition, yes, no) => vec![condition, yes, no],
        NixKind::ArgumentFunction(defaults, body)
        | NixKind::OpenArgumentFunction(defaults, body) => defaults
            .iter_mut()
            .filter_map(|(_, default)| default.as_mut())
            .chain(std::iter::once(body.as_mut()))
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(all(test, feature = "evaluation"))]
mod tests;
