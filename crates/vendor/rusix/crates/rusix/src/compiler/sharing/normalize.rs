//! Expose lexical bindings and literal inputs hidden behind immediate lambda calls.
//!
//! A lambda application already binds its argument lazily. Express that binding
//! as a fresh `let`, preserving sharing while making its scope visible to the
//! optimizer. Only literal values are propagated; opaque calls remain deferred.

use super::{IMPLICIT_NAMES, Names, NixExpr, NixKind, children_mut};
use std::collections::HashMap;

/// Report whether a callee is a known lambda with no demanding wrapper to preserve.
fn lambda(expr: &NixExpr) -> bool {
    if expr.error_context {
        return false;
    }
    match &expr.kind {
        // Native builtin calls and diagnostic wrappers contain implicit
        // references to this name, which ordinary variable renaming cannot see.
        NixKind::Lambda(name, _) => !IMPLICIT_NAMES.contains(&name.as_str()),
        NixKind::Group(value) => lambda(value),
        _ => false,
    }
}

/// Rename references to one binder, stopping where another declaration shadows it.
fn rename(expr: &mut NixExpr, from: &str, to: &str) {
    match &mut expr.kind {
        NixKind::Variable(name) if name == from => {
            *name = to.to_owned();
            return;
        }
        NixKind::Lambda(name, _) | NixKind::Let(name, ..) if name == from => return,
        NixKind::Function(names, _) if names.iter().any(|name| name == from) => return,
        NixKind::ArgumentFunction(arguments, _) | NixKind::OpenArgumentFunction(arguments, _)
            if arguments.iter().any(|(name, _)| name == from) =>
        {
            return;
        }
        _ => {}
    }
    for child in children_mut(expr) {
        rename(child, from, to);
    }
}

/// Replace an immediate lambda application with an equivalent capture-free lazy binding.
fn bind_application(expr: &mut NixExpr, names: &mut Names) {
    let NixKind::Apply(function, _) = &expr.kind else {
        return;
    };
    if !lambda(function) {
        return;
    }

    let NixKind::Apply(mut function, argument) = std::mem::replace(&mut expr.kind, NixKind::Null)
    else {
        unreachable!("application identified above")
    };
    let mut ancestry = Vec::new();
    let (parameter, mut body) = loop {
        if let Some(origin) = function.origin {
            ancestry.push(origin);
        }
        match function.kind {
            NixKind::Group(value) => function = value,
            NixKind::Lambda(name, body) => break (name, body),
            _ => unreachable!("known lambda callee"),
        }
    };
    let binding = names.fresh();
    rename(&mut body, &parameter, &binding);
    for origin in ancestry.into_iter().rev() {
        body = Box::new(NixExpr::attributed(NixKind::Group(body), origin));
    }
    expr.kind = NixKind::Let(binding, argument, body);
}

/// Copy only small values that neither evaluate an operation nor capture another lexical binding.
fn literal(expr: &NixExpr) -> Option<NixKind> {
    if expr.error_context && expr.origin.is_some() {
        return None;
    }
    match &expr.kind {
        NixKind::Group(value) => literal(value),
        // Boolean/null tokens and negative numerals resolve prelude names in
        // Nix. Copying them into a shadowing scope could change their values.
        NixKind::Int(value) if *value >= 0 => Some(expr.kind.clone()),
        NixKind::Float(value) if !value.is_sign_negative() => Some(expr.kind.clone()),
        // Platform names benefit from propagation. Large embedded documents
        // should retain their original lazy binding rather than grow per use.
        NixKind::String(value) if value.len() <= 64 => Some(expr.kind.clone()),
        _ => None,
    }
}

/// Normalize lexical calls and literal references with native recursive shadowing rules.
fn walk(
    expr: &mut NixExpr,
    names: &mut Names,
    constants: &mut HashMap<String, Vec<Option<NixKind>>>,
) {
    bind_application(expr, names);
    if let NixKind::Variable(name) = &expr.kind {
        if let Some(Some(value)) = constants.get(name).and_then(|stack| stack.last()) {
            expr.kind = value.clone();
        }
        return;
    }

    let declarations: Vec<_> = match &expr.kind {
        NixKind::Lambda(name, _) | NixKind::Let(name, ..) => vec![name.clone()],
        NixKind::Function(names, _) => names.clone(),
        NixKind::ArgumentFunction(arguments, _) | NixKind::OpenArgumentFunction(arguments, _) => {
            arguments.iter().map(|(name, _)| name.clone()).collect()
        }
        _ => Vec::new(),
    };
    for name in &declarations {
        constants.entry(name.clone()).or_default().push(None);
    }

    if let NixKind::Let(name, value, body) = &mut expr.kind {
        // The initializer is recursive too. Never substitute an outer value for
        // its shadowing declaration before inspecting the initializer itself.
        walk(value, names, constants);
        *constants
            .get_mut(name)
            .expect("entered let")
            .last_mut()
            .expect("visible declaration") = literal(value);
        walk(body, names, constants);
    } else {
        for child in children_mut(expr) {
            walk(child, names, constants);
        }
    }

    for name in &declarations {
        constants.get_mut(name).expect("entered declaration").pop();
    }
}

/// Expose lazy call bindings without evaluating defaults, imports, opaque functions or branches.
pub(super) fn normalize(expr: &mut NixExpr, names: &mut Names) {
    walk(expr, names, &mut HashMap::new());
}
