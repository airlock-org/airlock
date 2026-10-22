//! Describe Nix configuration with ordinary Rust structs, enums and functions.
//!
//! Nix is a language used to describe configuration and software builds. Rusix
//! turns Rust values into Nix expressions; Nix evaluates those expressions later.
//! Rust code can also describe references and calls without reading their results.
//!
//! Start with [`config`] for a local configuration tree. Nested structs become
//! nested named fields in Nix. Use [`IntoConfig`] and [`IntoRusixValue`] derives
//! for reusable types, [`interop`] to reuse existing packages and functions, and
//! [`nixos::NixosModule`] to combine configuration for NixOS. NixOS combines
//! modules and checks configurable fields, called *options*, during evaluation.
//!
//! [`compile`] generates Nix source from a Rust description. With the default
//! `evaluation` feature, `NixSession` can
//! evaluate the generated source in an offline, disposable Nix store. Compilation
//! does not launch Nix; evaluation never builds packages or activates a system.
//! Disable default features to use authoring and compilation without the evaluator.
//! Evaluation supports Linux; the fixture executable requires `dev-cli`.
#![warn(missing_docs)]

#[cfg(all(feature = "evaluation", not(unix)))]
compile_error!(
    "the evaluation feature requires Unix; use default-features = false for compilation only"
);

extern crate self as rusix;

use ir::{Assignment, Node, Origin, ValueKind};

/// Semantic IR construction and inspection for compiler/backend implementations.
pub mod ir;

mod authoring;

/// Compile deferred Rust descriptions into Nix source and inspection syntax.
pub mod compiler;

#[cfg(feature = "evaluation")]
mod evaluation;

pub mod diagnostic;

pub mod interop;

pub mod installation;

pub mod nixos;

pub mod package;

/// Common typed authoring interfaces; raw interop and backend access require explicit imports.
pub mod prelude;

mod value;

/// Turn an inline Rust module’s local types into Nix configuration values.
/// Nested structs become nested Nix attribute sets: collections of named fields.
/// Mark a root struct with `#[rusix(root)]` so it can be added to a
/// [`nixos::NixosModule`]. NixOS will combine that contribution with other modules.
/// The macro describes values; it does not evaluate Nix or declare option types.
///
/// ```
/// use rusix::{ nixos::NixosModule};
///
/// #[rusix::config]
/// mod configuration {
///     #[rusix(root)]
///     pub struct Machine {
///         // Service settings emitted under the Nix services namespace.
///         pub services: Services,
///     }
///
///     pub struct Services {
///         // Settings for the fictional service, emitted under services.example.
///         pub example: Example,
///     }
///
///     pub struct Example {
///         // Whether to enable the fictional service; this becomes services.example.enable.
///         pub enable: bool,
///     }
/// }
///
/// use configuration::{Machine, Services, Example};
/// let module = NixosModule::empty().add(Machine {
///     services: Services { example: Example { enable: true } },
/// });
/// // Defines services.example.enable = true in the generated NixOS module.
/// ```
///
/// # Mapping rules
///
/// Local structs and unit enums receive the existing conversion derives. Types
/// imported from elsewhere must already implement the conversion traits. Multiple
/// roots are allowed. Only inline modules are supported; no external files or
/// type definitions are inspected.
///
/// `rename_all = "preserve"` retains Rust field names, including underscores.
/// Fields use lowerCamelCase by default. `rename_all = "PascalCase"` changes the
/// convention for a struct; `rename` handles a field exception. `skip` omits a
/// field, and `flatten` inserts a nested record’s fields at its parent’s level.
/// Symbolic expressions and package handles keep their Nix behavior.
///
/// `Option<T>` normally maps `None` to Nix `null`. Explicit `#[rusix(omit_none)]`
/// omits a field when it is `None`. On a named struct, it applies only to direct
/// `Option` fields and does not propagate into nested types. Use field annotations
/// when some absent fields should be omitted and others should be null.
///
/// Unit enums become strings: `Server` becomes `"server"`, and `ReadOnly` becomes
/// `"readOnly"`. Enums carrying data need an explicit conversion that chooses
/// what their data means in Nix:
///
/// ```compile_fail,E0277
/// ///
/// #[rusix::config]
/// mod configuration {
///     enum Mode { Server, Client { endpoint: String } }
///
///     #[rusix(root)]
///     struct Machine {
///         // Service mode requiring explicit Nix lowering because Mode carries variant data.
///         mode: Mode,
///     }
/// }
/// ```
pub use rusix_derive::config;

/// Generate typed accessors for final NixOS configuration values.
/// NixOS combines configuration supplied by many modules. These accessors refer
/// to the resulting values, so other modules’ overrides still affect dependent
/// expressions. Rust constructs references; it never reads the final values.
/// This declares dependencies, not NixOS options or their actual types.
///
/// ```
/// use rusix::{ Expr};
///
/// #[rusix::options]
/// mod options {
///     #[rusix(root)]
///     struct Root {
///         // Service fields looked up after NixOS combines all configuration modules.
///         services: Services,
///     }
///
///     struct Services {
///         // Final settings of the fictional service, selected from services.example.
///         example: Example,
///     }
///
///     struct Example {
///         // Final service port selected from the combined NixOS configuration.
///         port: i64,
///     }
/// }
/// let port: Expr<i64> = options::root().services.example.port();
/// // Equivalent to OptionRef::<i64>::new("services.example.port").into_expr().
/// ```
///
/// Use exactly one root in an inline module. Local structs provide navigation;
/// leaf calls capture their Rust call location. `bool`, `String` and `i64`
/// leaves return [`Expr`] with the declared expectation. `NixValue`, `Option`,
/// `Vec`, `BTreeMap` and `HashMap` leaves return Nix expressions, not Rust
/// collections. NixOS checks option existence and actual value types.
///
/// Naming follows [`config`]. Mark a nested struct `#[rusix(value)]` to expose
/// `as_value()` for its entire Nix subtree. Roots have no whole-value accessor
/// or arbitrary field traversal. External symbolic interfaces can be reused with
/// `#[rusix(expression)]`; the declared type must implement [`interop::NixExpression`].
pub use rusix_derive::options;

/// Generate typed accessors for values in a Nix function’s named arguments.
/// Nixpkgs package functions commonly receive an attribute set of dependencies
/// and feature choices. This macro describes the fields an adapter needs.
/// Accessors construct Nix expressions; they do not read argument values in Rust.
///
/// ```
/// use rusix::{ interop::raw::NixValue};
///
/// #[rusix::args]
/// mod arguments {
///     #[rusix(root)]
///     struct Inputs {
///         // Feature choice looked up in the supplied Nix function arguments.
///         // Literal enable flag emitted as the Nix field enabled.
///         enabled: bool,
///         // Machine information looked up in the supplied Nix function arguments.
///         platform: Platform,
///     }
///
///     struct Platform {
///         // Nix CPU and operating-system identifier, such as x86_64-linux.
///         system: String,
///     }
/// }
/// let value = NixValue::record([
///     ("enabled", true.into()),
///     ("platform", NixValue::record([("system", "x86_64-linux".into())])),
/// ]);
/// let args = arguments::from_value(value);
/// let enabled = args.enabled();
/// let system = args.platform.system();
/// // Each accessor describes a field lookup; no Nix evaluation happens here.
/// ```
///
/// Bind the view to an existing [`interop::raw::NixValue`] using generated `from_value`.
/// This also works with placeholders supplied by
/// [`interop::raw::NixValue::function_attrs`]. Nix remains responsible for caller
/// arguments, defaults and actual types.
///
/// Argument roots also implement [`interop::NixExpression`]: they can be typed
/// callback parameters or package-set views through [`interop::Nixpkgs::view`].
/// Their `as_attrs()` method retains the complete argument record, including
/// undeclared fields, without reading it in Rust. Nested views need `#[rusix(value)]`
/// to support these whole-value operations.
///
/// Use one root in an inline module. Leaf types, naming and optional subtree
/// `as_value()` access follow [`options`]. Views can be cloned without evaluating
/// them. Accessor calls record their own Rust locations. External aliases and
/// view types are not inspected. Mark a field `#[rusix(expression)]` to retain
/// an external type or alias implementing [`interop::NixExpression`]. Rust checks
/// that trait contract; Nix checks the actual external value when demanded.
/// Retain raw NixValue for dynamic selections.
/// Generated `argument_names()` returns the mapped names of the root's direct
/// fields in declaration order. When the view declares the complete public
/// interface, pass these names to [`interop::PackageFunction::from_function_attrs`]
/// instead of maintaining a separate name list. Partial views list only their
/// declared fields; defaults and requiredness still belong to the function builder.
pub use rusix_derive::args;

#[doc(hidden)]
pub use rusix_derive::symbolic_text as __symbolic_text;

pub use rusix_derive::{IntoConfig, IntoRusixValue};
pub use value::{IntoRusixValue, RusixValue};

pub(crate) use authoring::sealed;
pub use authoring::{Config, ConfigValue, Expr, IntoConfig, ValidationError};
pub use compiler::{
    Generated, RenderLayout, RenderOptions, SourceSpan, compile, compile_value,
    compile_with_options,
};
pub use diagnostic::{Diagnostic, DiagnosticKind, DiagnosticOrigin, OriginRole, Provenance};

#[cfg(feature = "evaluation")]
pub use evaluation::{Evaluation, NixSession};
