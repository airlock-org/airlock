//! Typed handles retain authoring contracts while sharing the existing deferred IR.
use super::{
    NixLibrary, OverlayRef, PackageRef,
    raw::{AsNixValue, NixRepresentation, NixValue},
};
use crate::{
    ConfigValue, Expr, IntoRusixValue, RusixValue, ValidationError,
    ir::{IntoNode, Node, Origin},
    sealed,
};
use std::marker::PhantomData;

/// A value described in Rust that Nix will compute when it is needed.
///
/// Nix expressions can share a value, choose between alternatives, or reject a
/// value when a condition fails. Use this trait to describe those operations
/// while keeping the Rust type, such as [`Package`] or [`Expr<String>`].
/// Constructing an expression does not evaluate Nix or validate an external value.
/// Custom symbolic interfaces implement
/// [`NixRepresentation`]; raw representation
/// access requires importing that separate trait explicitly.
pub trait NixExpression: NixRepresentation + Clone + IntoRusixValue {
    /// Let the callback reuse this expression through one shared Nix parameter.
    ///
    /// Rust runs `build` once, immediately, with a symbolic parameter. Nix supplies
    /// the value later and evaluates it only if the callback's expression needs it.
    /// Reusing the parameter avoids repeating the original expression in Nix.
    /// Use this to guarantee sharing across different authored fields or closures;
    /// automatic compiler sharing is conservative about lexical dependencies and diagnostic paths.
    ///
    /// ```
    /// use rusix::{Expr, interop::NixExpression};
    ///
    /// let text = Expr::<String>::from("hello");
    /// let twice = text.bind(|shared| Expr::concat([shared.clone(), shared]));
    /// // Like Nix: (text: text + text) "hello".
    /// ```
    #[track_caller]
    fn bind<R: NixExpression>(self, build: impl FnOnce(Self) -> R) -> R {
        R::from_expression(
            NixValue::function(|parameter| build(Self::from_expression(parameter)).as_expression())
                .call(self.as_expression()),
        )
    }

    /// Convert a Rust record and give `build` a symbolic view of it to reuse.
    /// The callback runs once in Rust after conversion succeeds; Nix evaluates the
    /// resulting expression later. `Self` describes the expected Nix interface.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] if Rust structural conversion fails, such as
    /// flattening a field that is not a record. Success does not check the eventual
    /// Nix type or field names; those errors occur during Nix evaluation.
    #[track_caller]
    fn try_bind_record<R: NixExpression>(
        record: impl IntoRusixValue,
        build: impl FnOnce(Self) -> R,
    ) -> Result<R, ValidationError> {
        Ok(Self::from_expression(record.try_into_nix_value()?).bind(build))
    }

    /// Choose between expressions with the same Rust interface, lazily in Nix.
    /// Both arguments are constructed in Rust. Nix evaluates the condition and
    /// only the selected alternative when the result is needed.
    ///
    /// ```
    /// use rusix::{Expr, interop::NixExpression};
    ///
    /// let label = Expr::<String>::choose(true, "enabled".into(), "disabled".into());
    /// // Like Nix: if true then "enabled" else "disabled".
    /// ```
    #[track_caller]
    fn choose(condition: impl Into<Expr<bool>>, yes: Self, no: Self) -> Self {
        Self::from_expression(NixValue::if_else(
            condition.into(),
            yes.as_expression(),
            no.as_expression(),
        ))
    }

    /// Return this value if `condition` holds; otherwise fail with `message` in Nix.
    /// Uses the supplied library's `throwIfNot` helper. Rust constructs the guard;
    /// Nix checks it when the result is needed and leaves the value unforced on failure.
    #[track_caller]
    fn require(
        self,
        lib: &NixLibrary,
        condition: impl Into<Expr<bool>>,
        message: impl Into<Expr<String>>,
    ) -> Self {
        lib.throw_if_not(condition, message, self)
    }

    /// Return this value if `condition` holds; otherwise raise a Nix assertion error.
    /// Rust constructs the guard without checking it. Nix checks it before evaluating
    /// the guarded value, when the result is needed. Use [`Self::require`] to supply
    /// an explanatory failure message instead.
    ///
    /// ```
    /// use rusix::{Expr, interop::NixExpression};
    ///
    /// let answer = Expr::<i64>::from(42).asserted(true);
    /// // Like Nix: assert true; 42.
    /// ```
    #[track_caller]
    fn asserted(self, condition: impl Into<Expr<bool>>) -> Self {
        Self::from_expression(NixValue::assert(condition.into(), self.as_expression()))
    }
}

impl<T: NixRepresentation + Clone + IntoRusixValue> NixExpression for T {}

impl NixRepresentation for NixValue {
    fn from_expression(value: NixValue) -> Self {
        value
    }

    fn as_expression(&self) -> NixValue {
        self.clone()
    }
}

macro_rules! scalar {
    ($ty:ty) => {
        impl NixRepresentation for Expr<$ty> {
            fn from_expression(value: NixValue) -> Self {
                value.into_expr()
            }

            fn as_expression(&self) -> NixValue {
                self.clone().into()
            }
        }

        impl From<$ty> for Expr<$ty> {
            #[track_caller]
            fn from(value: $ty) -> Self {
                NixValue::from(value).into_expr()
            }
        }
    };
}

scalar!(bool);

scalar!(String);

scalar!(i64);

impl From<&str> for Expr<String> {
    #[track_caller]
    fn from(value: &str) -> Self {
        NixValue::from(value).into_expr()
    }
}

impl Expr<String> {
    /// Join text expressions in order, preserving any dependencies on package outputs.
    /// Nix strings can carry those dependencies as their *string context*, so a
    /// pathname inserted into a build recipe still refers to its producing package.
    /// Rust consumes the iterator now; Nix computes the joined text later.
    #[track_caller]
    pub fn concat(parts: impl IntoIterator<Item = Self>) -> Self {
        NixValue::concat_text(parts.into_iter().map(|part| part.as_expression())).into_expr()
    }
}

macro_rules! expression_handle {
    ($name:ident $(<$($parameter:ident),+>)?) => {
        impl$(<$($parameter: NixExpression),+>)? sealed::Sealed for $name$(<$($parameter),+>)? {}

        impl$(<$($parameter: NixExpression),+>)? ConfigValue for $name$(<$($parameter),+>)? {}

        impl$(<$($parameter: NixExpression),+>)? IntoNode for $name$(<$($parameter),+>)? {
            fn into_node(self, origin: Origin) -> Node {
                self.value.into_node(origin)
            }
        }

        impl$(<$($parameter: NixExpression),+>)? IntoRusixValue for $name$(<$($parameter),+>)? {
            #[track_caller]
            fn into_value(self) -> RusixValue {
                RusixValue::leaf(self)
            }
        }

        impl$(<$($parameter: NixExpression),+>)? NixRepresentation for $name$(<$($parameter),+>)? {
            fn from_expression(value: NixValue) -> Self {
                Self { value, $(_type: PhantomData::<($($parameter,)+)>,)? }
            }

            fn as_expression(&self) -> NixValue {
                self.value.clone()
            }
        }

        impl$(<$($parameter: NixExpression),+>)? From<$name$(<$($parameter),+>)?> for NixValue {
            fn from(value: $name$(<$($parameter),+>)?) -> Self {
                value.value
            }
        }
    };
}

/// Convert a literal or symbolic value to its natural deferred interface.
/// Unlike IntoRusixValue, this conversion retains its expression category.
pub trait IntoNixExpression {
    /// Symbolic interface associated with this value.
    type Expression: NixExpression;

    /// Construct the expression without evaluating Nix.
    #[track_caller]
    fn into_expression(self) -> Self::Expression;
}

impl<T: NixExpression> IntoNixExpression for T {
    type Expression = T;

    fn into_expression(self) -> T {
        self
    }
}

macro_rules! expression_literal {
    ($ty:ty, $expression:ty) => {
        impl IntoNixExpression for $ty {
            type Expression = $expression;

            #[track_caller]
            fn into_expression(self) -> Self::Expression {
                self.into()
            }
        }
    };
}

expression_literal!(bool, Expr<bool>);

expression_literal!(i64, Expr<i64>);

expression_literal!(String, Expr<String>);

expression_literal!(&str, Expr<String>);

expression_literal!(PackageRef, Package);

/// A value whose original Nix function can be called again with different arguments.
/// nixpkgs exposes this as `.override`, commonly for replacing dependencies or
/// feature options. This capability applies to packages and overridable callables.
/// External values are checked by Nix only when the override is demanded.
pub trait NixOverridable: NixExpression {
    /// Replace arguments to the original Nix function through its `.override` method.
    /// Unspecified arguments keep their previous values. Rust describes the override;
    /// Nix calls the function again when its result is needed.
    #[track_caller]
    fn override_arguments(&self, arguments: impl ConfigValue) -> Self {
        Self::from_expression(
            self.as_expression()
                .override_args(NixValue::literal(arguments)),
        )
    }
}

/// A deferred software build recipe and its named outputs.
///
/// In Nix, a build recipe (a *derivation*) describes inputs, build commands and
/// output locations. Use this handle to select outputs, inspect package fields,
/// or modify a recipe without building the software.
/// [`PackageRef`] identifies an existing package to look up; `Package` also
/// represents the result of a builder, function call, or override.
/// Rust construction does not check that an external value is a derivation or
/// that the recipe will build successfully. Nix checks operations when evaluated.
#[derive(Clone, Debug)]
pub struct Package {
    /// Expression for the software build recipe and its outputs; selecting fields stays deferred.
    value: NixValue,
}

expression_handle!(Package);

impl From<PackageRef> for Package {
    fn from(reference: PackageRef) -> Self {
        Self::from_expression(reference.as_value())
    }
}

impl NixOverridable for Package {}

impl Package {
    /// Describe this package's fields through a declared Rust view.
    /// Use a root or value view generated by [`crate::args`] to read metadata
    /// through accessors such as `package.drv_path()`. The view refers to the
    /// complete package, retaining its fields and outputs without evaluating it.
    /// Nix checks the selected fields when their values are needed.
    #[track_caller]
    pub fn view<V: NixExpression>(&self) -> V {
        V::from_expression(self.value.clone())
    }

    /// Select a named build output, such as `out`, `dev`, or `bin`.
    /// Packages can split files across outputs: `dev` commonly holds headers and
    /// `bin` executables, while `out` commonly names the default output. These names
    /// are package conventions; not every package provides them.
    /// `name` is one literal attribute name. Nix reports a missing output when the
    /// selection is evaluated; use [`NixLibrary::get_dev`] for its fallback behavior.
    #[track_caller]
    pub fn output(&self, name: &str) -> Self {
        Self::from_expression(self.value.clone().select_segments([name]))
    }

    /// Read a package field or nested path, such as `version` or `meta.description`.
    /// Dots in `name` separate path components, unlike [`NixAttrs::get`], which
    /// treats dots as part of a literal name. `T` is the expected expression type,
    /// not a Rust-time check; Nix reports missing fields or incompatible operations
    /// when evaluated.
    #[track_caller]
    pub fn field<T: NixExpression>(&self, name: &str) -> T {
        T::from_expression(self.value.clone().select(name))
    }

    /// Call the original package function again with replacement inputs via `.override`.
    /// Use this for dependency or feature arguments; unspecified inputs stay unchanged.
    /// To change the build recipe's fields instead, use [`Self::override_attrs`].
    /// Rust constructs the call; Nix performs the override when needed.
    ///
    /// ```
    /// use rusix::interop::{NixAttrs, Nixpkgs, Package, raw::NixValue};
    ///
    /// let curl: Package = Nixpkgs::new().get("curl").into();
    /// let without_http2 = curl.override_arguments(NixAttrs::new([
    ///     ("http2Support", NixValue::from(false)),
    /// ]));
    /// // Like Nix: pkgs.curl.override { http2Support = false; }.
    /// ```
    #[track_caller]
    pub fn override_arguments(&self, arguments: impl ConfigValue) -> Self {
        NixOverridable::override_arguments(self, arguments)
    }

    /// Replace package-function inputs supplied as a structured Rust value.
    /// Like [`Self::override_arguments`], this describes a later Nix `.override` call.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] for invalid Rust structural conversion,
    /// such as flattening a non-record field. Success does not check the Nix
    /// function's accepted arguments or evaluate the resulting package.
    #[track_caller]
    pub fn try_override_arguments(
        &self,
        arguments: impl IntoRusixValue,
    ) -> Result<Self, ValidationError> {
        Ok(self.override_arguments(arguments.try_into_nix_value()?))
    }

    /// Change build-recipe fields through the package's `.overrideAttrs` method.
    /// `update` receives the previous builder attributes and returns replacements;
    /// unspecified attributes stay unchanged. Rust runs `update` once immediately
    /// with a symbolic view; Nix applies the generated function when needed.
    ///
    /// ```
    /// use rusix::interop::{NixAttrs, Nixpkgs, Package, raw::NixValue};
    ///
    /// let curl: Package = Nixpkgs::new().get("curl").into();
    /// let renamed = curl.override_attrs(|previous| NixAttrs::new([
    ///     ("pname", NixValue::from("my-curl")),
    ///     ("version", previous.get("version")),
    /// ]));
    /// // Like Nix: pkgs.curl.overrideAttrs (old: { pname = "my-curl"; inherit (old) version; }).
    /// ```
    #[track_caller]
    pub fn override_attrs(&self, update: impl FnOnce(NixAttrs) -> NixAttrs) -> Self {
        Self::from_expression(
            self.value
                .clone()
                .override_attrs(NixCallable::from_function(update)),
        )
    }

    /// Change build-recipe fields using a declared input view and a Rust result struct.
    /// Runs `update` once immediately with symbolic previous attributes of type `A`.
    /// Converts its result into replacement attributes; Nix applies the generated
    /// `.overrideAttrs` call when needed. Unspecified attributes stay unchanged.
    /// A root generated by [`crate::args`] can describe the fields `update` reads.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] if Rust structural conversion fails, such as
    /// flattening a non-record field. Success does not evaluate the override or
    /// check external fields, Nix types, or whether the recipe will build.
    #[track_caller]
    pub fn try_override_attrs<A: NixExpression, V: IntoRusixValue>(
        &self,
        update: impl FnOnce(A) -> V,
    ) -> Result<Self, ValidationError> {
        let update = NixCallable::<NixAttrs, A>::try_from_function(update)?;
        Ok(Self::from_expression(
            self.value.clone().override_attrs(update),
        ))
    }

    /// Add or replace fields on the returned package without changing its build recipe.
    /// This is a shallow Nix attribute-set union: values on the right win, including
    /// whole nested sets. It retains existing override methods; these field changes
    /// do not automatically propagate to other outputs or later overrides.
    /// Use this to attach data such as labels or annotations to a package.
    ///
    /// ```
    /// use rusix::interop::{NixAttrs, Nixpkgs, Package, raw::NixValue};
    ///
    /// let curl: Package = Nixpkgs::new().get("curl").into();
    /// let annotated = curl.extend(NixAttrs::new([("label", NixValue::from("local"))]));
    /// // Like Nix: pkgs.curl // { label = "local"; }.
    /// ```
    #[track_caller]
    pub fn extend(&self, attributes: NixAttrs) -> Self {
        Self::from_expression(self.value.clone().merge_attrs(attributes))
    }
}

/// A Nix function that adds or replaces fields in a nixpkgs package set.
///
/// An *overlay* receives two sets: `final`, containing the results of all overlays,
/// and `prev`, containing packages before this overlay. It returns named additions
/// or replacements. Use `prev` to modify an existing package without referring to
/// its own replacement; use `final` to depend on another package's final version.
/// The returned fields can include packages, helpers and nested collections.
///
/// Construct one with [`Self::try_from_function`] for declared package views and
/// ordinary Rust result structs, or [`Self::from_function`] for dynamic named fields.
/// You can also convert an existing [`OverlayRef`].
/// Pass it to [`super::Nixpkgs::with_overlay`] to customize package lookups, or place
/// it in a [`crate::Config`] to generate the overlay function itself. Rust constructs
/// the expression; Nix applies it later without building the described packages.
#[derive(Clone, Debug)]
pub struct Overlay {
    /// Function receiving final and previous package sets and returning additions or replacements.
    value: NixValue,
}

expression_handle!(Overlay);

impl From<OverlayRef> for Overlay {
    fn from(reference: OverlayRef) -> Self {
        Self::from_expression(reference.as_value())
    }
}

impl Overlay {
    /// Describe an overlay using declared field views and a structured Rust result.
    ///
    /// `F` describes the final package set and `P` the preceding set; roots generated
    /// by [`crate::args`] provide accessors for the fields you need. Rust runs `build`
    /// once immediately with symbolic views, then converts its returned fields.
    /// Nix applies the generated `final: prev: { ... }` function when needed.
    /// Fields omitted from the returned record keep their preceding definitions.
    ///
    /// ```
    /// use rusix::{ IntoRusixValue, interop::{Nixpkgs, Overlay, Package}};
    ///
    /// #[rusix::args]
    /// mod packages {
    ///     use rusix::interop::Package;
    ///
    ///     #[rusix(root)]
    ///     struct Packages {
    ///         // Existing curl package selected from this supplied nixpkgs package set.
    ///         curl: Package,
    ///     }
    /// }
    ///
    /// #[derive(IntoRusixValue)]
    /// struct Changes {
    ///     // Package exported as myCurl by the Nix overlay.
    ///     my_curl: Package,
    /// }
    ///
    /// let overlay = Overlay::try_from_function(
    ///     |_final_pkgs: packages::Packages, prev: packages::Packages| {
    ///         Changes { my_curl: prev.curl() }
    ///     },
    /// ).expect("valid overlay fields");
    /// let pkgs = Nixpkgs::new().with_overlay(overlay);
    /// // Like Nix: final: prev: { myCurl = prev.curl; }.
    /// ```
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] if structural conversion of the result fails,
    /// such as flattening a non-record field. Duplicate names are checked during
    /// compilation. Success does not check external package names or prove the
    /// resulting Nix value is an attribute set; Nix checks those expectations later.
    #[track_caller]
    pub fn try_from_function<F: NixExpression, P: NixExpression, V: IntoRusixValue>(
        build: impl FnOnce(F, P) -> V,
    ) -> Result<Self, ValidationError> {
        let mut error = None;
        let function = NixValue::function(|final_pkgs| {
            match NixCallable::<NixAttrs, P>::try_from_function(|prev_pkgs| {
                build(F::from_expression(final_pkgs), prev_pkgs)
            }) {
                Ok(function) => function.as_expression(),
                Err(failure) => {
                    error = Some(failure);
                    NixValue::null()
                }
            }
        });
        match error {
            Some(error) => Err(error),
            None => Ok(Self::from_expression(function)),
        }
    }

    /// Describe `final: prev: { ... }` through typed attribute-set views.
    ///
    /// Rust runs `build` once immediately with symbolic `final` and `prev`
    /// parameters. It returns the fields this overlay adds or replaces, leaving
    /// other fields unchanged. Nix resolves lookups and applies the generated
    /// function when a customized package-set value is needed. No package types
    /// or external field names are checked during Rust construction.
    ///
    /// ```
    /// use rusix::{IntoConfig, interop::{NixAttrs, Nixpkgs, Overlay, Package, PackageRef}};
    ///
    /// #[derive(IntoConfig)]
    /// struct Output {
    ///     // Function describing additions or replacements to the Nix package set.
    ///     overlay: Overlay,
    ///     // Existing package selected from the customized Nix package set.
    ///     package: PackageRef,
    /// }
    ///
    /// let overlay = Overlay::from_function(|_final_pkgs, prev_pkgs| {
    ///     let curl: Package = prev_pkgs.field("curl");
    ///     NixAttrs::new([("myCurl", curl.into())])
    /// });
    /// let pkgs = Nixpkgs::new().with_overlay(overlay.clone());
    /// let output = Output { overlay, package: pkgs.get("myCurl") };
    /// // Like Nix: final: prev: { myCurl = prev.curl; }.
    /// ```
    #[track_caller]
    pub fn from_function(build: impl FnOnce(NixAttrs, NixAttrs) -> NixAttrs) -> Self {
        Self::from_expression(NixValue::function(|final_pkgs| {
            NixValue::function(|prev_pkgs| {
                build(
                    NixAttrs::from_expression(final_pkgs),
                    NixAttrs::from_expression(prev_pkgs),
                )
                .as_expression()
            })
        }))
    }
}

/// A Nix value whose original function arguments can be changed with `.override`.
///
/// nixpkgs attaches this method to results created through its helpers. Calling
/// it supplies replacement arguments; Nix reruns the function when the new result
/// is needed. Use this handle to describe those replacements from Rust.
///
/// Packages already have this contract. Wrap other external expressions explicitly
/// when they are known to be overridable, such as nixpkgs fetcher callables. Plain
/// constructed functions do not implement the override capability automatically.
/// This expectation does not inspect the Nix expression or force its attributes.
#[derive(Clone, Debug)]
pub struct Overridable<T: NixExpression> {
    /// Expression whose Nix result is expected to provide the argument-changing `.override` method.
    value: NixValue,
    /// Rust interface retained after argument overrides; the Nix value is not inspected here.
    _type: PhantomData<(T,)>,
}

expression_handle!(Overridable<T>);

impl<T: NixExpression> NixOverridable for Overridable<T> {}

impl<T: NixExpression> Overridable<T> {
    /// Access the wrapped symbolic interface without discarding its result type.
    pub fn inner(&self) -> T {
        T::from_expression(self.value.clone())
    }

    /// Extend an overridable attribute set using ordinary shallow Nix union.
    /// Callable attribute sets retain their __functor and override methods.
    #[track_caller]
    pub fn extend(&self, attributes: NixAttrs) -> Self {
        Self::from_expression(self.value.clone().merge_attrs(attributes))
    }
}

impl<R: NixExpression, A: NixExpression> Overridable<NixCallable<R, A>> {
    /// Apply the wrapped callable, retaining its declared result interface.
    #[track_caller]
    pub fn call(&self, argument: impl Into<A>) -> R {
        self.inner().call(argument)
    }
}

impl<R: NixExpression> Overridable<NixCallable<R>> {
    /// Describe a call with a structured Rust argument, retaining the result interface.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] if Rust structural conversion fails. Success
    /// does not evaluate the call or validate the function's Nix argument type.
    #[track_caller]
    pub fn try_call(&self, argument: impl IntoRusixValue) -> Result<R, ValidationError> {
        self.inner().try_call(argument)
    }
}

/// A Nix function that takes an argument described by `A` and returns a value described by `R`.
///
/// Use this to construct functions in Rust or call existing Nix functions while
/// keeping a useful Rust interface for their results. The default [`NixValue`]
/// parameter accepts values with different shapes. These types describe expected
/// interfaces; they do not validate the Nix function or run it in Rust.
/// Unlike [`super::NixFunction`], which refers to an existing function, this also
/// represents functions built by callbacks and symbolic function parameters.
#[derive(Clone, Debug)]
pub struct NixCallable<R: NixExpression = NixValue, A: NixExpression = NixValue> {
    /// Function expression that Nix will apply when a described call is demanded.
    value: NixValue,
    /// Expected result and argument interfaces for Rust calls; Nix checks actual values later.
    _type: PhantomData<(R, A)>,
}

expression_handle!(NixCallable<R, A>);

impl<R: NixExpression, A: NixExpression> NixCallable<R, A> {
    /// Construct a Nix function by running `build` once, immediately in Rust.
    /// The callback receives a symbolic parameter of type `A`, not a concrete Nix
    /// value. Its result describes the function body; Nix supplies the argument and
    /// evaluates that body when a later call's result is needed. Rust side effects
    /// in `build` happen during construction, never during those Nix calls.
    ///
    /// ```
    /// use rusix::{Expr, interop::NixCallable};
    ///
    /// let greet = NixCallable::<Expr<String>, Expr<String>>::from_function(|name| {
    ///     name.with_prefix("hello ")
    /// });
    /// let greeting = greet.call("world");
    /// // Like Nix: (name: "hello " + name) "world".
    /// ```
    #[track_caller]
    pub fn from_function(build: impl FnOnce(A) -> R) -> Self {
        Self::from_expression(NixValue::function(|parameter| {
            build(A::from_expression(parameter)).as_expression()
        }))
    }

    /// Construct a Nix function whose body is a structured Rust value.
    /// Runs `build` once immediately with a symbolic parameter, then converts its
    /// result. `R` describes the expected result interface; Nix checks the actual
    /// function behavior when a later call is evaluated.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] if the body cannot be structurally converted,
    /// such as when a flattened field is not a record. Success does not evaluate
    /// the body, check its eventual Nix type, or validate external references.
    #[track_caller]
    pub fn try_from_function<V: IntoRusixValue>(
        build: impl FnOnce(A) -> V,
    ) -> Result<Self, ValidationError> {
        let mut error = None;
        let function = NixValue::function(|parameter| {
            match build(A::from_expression(parameter)).try_into_nix_value() {
                Ok(body) => body,
                Err(failure) => {
                    error = Some(failure);
                    NixValue::null()
                }
            }
        });
        match error {
            Some(error) => Err(error),
            None => Ok(Self::from_expression(function)),
        }
    }

    /// Describe a Nix call with one argument, retaining the expected result type `R`.
    /// Rust converts the argument to its expression interface; Nix runs the function
    /// only when the result is needed.
    #[track_caller]
    pub fn call(&self, argument: impl Into<A>) -> R {
        R::from_expression(self.value.clone().call(argument.into().as_expression()))
    }
}

impl<R: NixExpression> NixCallable<R> {
    /// Describe a call with a user-defined Rust record as its argument.
    /// Converts the argument now and describes a Nix call to evaluate later.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] if Rust structural conversion fails, such as
    /// flattening a non-record field. Success does not check the Nix function's
    /// accepted arguments or evaluate its result.
    #[track_caller]
    pub fn try_call(&self, argument: impl IntoRusixValue) -> Result<R, ValidationError> {
        Ok(self.call(argument.try_into_nix_value()?))
    }
}

/// A deferred Nix collection of named fields, called an *attribute set*.
///
/// Attribute sets serve as records or maps in Nix. Use this handle to select fields
/// or combine sets without computing their values in Rust. `T` describes each
/// member's expression interface: [`NixValue`] allows mixed types, while
/// `NixAttrs<Package>` can represent a family of packages. External values are
/// checked by Nix when these operations are evaluated.
#[derive(Clone, Debug)]
pub struct NixAttrs<T: NixExpression = NixValue> {
    /// Expression for the collection of named Nix fields; field values remain lazy.
    value: NixValue,
    /// Expected Rust interface for each selected field, without evaluating the collection.
    _type: PhantomData<(T,)>,
}

expression_handle!(NixAttrs<T>);

impl<T: NixExpression> NixAttrs<T> {
    /// Construct a record without evaluating its members.
    /// Rust consumes the iterator immediately; the field values remain Nix expressions.
    #[track_caller]
    pub fn new(fields: impl IntoIterator<Item = (impl Into<String>, T)>) -> Self {
        Self::from_expression(NixValue::record(
            fields
                .into_iter()
                .map(|(key, value)| (key, value.as_expression())),
        ))
    }

    /// Select one literal member name, retaining its expression type.
    /// `get("meta.description")` reads a field literally named `meta.description`,
    /// rather than traversing a nested record. Nix reports an absent field when needed.
    #[track_caller]
    pub fn get(&self, name: &str) -> T {
        T::from_expression(self.value.clone().select_segments([name]))
    }

    /// Test attribute presence, including an attribute whose value is null.
    /// `name` is literal. Nix checks existence without evaluating the member's value.
    #[track_caller]
    pub fn has(&self, name: &str) -> Expr<bool> {
        self.value.clone().has_attr(name).into_expr()
    }

    /// Use the fallback only when the attribute is absent, not when it is null.
    /// `name` is literal. Rust constructs the fallback now; Nix evaluates it only
    /// when the field is missing and the result is needed.
    #[track_caller]
    pub fn get_or(&self, name: &str, fallback: impl Into<T>) -> T {
        T::from_expression(
            self.value
                .clone()
                .attr_or(name, fallback.into().as_expression()),
        )
    }

    /// Return null for an absent attribute, retaining the member interface otherwise.
    /// `name` is literal. Use [`Self::has`] to distinguish an absent attribute
    /// from a present null value. Neither case is determined during Rust construction.
    #[track_caller]
    pub fn get_optional(&self, name: &str) -> NixNullable<T> {
        NixNullable::from_expression(self.value.clone().attr_or(name, NixValue::null()))
    }

    /// Combine named fields, using the right-hand value when both sets have a name.
    /// This is Nix's shallow attribute-set union (`//`); member values remain lazy.
    /// Nested sets are replaced as whole values, rather than merged recursively.
    ///
    /// ```
    /// use rusix::interop::{NixAttrs, raw::NixValue};
    ///
    /// let left = NixAttrs::new([("nested", NixValue::record([("a", 1.into())]))]);
    /// let right = NixAttrs::new([("nested", NixValue::record([("b", 2.into())]))]);
    /// let merged = left.merge(right);
    /// // Like Nix: { nested = { a = 1; }; } // { nested = { b = 2; }; }
    /// // The result has nested.b, but no nested.a.
    /// ```
    #[track_caller]
    pub fn merge(self, right: Self) -> Self {
        Self::from_expression(self.value.merge_attrs(right))
    }
}

impl NixAttrs {
    /// Convert a structured Rust record at an attribute-set operation boundary.
    /// This checks Rust structural conversion, not the eventual Nix value type.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] for invalid conversion, such as flattening a
    /// non-record field. Success does not ensure the result is a Nix attribute set;
    /// Nix checks that expectation when an attribute-set operation is evaluated.
    #[track_caller]
    pub fn try_from_record(record: impl IntoRusixValue) -> Result<Self, ValidationError> {
        Ok(Self::from_expression(record.try_into_nix_value()?))
    }

    /// Access a heterogeneous member through an explicit expected interface.
    /// `name` is literal, as in [`Self::get`]. The expected type `T` does not inspect
    /// or validate the member in Rust; Nix checks operations on the selected value.
    #[track_caller]
    pub fn field<T: NixExpression>(&self, name: &str) -> T {
        T::from_expression(self.get(name))
    }
}

/// A deferred, ordered Nix list whose elements use the expression interface `T`.
/// Use it to assemble or conditionally include values that Nix computes later.
/// Rust builds the list description; Nix evaluates its elements only when needed.
#[derive(Clone, Debug)]
pub struct NixList<T: NixExpression> {
    /// Expression for an ordered Nix list whose elements are evaluated only when needed.
    value: NixValue,
    /// Expected Rust interface for list elements; it does not contain evaluated elements.
    _type: PhantomData<(T,)>,
}

expression_handle!(NixList<T>);

impl<T: NixExpression> NixList<T> {
    /// Construct a list by consuming the Rust iterator immediately.
    /// Each element keeps its expression and Rust source location; Nix evaluates
    /// those elements only when needed.
    #[track_caller]
    pub fn new(items: impl IntoIterator<Item = T>) -> Self {
        Self::from_expression(NixValue::list(
            items.into_iter().map(|item| item.as_expression()),
        ))
    }

    /// Join lists in iterator order, preserving element order and lazy values.
    /// Uses Nix's builtin `concatLists` rather than a nixpkgs library function.
    #[track_caller]
    pub fn concat(lists: impl IntoIterator<Item = Self>) -> Self {
        Self::from_expression(NixValue::concat_lists(
            lists.into_iter().map(|list| list.as_expression()),
        ))
    }

    /// Concatenate through the exact supplied library while retaining element types.
    #[track_caller]
    pub fn concat_with(lib: &NixLibrary, lists: impl IntoIterator<Item = Self>) -> Self {
        lib.concat_lists(lists)
    }

    /// Describe `[value]` when `condition` is true, or an empty list when false.
    /// Calls the supplied library's `optional` helper. Rust constructs the value
    /// regardless of the condition; Nix leaves an excluded value unevaluated.
    #[track_caller]
    pub fn optional(lib: &NixLibrary, condition: impl Into<Expr<bool>>, value: T) -> Self {
        lib.optional(condition, value)
    }

    /// Keep this list when `condition` is true, or use an empty list when false.
    /// Calls the supplied library's `optionals` helper without adding a list layer.
    /// Rust constructs the condition now; Nix leaves excluded elements unevaluated.
    #[track_caller]
    pub fn when(self, lib: &NixLibrary, condition: impl Into<Expr<bool>>) -> Self {
        lib.optionals(condition, self)
    }
}

/// A file or directory path that Nix will resolve when its value is needed.
/// Nix paths are a separate value type from strings containing pathnames.
/// Use this handle for recipe files and directories while preserving that distinction.
/// Rust construction does not access the filesystem or check an external value's type.
#[derive(Clone, Debug)]
pub struct NixPath {
    /// Expression expected to produce a Nix filesystem path, without accessing the file in Rust.
    value: NixValue,
}

expression_handle!(NixPath);

impl NixPath {
    /// Convert absolute path text into a Nix filesystem path for source-selection helpers.
    /// Unlike Nix's `builtins.toPath`, the result has type `path`, not `string`.
    /// The input must identify an existing source, not an unbuilt derivation output.
    /// Its string dependency context is discarded so selected files can become
    /// independent build inputs through file sets or [`Self::snapshot`]. No file
    /// is read or copied by this conversion; Nix rejects relative or invalid text
    /// only when it demands the value.
    #[track_caller]
    pub fn from_source_text(path: impl Into<Expr<String>>) -> Self {
        let value = NixValue::builtin("unsafeDiscardStringContext").call(path.into());
        Self::from_expression(NixValue::from_node(Node {
            origin: Origin::caller("absolute filesystem path"),
            kind: crate::ir::ValueKind::FilesystemPath(Box::new(value.0)),
        }))
    }

    /// Copies this existing file or directory into the store when Nix demands the value.
    /// The result is a store-backed string with dependency context for that snapshot.
    /// Use this after selecting a file from a source tree to depend on that file's
    /// contents, rather than retaining a string reference to the entire source tree.
    /// Rust does not read the source. Nix reports missing or inaccessible paths during
    /// evaluation; this operation does not build derivations or fetch sources.
    #[track_caller]
    pub fn snapshot(&self) -> Expr<String> {
        NixValue::builtin("path")
            .call(NixValue::record([("path", self.as_expression())]))
            .into_expr()
    }

    /// Append a nonempty relative path without reading the filesystem in Rust.
    /// Slash-separated components must exclude empty, `.` and `..` segments.
    /// The result remains a Nix path; text is escaped as data and string dependency
    /// context is retained. Nix resolves the path only when it is demanded.
    ///
    /// # Errors
    /// Returns a structural validation error for an absolute path, traversal,
    /// an empty component or NUL. This does not check whether the file exists.
    #[track_caller]
    pub fn join_relative(&self, relative: &str) -> Result<Self, ValidationError> {
        if relative.contains('\0')
            || relative
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
        {
            return Err(ValidationError {
                origin: crate::ir::Origin::caller("relative Nix path"),
                message: "expected a nonempty relative path without empty, . or .. components"
                    .into(),
            });
        }

        Ok(Self::from_expression(NixValue::from_node(
            crate::ir::Node {
                origin: crate::ir::Origin::caller("append relative Nix path"),
                kind: crate::ir::ValueKind::PathAppend {
                    value: Box::new(self.value.clone().0),
                    suffix: format!("/{relative}"),
                },
            },
        )))
    }
}

impl super::ToNixText for NixPath {
    #[track_caller]
    fn to_nix_text(self) -> Expr<String> {
        self.value.to_text().into_expr()
    }
}

/// A Nix expression that may produce an absent value (`null`) or a value described by T.
/// Use it for optional arguments or results whose presence depends on Nix evaluation.
/// Rust Option chooses a variant during authoring; this interface defers that choice.
#[derive(Clone, Debug)]
pub struct NixNullable<T: NixExpression> {
    /// Expression that Nix may resolve to null or to a value with interface T.
    value: NixValue,
    /// Rust interface for the non-null result; the null choice is made during Nix evaluation.
    _type: PhantomData<(T,)>,
}

expression_handle!(NixNullable<T>);

impl<T: NixExpression> NixNullable<T> {
    /// Describe null without evaluating a potential value.
    #[track_caller]
    pub fn null() -> Self {
        Self::from_expression(NixValue::null())
    }

    /// Use this expression as the present alternative, preserving its Rust location.
    /// No Nix evaluation occurs; an externally supplied expression can still evaluate
    /// to null, in which case [`Self::is_null`] will report true.
    #[track_caller]
    pub fn some(value: impl Into<T>) -> Self {
        Self::from_expression(value.into().as_expression())
    }

    /// Test for null in Nix when demanded.
    #[track_caller]
    pub fn is_null(&self) -> Expr<bool> {
        self.value.clone().equals(NixValue::null()).into_expr()
    }

    /// Select a fallback only for null; the unused branch remains unforced.
    /// Rust constructs the fallback immediately. Nix checks for null and evaluates
    /// only the selected expression when the result is needed.
    #[track_caller]
    pub fn unwrap_or(self, fallback: impl Into<T>) -> T {
        self.bind(|value| {
            T::choose(
                value.is_null(),
                fallback.into(),
                T::from_expression(value.value),
            )
        })
    }

    /// Describe a transformation that Nix evaluates only for a non-null value.
    ///
    /// Rust runs `transform` once immediately with a symbolic parameter, even if
    /// this expression is literal null. Its result becomes the non-null branch of
    /// a Nix conditional. Rust side effects in the callback always happen during
    /// construction; only evaluation of the generated expression is conditional.
    ///
    /// ```
    /// use rusix::{Expr, interop::NixNullable};
    ///
    /// let name = NixNullable::<Expr<String>>::some("world");
    /// let greeting = name.map(|name| name.with_prefix("hello ")).unwrap_or("anonymous");
    /// // Like Nix: if name == null then "anonymous" else "hello " + name.
    /// ```
    #[track_caller]
    pub fn map<U: NixExpression>(self, transform: impl FnOnce(T) -> U) -> NixNullable<U> {
        self.bind(|value| {
            NixNullable::choose(
                value.is_null(),
                NixNullable::null(),
                NixNullable::some(transform(T::from_expression(value.value))),
            )
        })
    }
}

/// nixpkgs' standard build environment: build tools, platform information and a recipe builder.
///
/// Use [`Self::mk_derivation`] to describe a software build and the platform views
/// to choose platform-specific inputs or commands. This handle uses the supplied
/// Nix `stdenv`; Rust does not recreate its builders or build the software.
#[derive(Clone, Debug)]
pub struct Stdenv {
    /// Expression for nixpkgs' build tools, platform information and mkDerivation function.
    value: NixValue,
}

expression_handle!(Stdenv);

impl Stdenv {
    /// Describe a build recipe through the supplied `stdenv.mkDerivation`.
    /// Attributes specify fields such as the package name, source and build inputs.
    /// Rust constructs the call; Nix evaluates the recipe later without building it.
    #[track_caller]
    pub fn mk_derivation(&self, attributes: impl ConfigValue) -> Package {
        self.builder().call(NixValue::literal(attributes))
    }

    /// Describe a build recipe from a structured Rust value via `stdenv.mkDerivation`.
    /// Converts the recipe now and leaves the builder call for Nix evaluation.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] if Rust structural conversion fails, such as
    /// flattening a non-record field. Success does not check required builder
    /// fields, evaluate external inputs, or prove the package will build.
    #[track_caller]
    pub fn try_mk_derivation(
        &self,
        attributes: impl IntoRusixValue,
    ) -> Result<Package, ValidationError> {
        self.builder().try_call(attributes)
    }

    /// Obtain the supplied `mkDerivation` function for deferred calls.
    /// It accepts a recipe attribute set or a function from [`super::FinalAttrs`]
    /// to that set, allowing recipe fields to refer to the final overridden values.
    #[track_caller]
    pub fn builder(&self) -> NixCallable<Package> {
        NixCallable::from_expression(self.value.clone().select("mkDerivation"))
    }

    /// Describe the platform where the package's built programs will run.
    /// This is the *host* platform; during cross-compilation it differs from the
    /// build machine. Fields refer to the supplied `stdenv.hostPlatform` record.
    #[track_caller]
    pub fn host_platform(&self) -> super::Platform {
        super::Platform::from_expression(self.value.clone().select("hostPlatform"))
    }

    /// Describe the platform where the package's build commands execute.
    /// This is the *build* platform, which supplies tools runnable during the build.
    /// Fields refer to the supplied `stdenv.buildPlatform` record.
    #[track_caller]
    pub fn build_platform(&self) -> super::Platform {
        super::Platform::from_expression(self.value.clone().select("buildPlatform"))
    }

    /// Describe the platform for which a built compiler will generate code.
    /// This is the *target* platform, relevant when the package being built is a
    /// compiler or similar tool. It usually matches the host platform for ordinary
    /// packages. Fields refer to the supplied `stdenv.targetPlatform` record.
    #[track_caller]
    pub fn target_platform(&self) -> super::Platform {
        super::Platform::from_expression(self.value.clone().select("targetPlatform"))
    }

    /// Compare complete build/host values without reconstructing platform records.
    #[track_caller]
    pub fn build_host_equal(&self) -> Expr<bool> {
        crate::package::build_host_equal(self.value.clone())
    }
}

impl NixRepresentation for NixLibrary {
    fn from_expression(value: NixValue) -> Self {
        Self::from_value(value)
    }

    fn as_expression(&self) -> NixValue {
        self.value.clone()
    }
}

impl IntoRusixValue for NixLibrary {
    #[track_caller]
    fn into_value(self) -> RusixValue {
        RusixValue::leaf(self.as_expression())
    }
}

impl sealed::Sealed for NixLibrary {}

impl ConfigValue for NixLibrary {}

impl IntoNode for NixLibrary {
    fn into_node(self, origin: Origin) -> Node {
        self.as_expression().into_node(origin)
    }
}

impl Expr<String> {
    /// Call optionalString on the exact supplied library, retaining text context.
    #[track_caller]
    pub fn when(self, lib: &NixLibrary, condition: impl Into<Expr<bool>>) -> Self {
        lib.optional_text(condition.into(), self)
    }
}

impl From<NixLibrary> for NixValue {
    fn from(value: NixLibrary) -> Self {
        value.as_expression()
    }
}
