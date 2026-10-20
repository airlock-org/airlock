//! Compiler-owned shape metadata for pinned mkDerivation dependency diagnostics.
//!
//! This reads authored IR structure only. Unknown functions, conditions and list
//! lengths stop indexing; no Nix values or package identities are evaluated here.
use crate::{
    Config,
    ir::{Node, Origin, Source, ValueKind},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, rc::Rc};

pub(crate) const REVISION: &str = "8b27c1239e5c421a2bbc2c65d52e4a6fbf2ff296";

const MAX_CHILDREN: usize = 4096;

const MAX_NAME_BYTES: usize = 1024;

const DEPENDENCIES: &[&str] = &[
    "buildInputs",
    "nativeBuildInputs",
    "propagatedBuildInputs",
    "propagatedNativeBuildInputs",
    "depsBuildBuild",
    "depsBuildTarget",
    "depsHostHost",
    "depsTargetTarget",
    "depsBuildBuildPropagated",
    "depsBuildTargetPropagated",
    "depsHostHostPropagated",
    "depsTargetTargetPropagated",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Metadata {
    /// Format version used when reading saved dependency-diagnostic metadata.
    pub(crate) version: u32,
    /// Pinned nixpkgs revision whose dependency-validation messages this metadata describes.
    pub(crate) revision: String,
    /// Authored build recipes and dependency lists that can explain failures reported by nixpkgs.
    pub(crate) boundaries: Vec<Boundary>,
    /// Rust origin IDs for assertion conditions, so failures do not blame an unused assertion body.
    pub(crate) guard_operations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Boundary {
    /// Root configuration assignment through which this build recipe is reachable.
    pub(crate) entry: Origin,
    /// Rust operation that called mkDerivation to describe this package build.
    pub(crate) call: Origin,
    /// Package name recovered from the recipe, typically pname without its version.
    pub(crate) name: String,
    /// Build-recipe name, including the version when derived from pname and version.
    pub(crate) full_name: String,
    /// Names of authored build recipes that use this package in their dependency lists.
    pub(crate) parents: Vec<String>,
    /// Whether a dependency could not be understood from Rust expressions alone.
    pub(crate) opaque_children: bool,
    /// Build dependency lists, such as buildInputs, whose authored members can be located.
    pub(crate) fields: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Field {
    /// Dependency-list field name in the Nix build recipe, such as nativeBuildInputs.
    pub(crate) name: String,
    /// Rust expression that supplied this dependency list to the build recipe.
    pub(crate) origin: Origin,
    /// List members recoverable from authored expressions, with their supplier and consumer locations.
    pub(crate) children: Vec<Child>,
    /// Whether the full outer list was recovered; false means later member positions may be unknown.
    pub(crate) complete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Child {
    /// Zero-based, outermost first; backend indexes are normalized by the parser.
    pub(crate) path: Vec<usize>,
    /// Rust expression that supplied the dependency at this list position.
    pub(crate) origin: Origin,
    /// Rust operation that selected or passed this member into the consuming dependency list.
    pub(crate) consumer: Origin,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Space {
    Packages,
    Library,
    Builtins,
}

type Env<'a> = Rc<Vec<(u64, Binding<'a>)>>;

#[derive(Clone)]
struct Expr<'a> {
    /// Authored value or operation being inspected without evaluating Nix.
    node: &'a Node,
    /// Known function-argument bindings used to follow shared dependencies through authored expressions.
    env: Env<'a>,
}

#[derive(Clone)]
enum Term<'a> {
    Expr(Expr<'a>),
    Namespace(Space, Vec<String>),
}

#[derive(Clone)]
enum Binding<'a> {
    Value(Term<'a>),
    Arguments {
        names: &'a [String],
        defaults: &'a [(String, Node)],
        supplied: Term<'a>,
        packages: bool,
    },
    FinalAttrs(&'a Node),
}

enum Lookup<'a> {
    Found(Term<'a>),
    Absent,
    Unknown,
}

fn expr<'a>(node: &'a Node, env: &Env<'a>) -> Term<'a> {
    Term::Expr(Expr {
        node,
        env: env.clone(),
    })
}

fn extend<'a>(env: &Env<'a>, id: u64, binding: Binding<'a>) -> Env<'a> {
    let mut next = env.as_ref().clone();
    next.push((id, binding));
    Rc::new(next)
}

fn binding<'a, 'e>(e: &'e Expr<'a>, id: u64) -> Option<&'e Binding<'a>> {
    e.env
        .iter()
        .rev()
        .find_map(|(key, value)| (*key == id).then_some(value))
}

fn namespace(term: &Term<'_>, space: Space, path: &[&str]) -> bool {
    matches!(term, Term::Namespace(s, p) if *s == space && p.iter().map(String::as_str).eq(path.iter().copied()))
}

fn applications<'a>(e: &Expr<'a>) -> (Term<'a>, Vec<Term<'a>>) {
    let mut node = e.node;
    let mut args = Vec::new();
    while let ValueKind::Apply(function, argument) = &node.kind {
        args.push(expr(argument, &e.env));
        node = function;
    }
    args.reverse();
    (expr(node, &e.env), args)
}

// Follow only lexical substitutions and authored function applications. External
// applications stay opaque; pinned helper shapes are handled separately below.
fn head<'a>(term: Term<'a>, fuel: usize) -> Term<'a> {
    if fuel == 0 {
        return term;
    }
    let Term::Expr(e) = &term else {
        return term;
    };
    match &e.node.kind {
        ValueKind::Reference(reference) => {
            let space = match &reference.source {
                Source::Builtins => Some(Space::Builtins),
                Source::Library => Some(Space::Library),
                Source::Packages { overlays } if overlays.is_empty() => Some(Space::Packages),
                _ => None,
            };
            space.map_or(term.clone(), |space| {
                Term::Namespace(
                    space,
                    reference
                        .path
                        .as_ref()
                        .map_or_else(Vec::new, |p| p.parts().to_vec()),
                )
            })
        }
        ValueKind::Parameter(id) => match binding(e, *id) {
            Some(Binding::Value(value)) => head(value.clone(), fuel - 1),
            _ => term,
        },
        ValueKind::Select(base, path) => {
            let mut value = expr(base, &e.env);
            for key in path.parts() {
                match lookup(value, key, fuel - 1) {
                    Lookup::Found(found) => value = found,
                    _ => return term,
                }
            }
            head(value, fuel - 1)
        }
        ValueKind::Assert(_, value) => head(expr(value, &e.env), fuel - 1),
        ValueKind::If(condition, yes, no) => match boolean(expr(condition, &e.env), fuel - 1) {
            Some(condition) => head(expr(if condition { yes } else { no }, &e.env), fuel - 1),
            None => term,
        },
        ValueKind::Apply(function, argument) => {
            let callable = head(expr(function, &e.env), fuel - 1);
            if let Term::Expr(f) = callable {
                match &f.node.kind {
                    ValueKind::Function { binding, body } => {
                        let env = extend(&f.env, *binding, Binding::Value(expr(argument, &e.env)));
                        return head(expr(body, &env), fuel - 1);
                    }
                    ValueKind::FunctionAttrs {
                        binding,
                        arguments,
                        defaults,
                        body,
                        ..
                    } => {
                        let env = extend(
                            &f.env,
                            *binding,
                            Binding::Arguments {
                                names: arguments,
                                defaults,
                                supplied: expr(argument, &e.env),
                                packages: false,
                            },
                        );
                        return head(expr(body, &env), fuel - 1);
                    }
                    _ => {}
                }
            }
            let (callee, args) = applications(e);
            let callee = head(callee, fuel - 1);
            if namespace(&callee, Space::Packages, &["callPackage"])
                && args.len() == 2
                && let Term::Expr(f) = head(args[0].clone(), fuel - 1)
                && let ValueKind::FunctionAttrs {
                    binding,
                    arguments,
                    defaults,
                    body,
                    ..
                } = &f.node.kind
            {
                let env = extend(
                    &f.env,
                    *binding,
                    Binding::Arguments {
                        names: arguments,
                        defaults,
                        supplied: args[1].clone(),
                        packages: true,
                    },
                );
                return head(expr(body, &env), fuel - 1);
            }
            term
        }
        _ => term,
    }
}

fn lookup<'a>(term: Term<'a>, key: &str, fuel: usize) -> Lookup<'a> {
    if fuel == 0 {
        return Lookup::Unknown;
    }
    match head(term, fuel - 1) {
        Term::Namespace(space, mut path) => {
            path.push(key.into());
            Lookup::Found(Term::Namespace(space, path))
        }
        Term::Expr(e) => match &e.node.kind {
            ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => fields
                .iter()
                .find(|(name, _)| name == key)
                .map_or(Lookup::Absent, |(_, node)| {
                    Lookup::Found(expr(node, &e.env))
                }),
            ValueKind::AttrMerge(left, right) => match lookup(expr(right, &e.env), key, fuel - 1) {
                Lookup::Absent => lookup(expr(left, &e.env), key, fuel - 1),
                value => value,
            },
            ValueKind::If(_, yes, no) => {
                // Unknown conditional key sets may still prove a key absent.
                if matches!(lookup(expr(yes, &e.env), key, fuel - 1), Lookup::Absent)
                    && matches!(lookup(expr(no, &e.env), key, fuel - 1), Lookup::Absent)
                {
                    Lookup::Absent
                } else {
                    Lookup::Unknown
                }
            }
            ValueKind::Parameter(id) => match binding(&e, *id) {
                Some(Binding::Arguments {
                    names,
                    defaults,
                    supplied,
                    packages,
                }) => match lookup(supplied.clone(), key, fuel - 1) {
                    Lookup::Absent => {
                        if let Some((_, default)) = defaults.iter().find(|(n, _)| n == key) {
                            // callPackage auto-arguments override named defaults.
                            // Without evaluating the package scope, their presence
                            // is unknown. Only direct applications may use a default
                            // as evidence for a static list length/name.
                            if *packages {
                                Lookup::Unknown
                            } else {
                                Lookup::Found(expr(default, &e.env))
                            }
                        } else if *packages && names.iter().any(|name| name == key) {
                            Lookup::Found(Term::Namespace(Space::Packages, vec![key.into()]))
                        } else {
                            Lookup::Unknown
                        }
                    }
                    value => value,
                },
                Some(Binding::FinalAttrs(body)) => lookup(expr(body, &e.env), key, fuel - 1),
                _ => Lookup::Unknown,
            },
            _ => Lookup::Unknown,
        },
    }
}

fn library(term: &Term<'_>, name: &str) -> bool {
    namespace(term, Space::Library, &[name]) || namespace(term, Space::Packages, &["lib", name])
}

fn boolean(term: Term<'_>, fuel: usize) -> Option<bool> {
    if fuel == 0 {
        return None;
    }
    let Term::Expr(e) = head(term, fuel - 1) else {
        return None;
    };
    match &e.node.kind {
        ValueKind::Bool(value) => Some(*value),
        ValueKind::If(condition, yes, no) => {
            if let Some(value) = boolean(expr(condition, &e.env), fuel - 1) {
                boolean(expr(if value { yes } else { no }, &e.env), fuel - 1)
            } else {
                let yes = boolean(expr(yes, &e.env), fuel - 1)?;
                (boolean(expr(no, &e.env), fuel - 1)? == yes).then_some(yes)
            }
        }
        _ => None,
    }
}

fn text(term: Term<'_>, fuel: usize) -> Option<String> {
    if fuel == 0 {
        return None;
    }
    let Term::Expr(e) = head(term, fuel - 1) else {
        return None;
    };
    match &e.node.kind {
        ValueKind::String(value) => (value.len() <= MAX_NAME_BYTES).then(|| value.clone()),
        ValueKind::ToText(value) => text(expr(value, &e.env), fuel - 1),
        ValueKind::StringPrefix { prefix, value } => {
            let value = text(expr(value, &e.env), fuel - 1)?;
            (prefix.len() + value.len() <= MAX_NAME_BYTES).then(|| format!("{prefix}{value}"))
        }
        ValueKind::Apply(..) => {
            let (callee, args) = applications(&e);
            let callee = head(callee, fuel - 1);
            if library(&callee, "optionalString") && args.len() == 2 {
                return if boolean(args[0].clone(), fuel - 1)? {
                    text(args[1].clone(), fuel - 1)
                } else {
                    Some(String::new())
                };
            }
            if (library(&callee, "concatStringsSep")
                || namespace(&callee, Space::Builtins, &["concatStringsSep"]))
                && args.len() == 2
            {
                let sep = text(args[0].clone(), fuel - 1)?;
                let Term::Expr(parts) = head(args[1].clone(), fuel - 1) else {
                    return None;
                };
                let ValueKind::List(items) = &parts.node.kind else {
                    return None;
                };
                if items.len() > MAX_CHILDREN {
                    return None;
                }
                let strings: Option<Vec<_>> = items
                    .iter()
                    .map(|n| text(expr(n, &parts.env), fuel - 1))
                    .collect();
                let strings = strings?;
                let size = strings.iter().map(String::len).sum::<usize>()
                    + sep.len() * strings.len().saturating_sub(1);
                return (size <= MAX_NAME_BYTES).then(|| strings.join(&sep));
            }
            None
        }
        _ => None,
    }
}

// A bounded set of literal names is safe when a feature condition is unknown:
// the diagnostic must match an exact alternative, never a guessed default.
fn names(term: Term<'_>, fuel: usize) -> Option<Vec<String>> {
    if fuel == 0 {
        return None;
    }
    if let Some(value) = text(term.clone(), fuel) {
        return Some(vec![value]);
    }
    let Term::Expr(e) = head(term, fuel - 1) else {
        return None;
    };
    match &e.node.kind {
        ValueKind::If(_, yes, no) => {
            let mut values = names(expr(yes, &e.env), fuel - 1)?;
            for value in names(expr(no, &e.env), fuel - 1)? {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            (values.len() <= 8).then_some(values)
        }
        ValueKind::ToText(value) => names(expr(value, &e.env), fuel - 1),
        ValueKind::StringPrefix { prefix, value } => names(expr(value, &e.env), fuel - 1)?
            .into_iter()
            .map(|value| {
                (prefix.len() + value.len() <= MAX_NAME_BYTES).then(|| format!("{prefix}{value}"))
            })
            .collect(),
        ValueKind::Apply(..) => {
            let (callee, args) = applications(&e);
            let callee = head(callee, fuel - 1);
            if library(&callee, "optionalString") && args.len() == 2 {
                let mut values = names(args[1].clone(), fuel - 1)?;
                if !values.iter().any(String::is_empty) {
                    values.push(String::new());
                }
                return (values.len() <= 8).then_some(values);
            }
            if (library(&callee, "concatStringsSep")
                || namespace(&callee, Space::Builtins, &["concatStringsSep"]))
                && args.len() == 2
            {
                let sep = text(args[0].clone(), fuel - 1)?;
                let Term::Expr(parts) = head(args[1].clone(), fuel - 1) else {
                    return None;
                };
                let ValueKind::List(items) = &parts.node.kind else {
                    return None;
                };
                if items.len() > MAX_CHILDREN {
                    return None;
                }
                let mut values = vec![String::new()];
                for (index, item) in items.iter().enumerate() {
                    let part = names(expr(item, &parts.env), fuel - 1)?;
                    let mut next = Vec::new();
                    for prefix in &values {
                        for suffix in &part {
                            let separator = if index == 0 { "" } else { &sep };
                            if prefix.len() + separator.len() + suffix.len() > MAX_NAME_BYTES {
                                return None;
                            }
                            let value = format!("{prefix}{separator}{suffix}");
                            if !next.contains(&value) {
                                next.push(value);
                            }
                            if next.len() > 8 {
                                return None;
                            }
                        }
                    }
                    values = next;
                }
                return Some(values);
            }
            None
        }
        _ => None,
    }
}

// A supplier origin follows lexical aliases but stops at the producing package
// application, retaining its Rust call rather than the package's implementation.
fn supplier<'a>(term: Term<'a>, fuel: usize) -> Option<Origin> {
    if fuel == 0 {
        return None;
    }
    let Term::Expr(e) = term else {
        return None;
    };
    match &e.node.kind {
        ValueKind::Parameter(id) => {
            if let Some(Binding::Value(value)) = binding(&e, *id) {
                supplier(value.clone(), fuel - 1)
            } else {
                Some(e.node.origin.clone())
            }
        }
        ValueKind::Select(base, path) => {
            let mut value = expr(base, &e.env);
            for key in path.parts() {
                match lookup(value, key, fuel - 1) {
                    Lookup::Found(found) => value = found,
                    _ => return Some(e.node.origin.clone()),
                }
            }
            supplier(value, fuel - 1).or_else(|| Some(e.node.origin.clone()))
        }
        _ => Some(e.node.origin.clone()),
    }
}

struct List<'a> {
    /// Known leading list members in their Nix order; inspection stops when the shape is unknown.
    prefix: Vec<Term<'a>>,
    /// Whether the prefix covers the entire outer list rather than only its known beginning.
    complete: bool,
}

fn list<'a>(term: Term<'a>, fuel: usize) -> List<'a> {
    let unknown = || List {
        prefix: Vec::new(),
        complete: false,
    };
    if fuel == 0 {
        return unknown();
    }
    let Term::Expr(e) = head(term, fuel - 1) else {
        return unknown();
    };
    match &e.node.kind {
        ValueKind::List(items) => List {
            prefix: items
                .iter()
                .take(MAX_CHILDREN)
                .map(|n| expr(n, &e.env))
                .collect(),
            complete: items.len() <= MAX_CHILDREN,
        },
        ValueKind::Apply(..) => {
            let (callee, args) = applications(&e);
            let callee = head(callee, fuel - 1);
            if args.len() == 1
                && (namespace(&callee, Space::Builtins, &["concatLists"])
                    || library(&callee, "concatLists"))
            {
                let parts = list(args[0].clone(), fuel - 1);
                let mut result = List {
                    prefix: Vec::new(),
                    complete: parts.complete,
                };
                for part in parts.prefix {
                    let part = list(part, fuel - 1);
                    let available = MAX_CHILDREN - result.prefix.len();
                    let truncated = part.prefix.len() > available;
                    result
                        .prefix
                        .extend(part.prefix.into_iter().take(available));
                    if !part.complete || truncated || result.prefix.len() == MAX_CHILDREN {
                        result.complete = false;
                        break;
                    }
                }
                return result;
            }
            if args.len() == 2 && (library(&callee, "optional") || library(&callee, "optionals")) {
                return match boolean(args[0].clone(), fuel - 1) {
                    Some(false) => List {
                        prefix: Vec::new(),
                        complete: true,
                    },
                    Some(true) if library(&callee, "optional") => List {
                        prefix: vec![args[1].clone()],
                        complete: true,
                    },
                    Some(true) => list(args[1].clone(), fuel - 1),
                    None => unknown(),
                };
            }
            unknown()
        }
        _ => unknown(),
    }
}

fn children(terms: &[Term<'_>], path: &[usize], fuel: usize, out: &mut Vec<Child>) {
    if fuel == 0 {
        return;
    }
    for (index, term) in terms.iter().enumerate() {
        if out.len() == MAX_CHILDREN {
            return;
        }
        let Term::Expr(e) = term else {
            continue;
        };
        let mut path = path.to_vec();
        path.push(index);
        out.push(Child {
            path: path.clone(),
            origin: supplier(term.clone(), fuel).unwrap_or_else(|| e.node.origin.clone()),
            consumer: e.node.origin.clone(),
        });
        let nested = list(term.clone(), fuel - 1);
        children(&nested.prefix, &path, fuel - 1, out);
    }
}

fn recipe(term: Term<'_>) -> Term<'_> {
    match head(term, 48) {
        Term::Expr(e) => match &e.node.kind {
            ValueKind::Function { binding, body } => {
                let env = extend(&e.env, *binding, Binding::FinalAttrs(body));
                expr(body, &env)
            }
            _ => Term::Expr(e),
        },
        value => value,
    }
}

fn member<'a>(term: &Term<'a>, name: &str) -> Option<Term<'a>> {
    if let Lookup::Found(value) = lookup(term.clone(), name, 48) {
        Some(value)
    } else {
        None
    }
}

fn package(term: Term<'_>, fuel: usize) -> Option<Expr<'_>> {
    if fuel == 0 {
        return None;
    }
    let Term::Expr(e) = head(term, fuel - 1) else {
        return None;
    };
    match &e.node.kind {
        ValueKind::Apply(function, _)
            if namespace(
                &head(expr(function, &e.env), fuel - 1),
                Space::Packages,
                &["stdenv", "mkDerivation"],
            ) =>
        {
            Some(e)
        }
        ValueKind::AttrMerge(left, right)
            if matches!(
                lookup(expr(right, &e.env), "drvPath", fuel - 1),
                Lookup::Absent
            ) && matches!(
                lookup(expr(right, &e.env), "outPath", fuel - 1),
                Lookup::Absent
            ) =>
        {
            package(expr(left, &e.env), fuel - 1)
        }
        ValueKind::Select(base, path)
            if matches!(
                path.parts().last().map(String::as_str),
                Some("drvPath" | "outPath")
            ) =>
        {
            let mut value = expr(base, &e.env);
            for key in &path.parts()[..path.parts().len() - 1] {
                value = member(&value, key)?;
            }
            package(value, fuel - 1)
        }
        _ => None,
    }
}

// This pinned scope helper's projection-only callback selects upstream Perl
// packages. It cannot construct a competing Rust-authored backend boundary.
fn package_scope_projections(callee: &Term<'_>, args: &[Term<'_>]) -> bool {
    let Term::Namespace(Space::Packages, path) = callee else {
        return false;
    };
    if !path
        .iter()
        .rev()
        .take(2)
        .map(String::as_str)
        .eq(["withPackages", "perl"])
        || args.len() != 1
    {
        return false;
    }
    let Term::Expr(callback) = head(args[0].clone(), 48) else {
        return false;
    };
    let ValueKind::Function { binding, body } = &callback.node.kind else {
        return false;
    };
    let ValueKind::List(items) = &body.kind else {
        return false;
    };
    items.iter().all(|item| matches!(&item.kind, ValueKind::Select(base, _) if matches!(base.kind, ValueKind::Parameter(id) if id == *binding)))
}

struct Collector {
    /// Authored package recipes discovered while inspecting configuration expressions.
    boundaries: Vec<Boundary>,
    /// Remaining traversal budget, bounding Rust-side inspection of a large expression tree.
    remaining: usize,
}

impl Collector {
    fn opaque_child(&mut self, entry: &Origin, parent: Option<&str>) {
        if let Some(parent) = parent {
            for boundary in &mut self.boundaries {
                if boundary.entry.id == entry.id && boundary.full_name == parent {
                    boundary.opaque_children = true;
                }
            }
        }
    }

    fn walk(&mut self, term: Term<'_>, entry: &Origin, parent: Option<&str>, depth: usize) {
        if depth == 0 || self.remaining == 0 {
            self.opaque_child(entry, parent);
            return;
        }
        self.remaining -= 1;
        let Term::Expr(e) = head(term, 48) else {
            return;
        };
        if let Some(pkg) = package(Term::Expr(e.clone()), 48) {
            let ValueKind::Apply(_, argument) = &pkg.node.kind else {
                return;
            };
            let attrs = recipe(expr(argument, &pkg.env));
            let named = member(&attrs, "name").and_then(|v| names(v, 48));
            let pname = member(&attrs, "pname").and_then(|v| names(v, 48));
            let version = member(&attrs, "version").and_then(|v| text(v, 48));
            let variants = named
                .map(|values| {
                    values
                        .into_iter()
                        .map(|n| (n.clone(), n))
                        .collect::<Vec<_>>()
                })
                .or_else(|| {
                    let version = version?;
                    Some(
                        pname?
                            .into_iter()
                            .map(|n| (n.clone(), format!("{n}-{version}")))
                            .collect(),
                    )
                });
            let Some(variants) = variants else {
                self.opaque_child(entry, parent);
                return;
            };
            let mut fields = Vec::new();
            for name in DEPENDENCIES {
                if let Some(Term::Expr(field)) = member(&attrs, name) {
                    let shape = list(Term::Expr(field.clone()), 48);
                    let mut indexed = Vec::new();
                    children(&shape.prefix, &[], 12, &mut indexed);
                    fields.push(Field {
                        name: (*name).into(),
                        origin: field.node.origin.clone(),
                        children: indexed,
                        complete: shape.complete,
                    });
                }
            }
            for (name, full_name) in variants {
                let boundary = Boundary {
                    entry: entry.clone(),
                    call: pkg.node.origin.clone(),
                    name,
                    full_name: full_name.clone(),
                    parents: parent.map(String::from).into_iter().collect(),
                    opaque_children: false,
                    fields: fields.clone(),
                };
                if let Some(existing) = self.boundaries.iter_mut().find(|b| {
                    b.entry == boundary.entry
                        && b.call == boundary.call
                        && b.name == boundary.name
                        && b.full_name == boundary.full_name
                        && b.fields == boundary.fields
                }) {
                    if let Some(parent) = parent
                        && !existing.parents.iter().any(|p| p == parent)
                    {
                        existing.parents.push(parent.into());
                    }
                    continue;
                }
                self.boundaries.push(boundary);
                // Index only dependency fields. Other recipe attributes may call
                // fetchers or retain unrelated packages without constituting these
                // dependency edges.
                for name in DEPENDENCIES {
                    if let Some(value) = member(&attrs, name) {
                        self.walk(value, entry, Some(&full_name), depth - 1);
                    }
                }
            }
            return;
        }
        if let ValueKind::Apply(..) = &e.node.kind {
            let (callee, args) = applications(&e);
            let callee = head(callee, 48);
            if package_scope_projections(&callee, &args) {
                return;
            }
            if args.len() == 2 && (library(&callee, "optional") || library(&callee, "optionals")) {
                if boolean(args[0].clone(), 48) == Some(false) {
                    return;
                }
                self.walk(args[1].clone(), entry, parent, depth - 1);
                return;
            }
            if args.len() == 1
                && (namespace(&callee, Space::Builtins, &["concatLists"])
                    || library(&callee, "concatLists")
                    || library(&callee, "flatten"))
            {
                self.walk(args[0].clone(), entry, parent, depth - 1);
                return;
            }
            // An unmodelled helper may reconstruct/override package values.
            // Its arguments are not evidence for the resulting backend owner.
            self.opaque_child(entry, parent);
            return;
        }
        if matches!(&e.node.kind, ValueKind::Reference(r) if matches!(r.source, Source::Input { .. }))
        {
            self.opaque_child(entry, parent);
        }
        let mut visit = |node| self.walk(expr(node, &e.env), entry, parent, depth - 1);
        match &e.node.kind {
            ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => {
                for (_, node) in fields {
                    visit(node);
                }
            }
            ValueKind::List(items) => {
                for node in items {
                    visit(node);
                }
            }
            ValueKind::Apply(a, b)
            | ValueKind::AttrMerge(a, b)
            | ValueKind::Equal(a, b)
            | ValueKind::Divide(a, b) => {
                visit(a);
                visit(b);
            }
            ValueKind::Select(base, _)
            | ValueKind::ToText(base)
            | ValueKind::StringPrefix { value: base, .. }
            | ValueKind::PathAppend { value: base, .. }
            | ValueKind::FilesystemPath(base)
            | ValueKind::InRange { value: base, .. } => visit(base),
            ValueKind::If(_, b, c) => {
                visit(b);
                visit(c);
            }
            ValueKind::Parameter(_) => self.opaque_child(entry, parent),
            _ => {}
        }
    }
}

// A failure in an assertion's condition must not be attributed to its still
// unforced body. Record condition roots: a shared accessor may occur in both
// the guard and body, but only the condition's ancestry proves guard evaluation.
fn guard_operations(node: &Node, out: &mut BTreeSet<String>, remaining: &mut usize) -> bool {
    if *remaining == 0 {
        return false;
    }
    *remaining -= 1;
    let mut visit = |node| guard_operations(node, out, remaining);
    match &node.kind {
        ValueKind::Assert(condition, value) => {
            out.insert(condition.origin.id.clone());
            guard_operations(condition, out, remaining) && guard_operations(value, out, remaining)
        }
        ValueKind::List(items) => items.iter().all(visit),
        ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => {
            fields.iter().all(|(_, n)| visit(n))
        }
        ValueKind::Apply(a, b)
        | ValueKind::AttrMerge(a, b)
        | ValueKind::Equal(a, b)
        | ValueKind::Divide(a, b) => visit(a) && visit(b),
        ValueKind::Select(value, _)
        | ValueKind::ToText(value)
        | ValueKind::StringPrefix { value, .. }
        | ValueKind::PathAppend { value, .. }
        | ValueKind::FilesystemPath(value)
        | ValueKind::InRange { value, .. } => visit(value),
        ValueKind::Function { body, .. } => visit(body),
        ValueKind::FunctionAttrs { defaults, body, .. } => {
            defaults.iter().all(|(_, n)| visit(n)) && visit(body)
        }
        ValueKind::If(a, b, c) => visit(a) && visit(b) && visit(c),
        ValueKind::Reference(reference) => match &reference.source {
            Source::Packages { overlays } | Source::NixosPackages { overlays } => {
                overlays.iter().all(visit)
            }
            _ => true,
        },
        _ => true,
    }
}

pub(crate) fn collect(config: &Config) -> Option<serde_json::Value> {
    let mut guards = BTreeSet::new();
    let mut remaining = 20_000;
    for assignment in &config.assignments {
        if !guard_operations(&assignment.value, &mut guards, &mut remaining) {
            return None;
        }
    }
    let mut collector = Collector {
        boundaries: Vec::new(),
        remaining: 20_000,
    };
    for assignment in &config.assignments {
        let root = expr(&assignment.value, &Rc::new(Vec::new()));
        // Only a known package demand establishes the active diagnostic scope.
        if let Some(pkg) = package(root, 64) {
            collector.walk(Term::Expr(pkg), &assignment.origin, None, 64);
        }
    }
    (!collector.boundaries.is_empty()).then(|| {
        serde_json::to_value(Metadata {
            version: 1,
            revision: REVISION.into(),
            boundaries: collector.boundaries,
            guard_operations: guards.into_iter().collect(),
        })
        .expect("compiler-owned backend metadata")
    })
}

pub(crate) fn read(value: &Option<serde_json::Value>) -> Option<Metadata> {
    let metadata: Metadata = serde_json::from_value(value.as_ref()?.clone()).ok()?;
    (metadata.version == 1 && metadata.revision == REVISION).then_some(metadata)
}

pub(crate) fn dependency_field(name: &str) -> bool {
    DEPENDENCIES.contains(&name)
}
