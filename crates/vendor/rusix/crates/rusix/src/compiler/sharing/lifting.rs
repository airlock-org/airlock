//! Keep argument-independent work in the function's enclosing lazy environment.
//!
//! A shared function can still repeat its invariant computations on every call.
//! Move substantial expressions outside that function only when their free
//! variables exclude its arguments. Existing lets remain barriers, so recursive
//! bindings and resolved argument records cannot escape their original scope.

use super::{Names, NixExpr, NixKind, Origin, Region, boundary, children, children_mut, rewrite};
use std::collections::{HashMap, HashSet};

/// Lexical dependencies and whether evaluation allocates or computes beyond a literal.
#[derive(Clone, Default)]
struct Dependencies {
    /// Names looked up outside this expression's own binders.
    free: HashSet<String>,
    /// Whether evaluation constructs a container or performs an operation beyond a literal.
    computes: bool,
}

/// Analyze lexical dependencies, including captures inside nested functions and recursive lets.
fn analyze(expr: &NixExpr, summaries: &mut HashMap<*const NixExpr, Dependencies>) -> Dependencies {
    if let Some(summary) = summaries.get(&(expr as *const NixExpr)) {
        return summary.clone();
    }
    let mut result = Dependencies::default();
    for child in children(expr) {
        let child = analyze(child, summaries);
        result.free.extend(child.free);
        result.computes |= child.computes;
    }
    match &expr.kind {
        NixKind::Variable(name) => {
            result.free.insert(name.clone());
        }
        NixKind::Lambda(name, _) | NixKind::Let(name, ..) => {
            result.free.remove(name);
        }
        NixKind::Function(names, _) => {
            for name in names {
                result.free.remove(name);
            }
        }
        NixKind::ArgumentFunction(arguments, _) | NixKind::OpenArgumentFunction(arguments, _) => {
            for (name, _) in arguments {
                result.free.remove(name);
            }
        }
        NixKind::Apply(..)
        | NixKind::Call(..)
        | NixKind::Binary(..)
        | NixKind::If(..)
        | NixKind::Assert(..)
        | NixKind::List(..)
        | NixKind::AttrSet(..)
        | NixKind::Select(..)
        | NixKind::ArgumentSelect(..) => result.computes = true,
        _ => {}
    }
    summaries.insert(expr, result.clone());
    result
}

/// Choose maximal invariant subtrees instead of introducing a binding for every child operation.
fn candidates(
    expr: &NixExpr,
    region: &Region<'_>,
    summaries: &HashMap<*const NixExpr, Dependencies>,
    parameters: &HashSet<String>,
    selected: &mut [bool],
) {
    if boundary(expr) {
        return;
    }
    let identity = region.identities[&(expr as *const NixExpr)];
    let dependencies = &summaries[&(expr as *const NixExpr)];
    if region.occurrences[identity].weight >= 16
        && dependencies.computes
        && dependencies.free.is_disjoint(parameters)
    {
        selected[identity] = true;
        return;
    }
    for child in children(expr) {
        candidates(child, region, summaries, parameters, selected);
    }
}

/// Preserve exported function syntax while sharing internal function invariants.
pub(super) fn lift(expr: &mut NixExpr, names: &mut Names) {
    walk(expr, names, false, &mut HashMap::new());
}

/// Attribute consumers such as Nix's flake loader inspect function syntax before forcing thunks.
fn walk(
    expr: &mut NixExpr,
    names: &mut Names,
    may_lift: bool,
    summaries: &mut HashMap<*const NixExpr, Dependencies>,
) {
    match &mut expr.kind {
        NixKind::AttrSet(fields) => {
            for (_, value) in fields {
                walk(value, names, false, summaries);
            }
        }
        NixKind::Group(value) => walk(value, names, may_lift, summaries),
        _ => {
            for child in children_mut(expr) {
                walk(child, names, true, summaries);
            }
        }
    }
    // Lifting preserves an expression's free variables. Cache that original
    // summary before rewriting it; introduced lets are opaque to parent regions,
    // so moved descendants never require new address-based cache entries.
    analyze(expr, summaries);
    if !may_lift {
        return;
    }
    let parameters = match &expr.kind {
        NixKind::Lambda(name, _) => HashSet::from([name.clone()]),
        NixKind::Function(names, _) => names.iter().cloned().collect(),
        NixKind::ArgumentFunction(arguments, _) | NixKind::OpenArgumentFunction(arguments, _) => {
            arguments.iter().map(|(name, _)| name.clone()).collect()
        }
        _ => return,
    };
    let origin = expr.origin.clone();
    let body = match &mut expr.kind {
        NixKind::Lambda(_, body)
        | NixKind::Function(_, body)
        | NixKind::ArgumentFunction(_, body)
        | NixKind::OpenArgumentFunction(_, body) => body,
        _ => unreachable!("function identified above"),
    };
    let mut region = Region::default();
    let mut enclosing: Vec<_> = origin.as_ref().into_iter().collect();
    region.index(body, &mut enclosing);
    let mut selected = vec![false; region.occurrences.len()];
    candidates(body, &region, summaries, &parameters, &mut selected);
    if !selected.iter().any(|selected| *selected) {
        return;
    }

    let identities = region.identities;
    let ancestry: HashMap<_, Vec<Origin>> = region
        .keys
        .into_iter()
        .filter(|(_, identity)| selected[*identity])
        .map(|(key, identity)| (identity, key.enclosing.into_iter().cloned().collect()))
        .collect();
    let mut definitions = Vec::new();
    rewrite(
        body,
        &identities,
        &selected,
        &ancestry,
        &mut HashMap::new(),
        &mut definitions,
        names,
    );
    let mut function = std::mem::replace(expr, NixExpr::plain(NixKind::Null));
    for (name, value) in definitions.into_iter().rev() {
        function = NixExpr::plain(NixKind::Let(name, Box::new(value), Box::new(function)));
    }
    *expr = function;
}
