//! Validate deferred expressions without evaluating Nix.
use super::{Node, Origin, ValueKind};
use crate::ValidationError;

pub(crate) fn reject_nul(text: &str, origin: &Origin) -> Result<(), ValidationError> {
    if text.contains('\0') {
        return Err(ValidationError {
            origin: origin.clone(),
            message: "NUL bytes are not supported in configuration strings".into(),
        });
    }

    Ok(())
}

pub(crate) fn validate_scoped(node: &Node, scope: &[u64]) -> Result<(), ValidationError> {
    let validate_value = |node| validate_scoped(node, scope);

    match &node.kind {
        ValueKind::String(text) => reject_nul(text, &node.origin)?,
        ValueKind::List(items) => {
            for item in items {
                validate_value(item)?;
            }
        }
        ValueKind::AttrSet(fields) | ValueKind::OpaqueRecord(fields) => {
            let mut seen = std::collections::BTreeSet::new();

            for (name, value) in fields {
                let opaque = matches!(node.kind, ValueKind::OpaqueRecord(_));
                let origin = if opaque { &node.origin } else { &value.origin };
                reject_nul(name, origin)?;

                if (!opaque && name.is_empty()) || !seen.insert(name) {
                    return Err(ValidationError {
                        origin: origin.clone(),
                        message: format!("invalid or duplicate record field: {name}"),
                    });
                }

                validate_value(value)?;
            }
        }
        ValueKind::Divide(left, right)
        | ValueKind::Apply(left, right)
        | ValueKind::AttrMerge(left, right)
        | ValueKind::Assert(left, right)
        | ValueKind::Equal(left, right) => {
            validate_value(left)?;
            validate_value(right)?;
        }
        ValueKind::InRange { value, message, .. } => {
            reject_nul(message, &node.origin)?;
            validate_value(value)?;
        }
        ValueKind::Float(value) if !value.is_finite() || value.is_subnormal() => {
            return Err(ValidationError {
                origin: node.origin.clone(),
                message: "Nix float literals require finite normal values or zero (no NaN, infinity, or subnormals)".into(),
            });
        }
        ValueKind::Bool(_) | ValueKind::Int(_) | ValueKind::Float(_) | ValueKind::Null => {}
        ValueKind::Reference(reference) => reference.validate_scoped(scope)?,
        ValueKind::Select(value, path) => {
            validate_value(value)?;
            path.validate(&node.origin)?;
        }
        ValueKind::Function { binding, body } => {
            let mut scope = scope.to_vec();
            scope.push(*binding);
            validate_scoped(body, &scope)?;
        }
        ValueKind::FunctionAttrs {
            binding,
            arguments,
            defaults,
            body,
            ..
        } => {
            let mut names = std::collections::BTreeSet::new();
            for name in arguments {
                let mut chars = name.chars();
                let first = chars
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
                let rest = chars.all(|c| c.is_ascii_alphanumeric() || "_-'".contains(c));
                if !first
                    || !rest
                    || name.starts_with("__rusix_")
                    || [
                        "if", "then", "else", "assert", "with", "let", "in", "rec", "inherit",
                    ]
                    .contains(&name.as_str())
                    || !names.insert(name)
                {
                    return Err(ValidationError {
                        origin: node.origin.clone(),
                        message: format!("invalid or duplicate Nix function argument: {name}"),
                    });
                }
            }
            let mut scope = scope.to_vec();
            scope.push(*binding);
            let mut seen = std::collections::BTreeSet::new();
            for (name, value) in defaults {
                if !names.contains(name) || !seen.insert(name) {
                    return Err(ValidationError {
                        origin: node.origin.clone(),
                        message: format!("unknown or duplicate Nix function default: {name}"),
                    });
                }
                validate_scoped(value, &scope)?;
            }
            validate_scoped(body, &scope)?;
        }
        ValueKind::Parameter(binding) => {
            if !scope.contains(binding) {
                return Err(ValidationError {
                    origin: node.origin.clone(),
                    message: "opaque callback parameter escaped its function scope".into(),
                });
            }
        }
        ValueKind::If(condition, yes, no) => {
            validate_value(condition)?;
            validate_value(yes)?;
            validate_value(no)?;
        }
        ValueKind::OptionReference(path) => path.validate(&node.origin)?,
        ValueKind::ToText(value) | ValueKind::FilesystemPath(value) => validate_value(value)?,
        ValueKind::PathAppend { value, suffix } => {
            reject_nul(suffix, &node.origin)?;
            if !suffix.starts_with('/')
                || suffix[1..]
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
            {
                return Err(ValidationError {
                    origin: node.origin.clone(),
                    message: "invalid relative path suffix".into(),
                });
            }
            validate_value(value)?;
        }
        ValueKind::StringPrefix { prefix, value } => {
            reject_nul(prefix, &node.origin)?;
            validate_value(value)?;
        }
    }

    Ok(())
}
