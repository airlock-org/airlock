//! Refer to existing Nix packages, functions and configuration from Rust.
//!
//! Nix is a language for describing values and build recipes. nixpkgs is a
//! collection of packages and utility functions written in that language; NixOS
//! uses Nix modules to combine system configuration from multiple sources.
//!
//! [`NixValue`] describes a value or expression that Nix will evaluate later.
//! An attribute set is Nix’s collection of named fields, like a Rust record or
//! map. [`PackageRef`] and other handles distinguish common uses of those values
//! without inspecting their contents in Rust. Nix checks lookups and function
//! arguments; Rusix records Rust locations to help explain failures.
use crate::{
    ConfigValue, ValidationError,
    ir::{IntoNode, Node, Origin, Reference, Source},
    sealed,
};
use std::path::PathBuf;

pub mod raw;

use raw::{AttrPath, NixFunctionExt, NixRepresentation, NixValue};

mod library;

mod records;

pub use records::{FinalAttrs, Platform};

mod typed;

mod text;

pub use text::ToNixText;

pub use library::NixLibrary;

pub use typed::{
    IntoNixExpression, NixAttrs, NixCallable, NixExpression, NixList, NixNullable, NixOverridable,
    NixPath, Overlay, Overridable, Package, Stdenv,
};

macro_rules! handle {
    ($name:ident, $docs:literal) => {
        #[doc = $docs]
        #[derive(Clone, Debug)]
        pub struct $name(
            /// Source, literal lookup path and Rust location for the object Nix resolves later.
            pub(crate) Reference,
        );

        impl crate::ir::ReferencedExpression for $name {
            fn reference(&self) -> &Reference {
                &self.0
            }
        }

        impl raw::AsNixValue for $name {
            /// Represent this object as a [`NixValue`] for use in records or function calls.
            /// The lookup and Rust source location are preserved. Rust neither evaluates
            /// the object nor converts it to a string.
            fn as_value(&self) -> NixValue {
                NixValue(self.0.node())
            }
        }
    };
}

handle!(
    PackageRef,
    "A reference to a package that Nix will look up later.

In nixpkgs, a package describes how to build software and where its outputs will
be stored. This handle can be passed to package-specific APIs such as
[`crate::nixos::NixosModule::system_packages`] without building the package.
Rust distinguishes it from a module handle; NixOS checks whether the referenced
value really is a package. The lookup records its Rust source location."
);

handle!(
    ModuleRef,
    "A reference to an existing NixOS configuration module.

A NixOS module contributes settings or declares configurable options for a
system; NixOS combines it with other modules. Import this handle with
[`crate::nixos::NixosModule::import_ref`]. Nix evaluates the module’s contents,
and Rusix records the Rust import location for errors in external Nix code."
);

handle!(
    NixFunction,
    "A reference to a Nix function that Rust can describe calls to.

Use it to reuse existing nixpkgs helpers or functions from a local Nix file.
[`raw::NixFunctionExt::call`] passes one argument;
[`raw::NixFunctionExt::apply`] passes arguments one at a
time, as Nix functions commonly require. Neither executes the function in Rust.
Nix checks the arguments, and the result is a [`NixValue`]; Rusix does not
infer that a function returns a package or module."
);

/// A nixpkgs package definition with named dependencies and feature options.
///
/// A package is commonly defined by a Nix function such as:
/// ```nix
/// { stdenv, lib, openssl, ... }: stdenv.mkDerivation { /* recipe */ }
/// ```
/// [`Nixpkgs::call_package`] examines its argument names and supplies matching
/// dependencies from nixpkgs, with explicit caller arguments taking precedence.
/// Construct one with [`Self::from_function_attrs`]; it stays deferred and supports
/// direct placement through [`ConfigValue`]. Typed calls and bindings retain its
/// interface. Dynamic inspection requires [`raw::NixRepresentation`].
/// Native defaults and `builtins.functionArgs` are preserved. The instantiated
/// result supports nixpkgs' `.override` machinery where Nix permits it.
///
/// The result parameter preserves the body's declared expression interface.
/// Its default is NixValue for dynamic interop; a Package result describes a
/// package, while `NixAttrs<Package>` can describe a family. Rust does not
/// validate external nixpkgs dependencies or prove that they return a derivation;
/// Nix checks the actual arguments and behavior. It wraps the existing function
/// expression, whereas [`NixFunction`] is a reference to an existing function.
#[derive(Clone, Debug)]
pub struct PackageFunction<R: NixExpression = NixValue> {
    /// Nix function with named dependency arguments, lazy defaults and a package or family body.
    function: NixValue,
    /// Declared Rust interface for a call result; it does not hold or evaluate that result.
    result: std::marker::PhantomData<R>,
}

impl<R: NixExpression> PackageFunction<R> {
    /// Define named dependencies/options, optional lazy defaults, and a body.
    /// This uses [`NixValue::function_attrs`]: Rust constructs the expression once
    /// with placeholders, and Nix supplies arguments and evaluates the body later.
    /// Defaults may depend on other arguments and remain unforced until needed.
    ///
    /// ```
    /// use rusix::{IntoConfig, interop::{raw::NixValue, Nixpkgs, PackageFunction}};
    ///
    /// #[derive(IntoConfig)]
    /// struct Output {
    ///     // Reusable package function exported so Nix callers can supply dependencies.
    ///     factory: PackageFunction,
    ///     // Deferred function result whose actual value and type are checked by Nix.
    ///     result: NixValue,
    /// }
    ///
    /// let factory = PackageFunction::from_function_attrs(["lib", "label"], |args| {
    ///     (vec![("label", "example".into())], args.select("label"))
    /// });
    /// let result = Nixpkgs::new().call_package(&factory, NixValue::record([] as [(&str, NixValue); 0]));
    /// let output = Output { factory, result };
    /// ```
    #[track_caller]
    pub fn from_function_attrs<K: Into<String>>(
        arguments: impl IntoIterator<Item = impl Into<String>>,
        build: impl FnOnce(NixValue) -> (Vec<(K, NixValue)>, R),
    ) -> Self {
        Self {
            function: NixValue::function_attrs(arguments, |args| {
                let (defaults, result) = build(args);
                (defaults, result.as_expression())
            }),
            result: std::marker::PhantomData,
        }
    }

    /// Define named dependencies with lazy defaults while accepting extra arguments.
    /// Use this for NixOS module functions, which request dependencies by name but
    /// receive additional module-system arguments. The callback sees only the
    /// declared arguments with their defaults resolved; extra fields are ignored.
    /// See [`NixValue::function_attrs_open`] for the argument-view semantics.
    ///
    /// ```
    /// use rusix::{nix_record, interop::{PackageFunction, raw::NixValue}};
    /// let module = PackageFunction::from_function_attrs_open(["pkgs"], |args| {
    ///     (Vec::<(&str, NixValue)>::new(), nix_record! {
    ///         "environment": nix_record! {
    ///             "systemPackages": NixValue::list([args.select("pkgs.hello")]),
    ///         },
    ///     })
    /// });
    /// // Like { pkgs, ... }: { environment.systemPackages = [ pkgs.hello ]; }
    /// ```
    #[track_caller]
    pub fn from_function_attrs_open<K: Into<String>>(
        arguments: impl IntoIterator<Item = impl Into<String>>,
        build: impl FnOnce(NixValue) -> (Vec<(K, NixValue)>, R),
    ) -> Self {
        Self {
            function: NixValue::function_attrs_open(arguments, |args| {
                let (defaults, result) = build(args);
                (defaults, result.as_expression())
            }),
            result: std::marker::PhantomData,
        }
    }

    /// Call this definition directly, preserving its declared result interface.
    /// For dependency injection and override support, prefer Nixpkgs::call_package.
    #[track_caller]
    pub fn call(&self, arguments: impl ConfigValue) -> R {
        R::from_expression(self.function.clone().call(arguments))
    }
}

impl<R: NixExpression> NixRepresentation for PackageFunction<R> {
    fn from_expression(function: NixValue) -> Self {
        Self {
            function,
            result: std::marker::PhantomData,
        }
    }

    fn as_expression(&self) -> NixValue {
        self.function.clone()
    }
}

impl<R: NixExpression> crate::IntoRusixValue for PackageFunction<R> {
    #[track_caller]
    fn into_value(self) -> crate::RusixValue {
        crate::RusixValue::leaf(self)
    }
}

impl<R: NixExpression> sealed::Sealed for PackageFunction<R> {}

impl<R: NixExpression> ConfigValue for PackageFunction<R> {}

impl<R: NixExpression> IntoNode for PackageFunction<R> {
    fn into_node(self, _: Origin) -> Node {
        self.function.0
    }
}

impl<R: NixExpression> From<PackageFunction<R>> for NixValue {
    fn from(value: PackageFunction<R>) -> Self {
        value.function
    }
}

handle!(
    OverlayRef,
    "A reference to a Nix function that extends or replaces packages in nixpkgs.

Such a function is called an *overlay*. It receives the final package set and
the preceding package set, then returns named additions or replacements. Pass
it to [`Nixpkgs::with_overlay`]; Nix applies it later. Rust does not inspect
package internals or verify the overlay’s function arguments. Convert it to
[`Overlay`] when composing it with authored overlays as a value."
);

impl sealed::Sealed for PackageRef {}

impl ConfigValue for PackageRef {}

impl IntoNode for PackageRef {
    fn into_node(self, _: Origin) -> Node {
        self.0.node()
    }
}

/// Build text containing values that Nix will evaluate later.
/// This resembles named Rust formatting, but returns a [`crate::Expr<String>`]
/// rather than a Rust `String`. Each hole uses [`ToNixText`] and Nix’s
/// `builtins.toString`, so a hole
/// can contain a Rust literal, a symbolic option reference or a package.
///
/// Nix strings can carry dependencies on package outputs. Interpolation keeps
/// those dependencies and the child expressions’ Rust locations; it does not
/// read symbolic values into Rust. Nix evaluates the text only when needed.
///
/// ```
/// use rusix::{nix_text, nixos::OptionRef};
/// let port = OptionRef::<i64>::new("services.example.port").into_expr();
/// let command = nix_text!("postgres --port={port}", port = port);
/// // Represents "postgres --port=" followed by Nix's toString of the final port.
/// ```
///
/// Use `{name}` for a hole and `{{` or `}}` for literal braces. Arguments must
/// be explicitly named and are constructed once, even if reused in the template.
/// Unknown, unused or duplicate names, malformed braces and formatting specifiers
/// are compile-time errors. Width, precision and debug formatting are unsupported.
///
/// # Multiline templates
///
/// A template beginning with a newline removes that first newline and a final
/// indentation-only closing line, then removes the common space/tab prefix from
/// nonblank lines. Relative indentation, blank lines and the newline before the
/// closing line remain. Tabs match tabs, not visual columns. Other templates
/// retain their exact whitespace; interpolated values are never reindented.
///
/// ```
/// use rusix::nix_text;
/// let script = nix_text!(
///     r#"
///         echo {message}
///     "#,
///     message = "ready",
/// );
/// // Describes "echo ready\n" after removing the source indentation.
/// ```
///
/// The comma-separated fragment form remains available for dynamic assembly.
/// Its parts must already be string expressions or string literals; use
/// [`ToNixText::to_nix_text`] for supported coercion or an explicit
/// [`NixValue::into_expr`] expectation for dynamic fragments.
#[macro_export]
macro_rules! nix_text {
    ($template:literal $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::__symbolic_text!($crate; $template $(, $name = $value)*)
    };
    ($($part:expr),* $(,)?) => {
        $crate::Expr::<String>::concat([$($part.into()),*])
    };
}

/// Build a Nix attribute set from named values using a concise macro.
/// An attribute set is Nix’s collection of named fields, like a Rust record or
/// map. Values can mix Rust literals with packages and expressions that Nix will
/// evaluate later; the macro does not evaluate them or convert them to strings.
///
/// ```
/// use rusix::nix_record;
/// let args = nix_record! { "name": "example", "enabled": true };
/// // Represents { name = "example"; enabled = true; }.
/// ```
///
/// Keys are literal strings or parenthesized Rust expressions. Dots remain
/// within one name rather than creating a nested path. Duplicate or NUL-containing
/// keys are rejected during validation. Prefer ordinary Rust structs for large
/// fixed records, and [`NixValue::record`] for dynamic keys.
#[macro_export]
macro_rules! nix_record {
    (@key ($key:expr)) => { $key };
    (@key $key:expr) => { $key };
    ($($key:tt : $value:expr),* $(,)?) => {{
        let fields: ::std::vec::Vec<(::std::string::String, $crate::interop::raw::NixValue)> =
            ::std::vec![$((::std::convert::Into::into($crate::nix_record!(@key $key)), $crate::interop::raw::NixValue::from($value))),*];
        $crate::interop::raw::NixValue::record(fields)
    }};
}

/// Access packages and utility functions from the pinned nixpkgs collection.
///
/// nixpkgs contains build descriptions for software, plus functions and source
/// files used to describe those builds. Its *package set* is a Nix attribute set
/// whose fields include packages such as `git`, helpers such as `writeText`, and
/// `stdenv`, the standard build environment. `stdenv.mkDerivation` turns build
/// attributes into a *derivation*: a recipe with inputs and output paths, not a
/// completed build.
///
/// Use [`Self::get`] for a package, [`Self::pkgs_function`] for a build helper,
/// and [`Self::function`] for a function in nixpkgs’ separate `lib` utility library.
/// Use [`Self::call_package`] to instantiate a [`PackageFunction`] with automatic
/// dependency selection. These methods construct expressions for Nix to evaluate later. They do not
/// fetch, build or inspect packages in Rust, and no package-specific Rust bindings
/// are generated.
///
/// [`Self::new`] uses a standalone package set. [`Self::from_module`] instead
/// uses the package set supplied by NixOS, preserving its platform and customizations.
/// Library lookups always use the pinned `lib`, independently of package overlays.
#[derive(Clone, Debug, Default)]
pub struct Nixpkgs {
    /// Whether package lookups use the NixOS module’s supplied `pkgs` value.
    module_scope: bool,
    /// Functions that extend or replace packages, applied in the author’s order.
    overlays: Vec<Node>,
}

impl Nixpkgs {
    /// Select the pinned nixpkgs collection for standalone package lookups.
    /// The current backend imports it for `x86_64-linux` with an empty nixpkgs
    /// configuration. No Nix evaluation happens here. Use [`Self::from_module`]
    /// to follow a NixOS system’s platform and package customizations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Refer to a file or directory within the pinned nixpkgs source tree.
    /// Use this for an existing patch, script or other build input. The result
    /// represents a Nix path; Rust does not read the file, import Nix code or fetch it.
    /// Paths must be relative and cannot traverse to parent directories.
    #[track_caller]
    pub fn source_path(&self, path: &str) -> NixPath {
        NixPath::from_expression(NixValue(
            Reference {
                source: Source::PinnedPath { path: path.into() },
                path: None,
                origin: Origin::caller(format!("nixpkgs source path {path}")),
            }
            .node(),
        ))
    }

    /// Select the packages supplied to the generated module by NixOS.
    /// NixOS calls this set `pkgs`; it includes the system’s platform, package
    /// configuration and overlays. References constructed here must be compiled
    /// inside a [`crate::nixos::NixosModule`], rather than as standalone configuration.
    pub fn from_module() -> Self {
        Self {
            module_scope: true,
            overlays: vec![],
        }
    }

    fn package_source(&self) -> Source {
        if self.module_scope {
            Source::NixosPackages {
                overlays: self.overlays.clone(),
            }
        } else {
            Source::Packages {
                overlays: self.overlays.clone(),
            }
        }
    }

    /// Add a function that extends or replaces packages in this package set.
    /// Nixpkgs calls these functions *overlays*; Nix applies them in the supplied
    /// order. Accepts Rust-authored [`Overlay`] values or existing [`OverlayRef`]
    /// handles. This extends the NixOS-supplied set or configures the standalone
    /// import. Rust retains the expression without running the overlay in Nix.
    pub fn with_overlay(mut self, overlay: impl Into<Overlay>) -> Self {
        self.overlays.push(overlay.into().as_expression().0);
        self
    }

    /// Describe this package set through a declared Rust field view.
    /// Use a root generated by [`crate::args`] to look up named fields through
    /// accessors such as `pkgs.curl()`. The view keeps this handle's overlays and
    /// standalone or NixOS-supplied package source, including undeclared fields.
    /// Rust constructs the view; Nix checks selected fields when needed.
    ///
    /// ```
    /// use rusix::{ interop::{Nixpkgs, Package}};
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
    /// let pkgs = Nixpkgs::new().view::<packages::Packages>();
    /// let curl: Package = pkgs.curl();
    /// ```
    #[track_caller]
    pub fn view<V: NixExpression>(&self) -> V {
        V::from_expression(NixValue(
            Reference {
                source: self.package_source(),
                path: None,
                origin: Origin::caller("nixpkgs package set view"),
            }
            .node(),
        ))
    }

    /// Refer to a package that Nix will look up later.
    /// `get("git")` represents `pkgs.git`; dots select nested package collections.
    /// Nix checks existence, and lookup failures retain this Rust call location.
    ///
    /// ```
    /// use rusix::interop::Nixpkgs;
    /// let git = Nixpkgs::new().get("git");
    /// // A package reference, not a built Git executable.
    /// ```
    #[track_caller]
    pub fn get(&self, path: &str) -> PackageRef {
        self.path(AttrPath::dotted(path).0)
    }

    /// Refer to a package using literal field names instead of a dotted path.
    /// For example, `["packages", "name.with.dot"]` keeps the second name intact.
    /// Nix performs the lookup later.
    #[track_caller]
    pub fn path(&self, parts: impl IntoIterator<Item = impl Into<String>>) -> PackageRef {
        let path = AttrPath::segments(parts);
        let origin = Origin::caller(format!("nixpkgs package lookup {}", path.0.join(".")));
        PackageRef(Reference {
            source: self.package_source(),
            path: Some(path),
            origin,
        })
    }

    /// Refer to an existing NixOS module file in the pinned nixpkgs tree.
    /// `file` is relative to `nixos/modules`. The module contributes configuration
    /// when imported by NixOS; Rust does not read it. Package overlays do not alter
    /// which source file this handle references.
    #[track_caller]
    pub fn module(&self, file: &str) -> ModuleRef {
        ModuleRef(Reference {
            source: Source::ModuleFile { path: file.into() },
            path: None,
            origin: Origin::caller(format!("NixOS module lookup {file}")),
        })
    }

    /// Refer to a function in nixpkgs’ `lib` utility library.
    /// For example, `function("concatStringsSep")` selects `lib.concatStringsSep`.
    /// Nix checks the function and arguments later. Build helpers such as `writeText`
    /// belong to the package set; select those with [`Self::pkgs_function`].
    #[track_caller]
    pub fn function(&self, path: &str) -> NixFunction {
        NixFunction(Reference {
            source: Source::Library,
            path: Some(AttrPath::dotted(path)),
            origin: Origin::caller(format!("nixpkgs lib function lookup {path}")),
        })
    }

    /// Wrap the pinned nixpkgs `lib` utility library with common call helpers.
    /// This uses the same library as [`Self::function`], without package-set overlays.
    /// To use the `lib` supplied by a package caller instead, wrap that argument
    /// through [`raw::expect::<NixLibrary>`].
    #[track_caller]
    pub fn library(&self) -> NixLibrary {
        NixLibrary::from_value(NixValue(
            Reference {
                source: Source::Library,
                path: None,
                origin: Origin::caller("nixpkgs library lookup"),
            }
            .node(),
        ))
    }

    /// Instantiate a package function using nixpkgs' dependency scope.
    /// This represents `pkgs.callPackage packageFunction overrides`: the real
    /// pinned nixpkgs helper supplies matching dependencies, and the caller's
    /// override attribute set takes precedence. Native defaults remain lazy.
    /// Lookups follow this set's overlays or NixOS-supplied package scope.
    ///
    /// Nix checks missing arguments and the package body. The factory's declared
    /// result interface is preserved; external expectations are not evaluated. Where
    /// supported by nixpkgs, [`Package::override_arguments`] changes package
    /// arguments after instantiation. Dynamic results retain ordinary Nix interop.
    #[track_caller]
    pub fn call_package<R: NixExpression>(
        &self,
        function: &PackageFunction<R>,
        overrides: impl Into<NixValue>,
    ) -> R {
        R::from_expression(
            self.pkgs_function("callPackage")
                .call(function.clone())
                .call(overrides.into()),
        )
    }

    /// Instantiate with a structured Rust override record, lowering at this boundary.
    /// Structural conversion errors are returned without evaluating any Nix values.
    #[track_caller]
    pub fn try_call_package<R: NixExpression>(
        &self,
        function: &PackageFunction<R>,
        overrides: impl crate::IntoRusixValue,
    ) -> Result<R, ValidationError> {
        Ok(self.call_package(function, overrides.try_into_nix_value()?))
    }

    /// Refer to an arbitrary function in the package set, such as `writeText` or
    /// `stdenv.mkDerivation`. These helpers describe generated files or builds.
    /// Lookups follow this set’s package overlays. Use [`Self::function`] for
    /// the separate `lib` utility library; Nix checks function arguments later.
    /// This selector returns a generic function reference, not a [`PackageFunction`].
    /// Use [`Self::call_package`] to instantiate a package definition.
    #[track_caller]
    pub fn pkgs_function(&self, path: &str) -> NixFunction {
        NixFunction(Reference {
            source: self.package_source(),
            path: Some(AttrPath::dotted(path)),
            origin: Origin::caller(format!("nixpkgs function lookup {path}")),
        })
    }
}

/// Refer to objects defined in a local Nix expression file.
///
/// Nix can load a file with `import` and then select fields from its result.
/// Use this handle for packages, modules, overlays or functions that are not
/// in the pinned nixpkgs collection. Rust records the file and lookup location;
/// Nix evaluates its contents later. This API does not fetch inputs or use flakes.
#[derive(Clone, Debug)]
pub struct InputRef {
    /// The local expression file and its human-readable name for diagnostics.
    source: Source,
}

impl InputRef {
    /// Identify a local Nix expression file to load later.
    /// `name` labels the input in diagnostics. Rust does not open or evaluate the
    /// file. Relative paths are resolved from Rust’s working directory when compiled;
    /// the file must still be available when Nix evaluates the generated expression.
    pub fn local(name: impl Into<String>, file: impl Into<PathBuf>) -> Self {
        Self {
            source: Source::Input {
                name: name.into(),
                file: file.into(),
            },
        }
    }

    #[track_caller]
    fn lookup(&self, path: &str, category: &str) -> Reference {
        Reference {
            source: self.source.clone(),
            path: Some(AttrPath::dotted(path)),
            origin: Origin::caller(format!("external {category} lookup {path}")),
        }
    }

    /// Refer to a package returned by this local Nix file.
    /// Dots select nested fields in the file’s result; Nix checks their existence
    /// and whether the result is suitable as a package.
    #[track_caller]
    pub fn package(&self, path: &str) -> PackageRef {
        PackageRef(self.lookup(path, "package"))
    }

    /// Refer to a NixOS configuration module returned by this file.
    /// Pass the handle to [`crate::nixos::NixosModule::import_ref`] to combine it
    /// with other modules; Nix evaluates its contents later.
    #[track_caller]
    pub fn module(&self, path: &str) -> ModuleRef {
        ModuleRef(self.lookup(path, "module"))
    }

    /// Refer to a Nix function returned by this file.
    /// Its calls are described in Rust and executed by Nix later; Rust does not
    /// infer the function’s argument or result types.
    #[track_caller]
    pub fn function(&self, path: &str) -> NixFunction {
        NixFunction(self.lookup(path, "function"))
    }

    /// Refer to an overlay function returned by this file.
    /// An overlay extends or replaces packages; pass it to [`Nixpkgs::with_overlay`]
    /// to have Nix apply it later.
    #[track_caller]
    pub fn overlay(&self, path: &str) -> OverlayRef {
        OverlayRef(self.lookup(path, "overlay"))
    }
}
