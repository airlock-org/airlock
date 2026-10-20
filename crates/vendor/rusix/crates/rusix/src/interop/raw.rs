//! Explicit dynamic Nix interop for adapters without a typed interface.
//!
//! Prefer the typed handles in [`super`] for normal authoring. This module
//! contains dynamic selection/application and unchecked interface expectations.
//! These operations remain lazy; Nix checks actual values when demanded.
use super::{ModuleRef, NixCallable, NixExpression, NixFunction, Nixpkgs, OverlayRef, PackageRef};
use crate::ir::{Reference, Source};
use crate::{
    ConfigValue, ValidationError,
    ir::{IntoNode, Node, Origin, ValueKind},
    sealed,
};

/// Representation access for custom symbolic interfaces and dynamic adapters.
///
/// Implementations must retain the expression and its Rust origins. Attaching
/// an interface states an expectation; it neither evaluates nor validates Nix.
/// Normal authors import [`super::NixExpression`] for typed lazy operations.
pub trait NixRepresentation {
    /// Attach an unchecked expected interface to an existing expression.
    fn from_expression(value: NixValue) -> Self;

    /// Erase the interface explicitly for dynamic Nix interop.
    fn as_expression(&self) -> NixValue;
}

/// Attach a symbolic interface at a raw boundary, without evaluating Nix.
/// Prefer typed constructors and calls when the expression category is known.
pub fn expect<T: super::NixExpression>(value: NixValue) -> T {
    T::from_expression(value)
}

/// A sequence of field names to look up in Nix attribute sets.
/// For example, `["services", "example", "port"]` describes
/// `services.example.port`. Names are data, never executable Nix source.
/// Use [`Self::segments`] when a field name itself contains a dot.
#[derive(Clone, Debug)]
pub struct AttrPath(
    /// Ordered literal field names; dots and other punctuation inside a name stay intact.
    pub(crate) Vec<String>,
);

impl AttrPath {
    /// Split a convenience dotted path; literal dots require [`Self::segments`].
    /// Empty or NUL-containing segments are rejected during IR validation.
    pub fn dotted(path: &str) -> Self {
        Self(path.split('.').map(str::to_owned).collect())
    }

    /// Construct a path from literal names, preserving punctuation within each segment.
    pub fn segments(parts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(parts.into_iter().map(Into::into).collect())
    }

    /// Inspect literal segments without resolving the referenced Nix value.
    pub fn parts(&self) -> &[String] {
        &self.0
    }

    pub(crate) fn validate(&self, origin: &Origin) -> Result<(), ValidationError> {
        if self.0.is_empty() || self.0.iter().any(|p| p.is_empty() || p.contains('\0')) {
            return Err(ValidationError {
                origin: origin.clone(),
                message: "Nix attribute paths need nonempty, NUL-free segments".into(),
            });
        }

        Ok(())
    }
}

/// A Rust description of a value or expression that Nix will evaluate later.
///
/// This can represent a literal such as `true` or `"hello"`, a list or record,
/// a package lookup, or a function call. Constructing it does not run Nix or
/// read a result into Rust. Even a value created from a concrete Rust literal
/// is stored as a description to include in generated Nix.
///
/// An attribute set is Nix’s collection of named fields, like a Rust record or
/// map. [`Self::record`] and [`Self::list`] can mix literals, package references
/// and expressions that depend on final NixOS options. These objects remain
/// Nix values; they are not converted to strings. Use [`Self::to_text`] when
/// conversion to text is intended.
///
/// Unlike [`crate::Expr<T>`], this wrapper does not promise a particular Rust
/// result type. Nix checks field existence, function arguments and actual value
/// types when it evaluates the expression. Rusix also records the Rust operations
/// that constructed it, so errors can be mapped back to source.
/// To construct one record from a user-defined Rust struct, derive
/// [`crate::IntoRusixValue`] and call
/// [`try_into_nix_value`](crate::IntoRusixValue::try_into_nix_value).
/// [`crate::RusixValue`] is the structural intermediate used by custom
/// conversions, rather than the value passed to Nix functions.
///
/// ```
/// use rusix::interop::{raw::NixValue, Nixpkgs};
/// let args = NixValue::record([
///     ("enabled", true.into()),
///     ("package", Nixpkgs::new().get("git").into()),
/// ]);
/// // Describes { enabled = true; package = pkgs.git; }, without evaluating pkgs.git.
/// ```
///
/// `From` accepts supported literals, expressions, handles, `Option<T>` and
/// `BTreeMap<String, NixValue>`. `None` becomes Nix `null`. Floating literals
/// must be finite normal numbers or zero; validation rejects NaN, infinity and
/// subnormal numbers. `!value` constructs Nix boolean negation.
#[derive(Clone, Debug)]
pub struct NixValue(
    /// The literal, lookup or operation to emit in Nix, with its Rust source location.
    pub(crate) Node,
);

impl sealed::Sealed for NixValue {}

impl ConfigValue for NixValue {}

impl IntoNode for NixValue {
    fn into_node(self, _: Origin) -> Node {
        self.0
    }
}

impl NixValue {
    pub(crate) fn from_node(node: Node) -> Self {
        Self(node)
    }

    /// Refer to a function or value supplied by the Nix language, such as `replaceStrings`.
    /// This represents `builtins.<name>`, separately from nixpkgs' caller-supplied `lib`.
    /// Rust does not execute the builtin; use [`Self::apply`] to describe its arguments.
    /// The name is one literal attribute, checked for existence later by Nix.
    #[track_caller]
    pub fn builtin(name: &str) -> Self {
        Self(
            Reference {
                source: Source::Builtins,
                path: Some(AttrPath::segments([name])),
                origin: Origin::caller(format!("Nix builtin lookup {name}")),
            }
            .node(),
        )
    }

    /// Combine two Nix attribute sets, with right-hand fields replacing left-hand fields.
    /// This represents `left // right`; replacement is shallow and selected values stay lazy.
    /// Nix checks both operands when needed. This is distinct from NixOS definition merging.
    #[track_caller]
    pub fn merge_attrs(self, right: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("Nix attribute-set union"),
            kind: ValueKind::AttrMerge(Box::new(self.0), Box::new(right.into().0)),
        })
    }

    /// Require a Nix condition before returning a value: `assert condition; value`.
    /// Rust constructs the expression without checking the condition. Nix evaluates it
    /// when the result is demanded, and a false condition leaves `value` unevaluated.
    /// This uses Nix's assertion syntax, independent of `lib.throwIfNot`, and does not
    /// contribute to the NixOS module assertion list.
    #[track_caller]
    pub fn assert(condition: impl Into<Self>, value: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("Nix expression assertion"),
            kind: ValueKind::Assert(Box::new(condition.into().0), Box::new(value.into().0)),
        })
    }

    /// Describe a Nix function that takes one argument and computes a result.
    /// Use this when an existing Nix helper expects a callback, such as a predicate
    /// or a `stdenv.mkDerivation` attribute builder.
    ///
    /// Rust executes `build` once with a placeholder [`NixValue`]. The returned
    /// expression becomes the function body; Nix supplies the actual argument later.
    /// Nixpkgs calls the attribute-builder argument `finalAttrs`: its fields reflect
    /// later derivation overrides, rather than a Rust snapshot of the build recipe.
    /// Outer Nix parameters remain accessible inside nested functions. A placeholder
    /// used outside its function is rejected during Rusix validation.
    #[track_caller]
    pub fn function(build: impl FnOnce(Self) -> Self) -> Self {
        let origin = Origin::caller("opaque Nix callback");
        let (binding, parameter) = Self::parameter(&origin);
        Self(Node {
            origin,
            kind: ValueKind::Function {
                binding,
                body: Box::new(build(parameter).0),
            },
        })
    }

    fn parameter(origin: &Origin) -> (u64, Self) {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);

        let binding = NEXT.fetch_add(1, Ordering::Relaxed);
        let parameter = Self(Node {
            origin: origin.clone(),
            kind: ValueKind::Parameter(binding),
        });

        (binding, parameter)
    }

    /// Describe a Nix function with named arguments and optional defaults.
    /// For example, the interface `["name", "label"]` can represent
    /// `{ name, label ? name }: label`. This is the usual shape of a nixpkgs package
    /// function, where named arguments supply dependencies and feature choices.
    ///
    /// Rust executes `build` once with placeholders for the arguments. Return the
    /// named defaults and body expression; names without defaults are required.
    /// Nix evaluates defaults only when needed, and defaults can refer to other
    /// arguments. Rust never reads the caller’s argument values.
    ///
    /// nixpkgs’ `callPackage` calls a package function and automatically supplies
    /// named dependencies from its package set. The generated function supports
    /// that mechanism and Nix’s `builtins.functionArgs` inspection of argument names
    /// and whether defaults exist. Nix checks missing or unexpected arguments.
    ///
    /// Names must be valid Nix parameter identifiers and cannot start with
    /// `__rusix_`. Known selections use parameter names directly. Whole-record uses
    /// or shadowed outer names retain a lazy record representation; placeholders
    /// that escape their function are rejected.
    ///
    /// ```
    /// use rusix::interop::raw::NixValue;
    /// let factory = NixValue::function_attrs(["name", "label"], |args| {
    ///     let name = args.clone().select("name");
    ///     (vec![("label", name)], args.select("label"))
    /// });
    /// let label = factory.call(NixValue::record([("name", "git".into())]));
    /// // Nix will return "git" using the label default; Rust only describes the call.
    /// ```
    #[track_caller]
    pub fn function_attrs<K: Into<String>>(
        arguments: impl IntoIterator<Item = impl Into<String>>,
        build: impl FnOnce(Self) -> (Vec<(K, Self)>, Self),
    ) -> Self {
        let origin = Origin::caller("Nix argument-set function");
        let (binding, parameter) = Self::parameter(&origin);
        let (defaults, body) = build(parameter);
        Self(Node {
            origin,
            kind: ValueKind::FunctionAttrs {
                binding,
                arguments: arguments.into_iter().map(Into::into).collect(),
                open: false,
                defaults: defaults
                    .into_iter()
                    .map(|(name, value)| (name.into(), value.0))
                    .collect(),
                body: Box::new(body.0),
            },
        })
    }

    /// Describe `{ name, optional ? default, ... }: body`, accepting extra arguments.
    /// NixOS supplies arguments such as `pkgs` by inspecting the named parameters,
    /// and also passes module-system arguments that a module need not consume.
    /// This form supports both behaviors while retaining native lazy defaults.
    ///
    /// As with [`Self::function_attrs`], `build` runs once in Rust. Its symbolic
    /// record contains only declared names, with defaults resolved; extra caller
    /// fields are ignored and never forced. This is not Nix's raw `args@{ ... }`
    /// binding, whose record omits defaults and includes arbitrary extra fields.
    #[track_caller]
    pub fn function_attrs_open<K: Into<String>>(
        arguments: impl IntoIterator<Item = impl Into<String>>,
        build: impl FnOnce(Self) -> (Vec<(K, Self)>, Self),
    ) -> Self {
        let mut function = Self::function_attrs(arguments, build);
        if let ValueKind::FunctionAttrs { open, .. } = &mut function.0.kind {
            *open = true;
        }

        function
    }

    /// Choose between two values when Nix evaluates a boolean condition.
    /// This represents `if condition then yes else no`. Both expressions are
    /// constructed in Rust, but Nix evaluates only the chosen branch. Nix checks
    /// the condition’s actual type; the result remains a [`NixValue`].
    #[track_caller]
    pub fn if_else(condition: impl Into<Self>, yes: impl Into<Self>, no: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix choice"),
            kind: ValueKind::If(
                Box::new(condition.into().0),
                Box::new(yes.into().0),
                Box::new(no.into().0),
            ),
        })
    }

    /// Construct boolean AND for Nix to evaluate later.
    /// Both operands must evaluate to booleans, but Nix evaluates the right operand
    /// only when the left is true. This shares [`crate::Expr::and`] semantics and
    /// uses Nix conditionals independently of nixpkgs `lib`.
    #[track_caller]
    pub fn and(self, other: impl Into<Self>) -> Self {
        self.into_expr::<bool>()
            .and(other.into().into_expr::<bool>())
            .into()
    }

    /// Construct boolean OR, sharing [`crate::Expr::or`] semantics.
    /// Nix checks demanded operands as booleans and evaluates the right operand
    /// only when the left is false. No caller-supplied library is consulted.
    #[track_caller]
    pub fn or(self, other: impl Into<Self>) -> Self {
        self.into_expr::<bool>()
            .or(other.into().into_expr::<bool>())
            .into()
    }

    /// Construct boolean implication, sharing [`crate::Expr::implies`] semantics.
    /// Nix checks demanded operands as booleans. A false left operand returns true
    /// without evaluating the right operand. No caller-supplied library is consulted.
    #[track_caller]
    pub fn implies(self, other: impl Into<Self>) -> Self {
        self.into_expr::<bool>()
            .implies(other.into().into_expr::<bool>())
            .into()
    }

    /// Compare two values using Nix’s `==` operation when they are evaluated.
    /// The result represents a Nix boolean; no comparison happens in Rust.
    #[track_caller]
    pub fn equals(self, other: impl Into<Self>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix equality"),
            kind: ValueKind::Equal(Box::new(self.0), Box::new(other.into().0)),
        })
    }

    /// Convert a value to text later using Nix’s `builtins.toString`.
    /// `builtins` contains functions supplied by the Nix language itself, separately
    /// from nixpkgs’ `lib`. Unsupported conversions fail during Nix evaluation.
    ///
    /// Nix strings can carry dependencies on package outputs as well as text bytes.
    /// This method preserves those dependencies, known as *string context*, instead
    /// of flattening a package path into an unrelated Rust string.
    #[track_caller]
    pub fn to_text(self) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix to text"),
            kind: ValueKind::ToText(Box::new(self.0)),
        })
    }

    /// Describe a supported Rust literal or preserve an existing Nix expression.
    /// Rust literals become Nix literals; expression and reference inputs keep their
    /// recorded source locations. Float values must be finite normal numbers or zero;
    /// Rusix validation rejects NaN, infinity and subnormals.
    #[track_caller]
    pub fn literal(value: impl ConfigValue) -> Self {
        Self(value.into_node(Origin::caller("opaque call argument")))
    }

    /// Construct explicit Nix null, useful for nullable settings and function arguments.
    #[track_caller]
    pub fn null() -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix null"),
            kind: ValueKind::Null,
        })
    }

    /// Build a Nix attribute set: a collection of named values, like a Rust record.
    /// It stays one value when passed to a function or configuration field; its keys
    /// are not expanded into configuration paths.
    ///
    /// Keys are literal names, so `"a.b"` is one field rather than two nested fields.
    /// A `BTreeMap` can supply dynamic keys. Iterator order determines generated field
    /// order, while Nix owns attribute-set semantics. Duplicate or NUL-containing
    /// keys are rejected during validation; empty names are allowed.
    #[track_caller]
    pub fn record(fields: impl IntoIterator<Item = (impl Into<String>, Self)>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix record"),
            kind: ValueKind::OpaqueRecord(
                fields
                    .into_iter()
                    .map(|(key, value)| (key.into(), value.0))
                    .collect(),
            ),
        })
    }

    /// Describe an ordered Nix list without evaluating its elements in Rust.
    /// Nix evaluates an element only when it is needed; elements can mix literals,
    /// packages and other expressions.
    #[track_caller]
    pub fn list(items: impl IntoIterator<Item = Self>) -> Self {
        Self(Node {
            origin: Origin::caller("opaque Nix list"),
            kind: ValueKind::List(items.into_iter().map(|value| value.0).collect()),
        })
    }

    /// Join several deferred Nix lists, preserving their element order.
    /// Uses `builtins.concatLists`, independently of nixpkgs `lib`. No inputs
    /// produce an empty list; individual element values remain lazy. Nix checks
    /// that each input evaluates to a list when the result is needed.
    /// Use [`super::NixLibrary::concat_lists`] to call the supplied library's function.
    #[track_caller]
    pub fn concat_lists(lists: impl IntoIterator<Item = Self>) -> Self {
        Self::builtin("concatLists").call(Self::list(lists))
    }

    /// Pass one argument to a Nix function when Nix evaluates the expression.
    /// Many Nix functions take arguments one at a time: chain calls to represent
    /// `function first second`, or use [`Self::apply`]. Rust does not execute the call.
    #[track_caller]
    pub fn call(self, argument: impl ConfigValue) -> Self {
        let origin = Origin::caller("opaque Nix function call");
        Self(Node {
            origin: origin.clone(),
            kind: ValueKind::Apply(Box::new(self.0), Box::new(argument.into_node(origin))),
        })
    }

    /// Pass arguments to a Nix function one at a time, in the supplied order.
    /// `function.apply([a, b])` represents `function a b`. This is repeated Nix
    /// function application, not a Rust call or a single list argument. Nix checks
    /// the arguments and evaluates them as required by the function.
    #[track_caller]
    pub fn apply(mut self, arguments: impl IntoIterator<Item = Self>) -> Self {
        // A direct call keeps track_caller; a function-pointer fold loses it.
        for argument in arguments {
            self = self.call(argument);
        }
        self
    }

    /// Join a Nix list of strings using `separator` between each pair.
    /// The list itself can be computed by Nix later. Elements must evaluate to
    /// strings; use [`Self::to_text`] for conversion. Package dependencies attached
    /// to each string are retained.
    #[track_caller]
    pub fn join_text(separator: &str, parts: Self) -> Self {
        Self::builtin("concatStringsSep").apply([separator.into(), parts])
    }

    /// Join the supplied expressions as text with no separator.
    /// Rust chooses the sequence of parts; Nix evaluates their string values later.
    /// For a mostly literal template with named holes, prefer [`crate::nix_text!`].
    #[track_caller]
    pub fn concat_text(parts: impl IntoIterator<Item = Self>) -> Self {
        Self::join_text("", Self::list(parts))
    }

    /// Replace text using ordered `(from, to)` pairs through `builtins.replaceStrings`.
    /// Inputs stay deferred and string dependencies are retained. This follows Nix's
    /// replacement rules, rather than applying a sequence of Rust string replacements.
    /// Use [`super::NixLibrary::replace_text`] to call the supplied library instead.
    #[track_caller]
    pub fn replace_text(
        self,
        replacements: impl IntoIterator<Item = (impl Into<Self>, impl Into<Self>)>,
    ) -> Self {
        let [from, to] = replacement_lists(replacements);
        Self::builtin("replaceStrings").apply([from, to, self])
    }

    /// Test for one literal attribute name supplied as text, possibly computed by Nix.
    /// Dots and punctuation remain part of the name, rather than describing a path.
    /// This uses `builtins.hasAttr` and does not evaluate the attribute's value.
    #[track_caller]
    pub fn has_attr(self, name: impl Into<Self>) -> Self {
        Self::builtin("hasAttr").apply([name.into(), self])
    }

    /// Select one literal attribute name, or evaluate `fallback` if it is absent.
    /// The name may be computed by Nix; dots and punctuation remain literal data.
    /// An existing null value is returned as null. The unchosen value stays lazy.
    /// Uses `builtins.hasAttr` and `builtins.getAttr`, independently of nixpkgs `lib`.
    #[track_caller]
    pub fn attr_or(self, name: impl Into<Self>, fallback: impl Into<Self>) -> Self {
        let name = name.into();
        let fallback = fallback.into();
        let origin = Origin::caller("opaque Nix attribute fallback");
        let (attrs_binding, attrs) = Self::parameter(&origin);
        let (name_binding, attribute_name) = Self::parameter(&origin);
        // Build outside Rust callbacks so track_caller reaches the public call.
        // Nix callbacks share both inputs while keeping the fallback lazy.
        let selected = Self::if_else(
            attrs.clone().has_attr(attribute_name.clone()),
            Self::builtin("getAttr").apply([attribute_name, attrs]),
            fallback,
        );
        let lookup = Self(Node {
            origin: origin.clone(),
            kind: ValueKind::Function {
                binding: name_binding,
                body: Box::new(selected.0),
            },
        });
        Self(Node {
            origin,
            kind: ValueKind::Function {
                binding: attrs_binding,
                body: Box::new(lookup.call(name).0),
            },
        })
        .call(self)
    }

    /// Call this package's supplied `override` function with argument changes.
    /// `overrides` may be a record or a Nix callback accepted by the package.
    /// nixpkgs owns default handling and dependency splicing; Rust only describes
    /// the call and retains the returned package's ordinary override interface.
    #[track_caller]
    pub fn override_args(self, overrides: impl Into<Self>) -> Self {
        self.select("override").call(overrides.into())
    }

    /// Call this package's supplied `overrideAttrs` function with an update.
    /// Pass a record or deferred callback accepted by the package's implementation,
    /// including nested callbacks for the recursive final/previous-attributes form.
    /// nixpkgs owns attribute and self-reference semantics; Rust does not rebuild
    /// or inspect the derivation.
    #[track_caller]
    pub fn override_attrs(self, update: impl Into<Self>) -> Self {
        self.select("overrideAttrs").call(update.into())
    }

    /// Look up named fields in a Nix attribute set later.
    /// `value.select("foo.bar")` represents `value.foo.bar`; it does not read
    /// fields into Rust. Missing fields can be reported at this Rust call.
    /// Use [`Self::select_segments`] when a field name itself contains a dot.
    #[track_caller]
    pub fn select(self, path: &str) -> Self {
        self.select_segments(path.split('.'))
    }

    /// Look up a sequence of literal field names in Nix attribute sets.
    /// For example, `["foo.bar", "baz"]` selects `value."foo.bar".baz`, preserving
    /// the dot within the first name. The lookup records this Rust call location.
    /// Inside named-argument functions, known arguments use their Nix parameter
    /// bindings directly.
    #[track_caller]
    pub fn select_segments(self, parts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let path = AttrPath::segments(parts);
        Self(Node {
            origin: Origin::caller(format!("opaque Nix selection {}", path.parts().join("."))),
            kind: ValueKind::Select(Box::new(self.0), path),
        })
    }

    /// Attach an expected scalar type so Rust can offer typed expression operations.
    /// For example, an `Expr<i64>` permits [`crate::Expr::divide`]. This does not
    /// read or check the Nix value. Supported expectations are `bool`, `String` and
    /// `i64`; Nix still checks the actual type when the expression is evaluated.
    pub fn into_expr<T>(self) -> crate::Expr<T>
    where
        crate::Expr<T>: ConfigValue,
    {
        crate::Expr::new(self.0.kind, self.0.origin)
    }
}

// Both builtin and library replacements use aligned lists without reading any text.
#[track_caller]
pub(super) fn replacement_lists(
    replacements: impl IntoIterator<Item = (impl Into<NixValue>, impl Into<NixValue>)>,
) -> [NixValue; 2] {
    let mut from = Vec::new();
    let mut to = Vec::new();
    // Conversions inside a Rust iterator callback would lose track_caller.
    for (pattern, replacement) in replacements {
        from.push(pattern.into());
        to.push(replacement.into());
    }
    [NixValue::list(from), NixValue::list(to)]
}

/// Negates a deferred Nix boolean, preserving its origin and checking its type in Nix.
impl std::ops::Not for NixValue {
    type Output = Self;

    #[track_caller]
    fn not(self) -> Self {
        Self::if_else(self, false, true)
    }
}

macro_rules! literal_conversion {
    ($($ty:ty),* $(,)?) => { $(
        impl From<$ty> for NixValue {
            #[track_caller]
            fn from(value: $ty) -> Self { Self::literal(value) }
        }
    )* };
}

literal_conversion!(
    bool,
    i32,
    i64,
    u8,
    u16,
    u32,
    i8,
    i16,
    f64,
    String,
    &str,
    crate::Expr<bool>,
    crate::Expr<i64>,
    crate::Expr<String>
);

macro_rules! reference_conversion {
    ($($ty:ty),* $(,)?) => { $(
        impl From<$ty> for NixValue {
            fn from(value: $ty) -> Self { value.as_value() }
        }
    )* };
}

reference_conversion!(PackageRef, ModuleRef, NixFunction, OverlayRef);

impl<T: Into<NixValue>> From<Option<T>> for NixValue {
    #[track_caller]
    fn from(value: Option<T>) -> Self {
        match value {
            Some(value) => value.into(),
            None => Self::null(),
        }
    }
}

impl From<std::collections::BTreeMap<String, NixValue>> for NixValue {
    #[track_caller]
    fn from(value: std::collections::BTreeMap<String, NixValue>) -> Self {
        Self::record(value)
    }
}

/// Explicit erasure of a reference handle for dynamic Nix interop.
/// Use typed packages, imports and external callable contracts where possible.
pub trait AsNixValue {
    /// Retain the deferred lookup and its Rust location while erasing its category.
    fn as_value(&self) -> NixValue;
}

/// Calls and unchecked signature expectations for an external Nix function.
/// Prefer a named typed helper or an already typed [`super::NixCallable`].
/// Import this trait explicitly when adapting an otherwise unknown function.
pub trait NixFunctionExt: AsNixValue {
    /// Declare the expected result interface of an external callable reference.
    /// This does not inspect its Nix implementation or evaluate its result.
    fn returning<R: NixExpression>(&self) -> NixCallable<R> {
        NixCallable::from_expression(self.as_value())
    }

    /// State both parameter and result expectations for an external callable.
    /// This does not inspect or eagerly validate the external Nix function.
    #[track_caller]
    fn signature<A: NixExpression, R: NixExpression>(&self) -> NixCallable<R, A> {
        NixCallable::from_expression(self.as_value())
    }

    /// Describe a call to this Nix function with one argument.
    /// Nix executes the call later and checks the argument. If the result is another
    /// function, continue with [`NixValue::call`] or use [`Self::apply`].
    #[track_caller]
    fn call(&self, argument: impl ConfigValue) -> NixValue {
        self.as_value().call(argument)
    }

    /// Describe successive calls to this Nix function, one for each argument.
    /// For example, `write_text.apply([name, text])` represents `writeText name text`.
    /// Arguments can mix Rust literals, records, package references and expressions.
    /// Nix executes the calls later; Rusix does not infer the result’s category.
    ///
    /// ```
    /// use rusix::interop::{Nixpkgs, raw::NixFunctionExt};
    /// let file = Nixpkgs::new().pkgs_function("writeText")
    ///     .apply(["example.conf".into(), "workers=4\n".into()]);
    /// // Nix will describe a generated file; constructing this call does not build it.
    /// ```
    #[track_caller]
    fn apply(&self, arguments: impl IntoIterator<Item = NixValue>) -> NixValue {
        self.as_value().apply(arguments)
    }
}

impl NixFunctionExt for NixFunction {}

/// Dynamic package-set/library lookup for adapters without typed helpers.
/// This trait is intentionally excluded from [`crate::prelude`].
pub trait NixpkgsExt {
    /// Refer to any named value in the package set, without claiming it is a package.
    /// Use this for metadata, records or other objects. Dots select nested fields;
    /// Rust constructs the lookup and Nix evaluates it later.
    #[track_caller]
    fn value(&self, path: &str) -> NixValue;

    /// Pass the entire package set as a Nix value, for helpers that expect `pkgs`.
    /// Rust does not inspect the packages or the final NixOS configuration.
    #[track_caller]
    fn as_value(&self) -> NixValue;

    /// Refer to a named value in nixpkgs’ `lib` utility library.
    /// This includes NixOS type objects such as `types.port`; they describe how
    /// NixOS validates and merges option values. Rust does not evaluate the lookup.
    #[track_caller]
    fn lib_value(&self, path: &str) -> NixValue;
}

impl NixpkgsExt for Nixpkgs {
    /// Refer to any named value in the package set, without claiming it is a package.
    /// Use this for metadata, records or other objects. Dots select nested fields;
    /// Rust constructs the lookup and Nix evaluates it later.
    #[track_caller]
    fn value(&self, path: &str) -> NixValue {
        NixValue(
            Reference {
                source: self.package_source(),
                path: Some(AttrPath::dotted(path)),
                origin: Origin::caller(format!("nixpkgs value lookup {path}")),
            }
            .node(),
        )
    }

    /// Pass the entire package set as a Nix value, for helpers that expect `pkgs`.
    /// Rust does not inspect the packages or the final NixOS configuration.
    #[track_caller]
    fn as_value(&self) -> NixValue {
        NixValue(
            Reference {
                source: self.package_source(),
                path: None,
                origin: Origin::caller("nixpkgs package set"),
            }
            .node(),
        )
    }

    /// Refer to a named value in nixpkgs’ `lib` utility library.
    /// This includes NixOS type objects such as `types.port`; they describe how
    /// NixOS validates and merges option values. Rust does not evaluate the lookup.
    #[track_caller]
    fn lib_value(&self, path: &str) -> NixValue {
        NixValue(
            Reference {
                source: Source::Library,
                path: Some(AttrPath::dotted(path)),
                origin: Origin::caller(format!("nixpkgs lib value lookup {path}")),
            }
            .node(),
        )
    }
}

/// Dynamic lookup in an external Nix input; package/module lookup stays typed.
pub trait InputRefExt {
    /// Refer to any value returned by this file, without assuming its category.
    /// Dots select nested fields. The resulting expression is evaluated by Nix later.
    #[track_caller]
    fn value(&self, path: &str) -> NixValue;
}

impl InputRefExt for super::InputRef {
    /// Refer to any value returned by this file, without assuming its category.
    /// Dots select nested fields. The resulting expression is evaluated by Nix later.
    #[track_caller]
    fn value(&self, path: &str) -> NixValue {
        NixValue(self.lookup(path, "value").node())
    }
}
