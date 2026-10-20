//! Construct typed configuration and deferred expressions in Rust.
use crate::ir::validation::reject_nul;
use crate::ir::{Assignment, IntoNode, Node, Origin, ValueKind};
use crate::{interop, ir};
use std::marker::PhantomData;

/// An expression that Nix will evaluate later, with an expected Rust result type.
///
/// For example, [`Expr<i64>`] describes an integer expression and provides
/// integer operations such as [`Self::divide`]. Calling those methods constructs
/// more expressions; it does not compute or read the result in Rust. Use
/// [`interop::raw::NixValue`] when the value’s category should remain unspecified.
///
/// `T` restricts Rust composition. For references to external Nix values, it is
/// an expectation rather than proof of their actual types; Nix or NixOS checks
/// those during evaluation. Child operations retain their Rust source locations.
/// Boolean expressions support `!` and short-circuit [`Self::and`], [`Self::or`]
/// and [`Self::implies`].
///
/// Integer and boolean expressions cannot be interchanged:
/// ```compile_fail,E0308
/// use rusix::Expr;
///
/// fn main() {
///     let _ = Expr::int(22).divide(Expr::boolean(true));
/// }
/// ```
#[derive(Clone, Debug)]
pub struct Expr<T> {
    /// The literal or computation Nix should evaluate, with its Rust source location.
    node: Node,
    /// The expected result category used to restrict Rust operations, not an evaluated value.
    ty: PhantomData<T>,
}

impl<T> Expr<T> {
    pub(crate) fn new(kind: ValueKind, origin: Origin) -> Self {
        Self {
            node: Node { origin, kind },
            ty: PhantomData,
        }
    }
}

impl Expr<i64> {
    /// Describe decimal text conversion of this integer using Nix’s `toString`.
    /// The result is an [`Expr<String>`]; Rust does not read or format the integer.
    #[track_caller]
    pub fn to_text(self) -> Expr<String> {
        Expr::new(
            ValueKind::ToText(Box::new(self.node)),
            Origin::caller("integer to text"),
        )
    }

    /// Embed a concrete integer while recording its Rust source origin.
    #[track_caller]
    pub fn int(value: i64) -> Self {
        Self::new(ValueKind::Int(value), Origin::caller("integer literal"))
    }

    /// Describe integer division for Nix to compute later.
    /// This uses Nix’s signed integer division. Division by zero is reported when
    /// Nix evaluates the result, with this Rust call as the consuming operation.
    #[track_caller]
    pub fn divide(self, denominator: Self) -> Self {
        Self::new(
            ValueKind::Divide(Box::new(self.node), Box::new(denominator.node)),
            Origin::caller("integer division"),
        )
    }

    /// Require the integer to be between `min` and `max`, inclusive, when Nix
    /// evaluates it. A rejected value fails with `message` at this Rust operation.
    /// The result remains an integer expression; no range check happens in Rust.
    #[track_caller]
    pub fn in_range(self, min: i64, max: i64, message: impl Into<String>) -> Self {
        Self::new(
            ValueKind::InRange {
                value: Box::new(self.node),
                min,
                max,
                message: message.into(),
            },
            Origin::caller("integer range constraint"),
        )
    }
}

impl Expr<String> {
    /// Place concrete text before this expression’s string when Nix evaluates it.
    /// Nix strings also track dependencies on package outputs; concatenation retains
    /// those dependencies. Rust does not read the string value.
    #[track_caller]
    pub fn with_prefix(self, prefix: impl Into<String>) -> Self {
        Self::new(
            ValueKind::StringPrefix {
                prefix: prefix.into(),
                value: Box::new(self.node),
            },
            Origin::caller("symbolic string prefix"),
        )
    }
}

impl Expr<bool> {
    /// Embed a concrete boolean while recording its Rust source origin.
    #[track_caller]
    pub fn boolean(value: bool) -> Self {
        Self::new(ValueKind::Bool(value), Origin::caller("boolean literal"))
    }

    /// Construct boolean AND for Nix to evaluate later.
    /// Nix checks the operands as booleans and evaluates the right operand only if
    /// the left is true. This uses Nix conditionals, independently of nixpkgs `lib`.
    #[track_caller]
    pub fn and(self, other: Self) -> Self {
        let other = interop::raw::NixValue::if_else(other, true, false);
        interop::raw::NixValue::if_else(self, other, false).into_expr()
    }

    /// Construct boolean OR for Nix to evaluate later.
    /// Nix checks demanded operands as booleans and evaluates the right operand
    /// only when the left is false, independently of nixpkgs `lib`.
    #[track_caller]
    pub fn or(self, other: Self) -> Self {
        let other = interop::raw::NixValue::if_else(other, true, false);
        interop::raw::NixValue::if_else(self, true, other).into_expr()
    }

    /// Construct boolean implication: the right operand must be true if the left is true.
    /// Nix checks demanded operands as booleans. A false left operand returns true
    /// without evaluating the right operand, independently of nixpkgs `lib`.
    #[track_caller]
    pub fn implies(self, other: Self) -> Self {
        (!self).or(other)
    }
}

/// Keeps boolean negation typed without evaluating the expression in Rust.
impl std::ops::Not for Expr<bool> {
    type Output = Self;

    #[track_caller]
    fn not(self) -> Self {
        (!interop::raw::NixValue::from(self)).into_expr()
    }
}

// A sealed conversion keeps arbitrary syntax and incorrectly typed Expr<T> out.
pub(crate) mod sealed {
    pub trait Sealed {}
}

/// A supported value that can be used directly in a derived configuration field,
/// [`Config::set_dynamic`] or a Nix call.
/// Built-in Rust literals, supported [`Expr`] types and Nix references implement
/// this trait. It is sealed, so users cannot add implementations. For your own
/// configuration types, derive or implement [`crate::IntoRusixValue`] instead.
pub trait ConfigValue: sealed::Sealed + ir::IntoNode {}

macro_rules! primitive {
    ($ty:ty, $variant:ident) => {
        impl sealed::Sealed for $ty {}

        impl ConfigValue for $ty {}

        impl IntoNode for $ty {
            fn into_node(self, origin: Origin) -> Node {
                Node {
                    origin,
                    kind: ValueKind::$variant(self.into()),
                }
            }
        }
    };
}

primitive!(bool, Bool);

primitive!(i32, Int);

primitive!(i64, Int);

primitive!(u8, Int);

primitive!(u16, Int);

primitive!(u32, Int);

primitive!(i8, Int);

primitive!(i16, Int);

primitive!(f64, Float);

primitive!(String, String);

primitive!(&str, String);

impl<T: ConfigValue> sealed::Sealed for Vec<T> {}

impl<T: ConfigValue> ConfigValue for Vec<T> {}

impl<T: ConfigValue> IntoNode for Vec<T> {
    fn into_node(self, origin: Origin) -> Node {
        let children = self
            .into_iter()
            .enumerate()
            .map(|(i, value)| {
                let child = Origin::new(
                    &origin.file,
                    origin.line,
                    origin.column,
                    format!("{}[{i}]", origin.purpose),
                );
                value.into_node(child)
            })
            .collect();
        Node {
            origin,
            kind: ValueKind::List(children),
        }
    }
}

macro_rules! expression_value {
    ($ty:ty) => {
        impl sealed::Sealed for Expr<$ty> {}

        impl ConfigValue for Expr<$ty> {}

        impl IntoNode for Expr<$ty> {
            fn into_node(self, _: Origin) -> Node {
                self.node
            }
        }
    };
}

expression_value!(i64);

expression_value!(bool);

expression_value!(String);

/// A group of settings that one component contributes to configuration.
/// For example, it can define `services.example.enable` and
/// `services.example.port`. Values may be concrete Rust literals or expressions
/// that Nix will evaluate later; constructing Config does not run Nix.
///
/// Prefer [`crate::config`] or the [`IntoConfig`] derive for ordinary Rust authoring.
/// [`Self::set_dynamic`] is the escape hatch for paths chosen at runtime.
///
/// A NixOS module supplies settings to a larger system configuration. Combine
/// independent groups with [`crate::nixos::NixosModule::add`] so NixOS can merge them.
/// Repeated or overlapping paths inside a single Config are validation errors;
/// separate contributions can define the same option under NixOS’s merge rules.
#[derive(Clone, Debug)]
pub struct Config {
    /// Source location at which this contribution was created.
    pub origin: Origin,
    /// Ordered bindings belonging to this contribution, before backend merging.
    pub assignments: Vec<Assignment>,
    /// Rust conversion failure retained for validate or compilation to report later.
    pub(crate) error: Option<ValidationError>,
}

/// Convert a complete Rust configuration component into a group of settings.
/// The component defines where its fields belong: a root with a `services`
/// field, for example, defines paths starting with `services`. Reusable nested
/// values use [`crate::IntoRusixValue`] and let their parent choose placement.
///
/// Use the [`crate::config`] macro for local types or derive this trait for reusable
/// root types. [`crate::nixos::NixosModule::add`] accepts either approach. Conversion
/// runs in Rust now, but generated expressions and NixOS merging happen later.
/// Derived conversion saves structural errors in the returned [`Config`] for
/// [`Config::validate`] or compilation to report. Returning `Config` without a
/// `Result` does not guarantee validity. By contrast,
/// [`crate::IntoRusixValue::try_into_nix_value`] reports invalid flattening immediately
/// when constructing one Nix value for a function argument.
///
/// For custom implementations, `#[track_caller]` propagates the author’s call
/// location through tracked helpers. Existing expression locations are retained;
/// untracked helper calls stop that location propagation.
pub trait IntoConfig {
    /// Turn this component into settings at their complete configuration paths.
    /// This describes the settings without evaluating expressions or combining them
    /// with other NixOS contributions.
    #[track_caller]
    fn into_config(self) -> Config;
}

/// Allow an explicit generic escape-hatch contribution alongside typed models.
impl IntoConfig for Config {
    fn into_config(self) -> Config {
        self
    }
}

/// Reuse an already lowered contribution without consuming it.
impl IntoConfig for &Config {
    fn into_config(self) -> Config {
        self.clone()
    }
}

impl Config {
    /// Start an empty contribution, capturing its caller for provenance.
    #[track_caller]
    pub fn new() -> Self {
        Self {
            origin: Origin::caller("configuration"),
            assignments: Vec::new(),
            error: None,
        }
    }

    /// Assign a value at a dotted configuration path chosen at runtime.
    /// Prefer a struct deriving [`IntoConfig`] for statically known fields: Rust
    /// cannot check this path or its expected value type. This escape hatch also
    /// supports deliberately invalid configurations in diagnostic fixtures.
    /// The value is described now and evaluated by Nix later. [`Self::validate`]
    /// checks paths and duplicates; NixOS checks option existence and actual types.
    /// Use structural authoring for a literal field name containing a dot.
    #[track_caller]
    pub fn set_dynamic(mut self, path: impl Into<String>, value: impl ConfigValue) -> Self {
        let path = path.into();
        let origin = Origin::caller(format!("set {path}"));
        let value_origin = Origin::caller(format!("value of {path}"));
        self.assignments.push(Assignment {
            origin,
            segments: path.split('.').map(str::to_owned).collect(),
            path,
            value: value.into_node(value_origin),
        });
        self
    }

    /// Check that this contribution can be represented unambiguously in generated Nix.
    /// This runs in Rust and rejects conflicting paths, invalid strings or float
    /// literals, and callback parameters used outside their functions. It does not
    /// run Nix or check NixOS option existence, types or assertions.
    pub fn validate(&self) -> Result<(), ValidationError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }

        let mut seen: Vec<&[String]> = Vec::new();

        for assignment in &self.assignments {
            let path = &assignment.path;
            reject_nul(path, &assignment.origin)?;
            let parts = assignment.path_segments();
            if parts.is_empty()
                || parts
                    .iter()
                    .any(|part| part.is_empty() || part.contains('\0'))
            {
                return Err(ValidationError {
                    origin: assignment.origin.clone(),
                    message: "option paths must have nonempty dot-separated segments".into(),
                });
            }

            if seen
                .iter()
                .any(|old| parts.starts_with(old) || old.starts_with(parts))
            {
                return Err(ValidationError {
                    origin: assignment.origin.clone(),
                    message: format!("duplicate or conflicting option path: {path}"),
                });
            }

            seen.push(parts);
            assignment.value.validate()?;
        }

        Ok(())
    }
}

impl Default for Config {
    #[track_caller]
    fn default() -> Self {
        Self::new()
    }
}

/// A configuration-description error found by Rust before Nix evaluation.
/// It identifies the Rust operation and explains why its description cannot be
/// compiled. NixOS option type errors are checked later by NixOS instead.
#[derive(Clone, Debug)]
pub struct ValidationError {
    /// Rust operation associated with the rejected value or binding.
    pub origin: Origin,
    /// Actionable reason that describes the violated invariant.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_and_ids_are_deterministic() {
        fn make() -> Config {
            Config::new().set_dynamic("a", true)
        }

        let first = make();
        let second = make();

        assert_eq!(first.assignments[0].origin, second.assignments[0].origin);
        assert_eq!(first.assignments[0].origin.file, file!());
        assert_ne!(first.origin.id, first.assignments[0].origin.id);
    }

    #[test]
    fn paths_are_validated_in_both_prefix_directions() {
        for paths in [["a", "a"], ["a", "a.b"], ["a.b", "a"], ["a..b", "x"]] {
            assert!(
                Config::new()
                    .set_dynamic(paths[0], true)
                    .set_dynamic(paths[1], 1)
                    .validate()
                    .is_err()
            );
        }
        assert!(
            Config::new()
                .set_dynamic("a.b", true)
                .set_dynamic("a.c", vec![22])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn unsupported_nul_is_rejected_at_its_origin() {
        let config = Config::new().set_dynamic("strings", vec!["valid", "bad\0string"]);
        let error = config.validate().unwrap_err();

        let ValueKind::List(items) = &config.assignments[0].value.kind else {
            panic!()
        };

        assert_eq!(error.origin, items[1].origin);
        assert!(
            Config::new()
                .set_dynamic("bad\0path", true)
                .validate()
                .is_err()
        );
        assert!(
            Config::new()
                .set_dynamic("number", Expr::int(1).in_range(0, 2, "bad\0message"))
                .validate()
                .is_err()
        );
    }
}
