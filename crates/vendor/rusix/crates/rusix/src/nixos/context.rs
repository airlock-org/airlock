//! Bind NixOS arguments and preserve the source of each configuration contribution.

use super::{Boundary, NixosModule};
use crate::Origin;
use crate::interop::{NixExpression, raw::NixValue};

/// Arguments supplied by NixOS when it combines a Rust-authored module with other modules.
/// Values remain symbolic in Rust. Configuration follows the final merged options;
/// packages retain the caller's platform, configuration and overlays.
/// Obtain this context through [`NixosModule::with_context`] or
/// [`NixosModule::with_arguments`]. References must stay inside that callback's result.
#[derive(Clone, Debug)]
pub struct ModuleContext {
    /// Final option values, including definitions from other modules.
    pub config: NixValue,
    /// The library supplied by the caller's module system.
    pub lib: NixValue,
    /// This system's package set; no independent nixpkgs import is introduced.
    pub pkgs: NixValue,
    /// Lexically bound named arguments, including explicitly declared extra inputs.
    arguments: NixValue,
}

impl ModuleContext {
    /// Refers to a declared argument using the requested symbolic interface.
    /// Declare extra names through [`NixosModule::with_arguments`]. NixOS supplies
    /// them from `specialArgs` or `_module.args`; imports that need an argument
    /// during module collection require the caller to supply it through `specialArgs`.
    /// The interface states an expectation; Nix checks actual values when demanded.
    #[track_caller]
    pub fn argument<T: NixExpression>(&self, name: &str) -> T {
        T::from_expression(self.arguments.clone().select_segments([name]))
    }
}

/// A configuration contribution and its imported modules, built inside a module context.
/// Each [`Self::new`] keeps the configuration expression's Rust origin as NixOS
/// definition metadata. Type and merge errors can therefore identify the component
/// that supplied a value even after evaluation has finished.
///
/// Configuration records are kept intact, including literal dotted keys and NixOS
/// conditions, priorities and ordering. Rusix does not guess which nested records
/// are options. Diagnostics identify the contribution's origin and NixOS's option
/// path; opaque records do not acquire invented field-level attribution.
#[derive(Clone, Debug)]
pub struct ModuleBody {
    /// One intact configuration expression and its source, or no direct settings.
    config: Option<NixValue>,
    /// Optional stable NixOS module identity used for import deduplication.
    key: Option<String>,
    /// Imported modules in declaration order, retaining native NixOS merge order.
    imports: Vec<NixValue>,
    /// Definition origins for this contribution and all composed children.
    definitions: Vec<Boundary>,
}

impl ModuleBody {
    /// Starts a module containing only imports, without introducing a definition origin.
    pub fn empty() -> Self {
        Self {
            config: None,
            key: None,
            imports: Vec::new(),
            definitions: Vec::new(),
        }
    }

    /// Contributes configuration while preserving the expression's original Rust location.
    /// Pass configuration fields here; module metadata such as `imports` and `key`
    /// belongs in the corresponding builder methods.
    pub fn new(config: NixValue) -> Self {
        let origin = config.0.origin.clone();
        Self {
            config: Some(config),
            definitions: vec![Boundary {
                path: String::new(),
                origin,
                file: None,
            }],
            ..Self::empty()
        }
    }

    /// Gives this module a stable identity for NixOS import deduplication.
    /// Use the same key only for contributions that should be loaded once together.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Imports a Rust-authored contribution and retains its definition origins.
    /// Import order is passed unchanged to NixOS, whose module collection rules
    /// determine list ordering. This does not flatten or merge configuration records.
    pub fn module(mut self, child: Self) -> Self {
        let (value, definitions) = child.into_parts();
        self.imports.push(value);
        self.definitions.extend(definitions);
        self
    }

    /// Imports a deferred path or module expression supplied by the caller.
    /// Expression origins are retained; definitions inside external Nix modules
    /// keep their Nix locations rather than being attributed to this Rust component.
    pub fn import_value(mut self, module: impl Into<NixValue>) -> Self {
        self.imports.push(module.into());
        self
    }

    /// Produces a native module record without evaluating configuration or imports.
    fn into_parts(self) -> (NixValue, Vec<Boundary>) {
        let mut fields = vec![("imports", NixValue::list(self.imports))];
        if let Some(config) = self.config {
            fields.push(("_file", config.0.origin.id.clone().into()));
            fields.push(("config", config));
        }
        if let Some(key) = self.key {
            fields.push(("key", key.into()));
        }

        (NixValue::record(fields), self.definitions)
    }
}

/// A bound module function and its definition metadata, constructed by
/// [`NixosModule::with_context`] or [`NixosModule::with_arguments`].
#[derive(Clone, Debug)]
pub struct ContextualModule {
    /// Closed function containing all symbolic argument references from the callback.
    pub(super) function: NixValue,
    /// Rust construction boundary used for validation and expression lowering.
    pub(super) origin: Origin,
    /// Configuration contributions carried by the function's body.
    pub(super) definitions: Vec<Boundary>,
}

impl ContextualModule {
    /// Describes a module that can capture expressions from an enclosing build function.
    /// The callback runs in Rust; NixOS later supplies the final configuration and
    /// package set. Captured expressions must remain inside their lexical scope.
    /// Use this when a machine or package constructor owns the module's dependencies.
    #[track_caller]
    pub fn new(build: impl FnOnce(ModuleContext) -> ModuleBody) -> Self {
        Self::with_arguments([] as [&str; 0], build)
    }

    /// Describes a module with additional inputs supplied by the NixOS caller.
    /// Prefer captured Rust values for dependencies already owned by an enclosing
    /// constructor; extra names are for the module system's argument interface.
    #[track_caller]
    pub fn with_arguments(
        extra: impl IntoIterator<Item = impl Into<String>>,
        build: impl FnOnce(ModuleContext) -> ModuleBody,
    ) -> Self {
        let arguments = ["config", "lib", "pkgs"]
            .into_iter()
            .map(String::from)
            .chain(extra.into_iter().map(Into::into));
        let mut definitions = Vec::new();
        let function = NixValue::function_attrs_open(arguments, |arguments| {
            let context = ModuleContext {
                config: arguments.clone().select("config"),
                lib: arguments.clone().select("lib"),
                pkgs: arguments.clone().select("pkgs"),
                arguments,
            };
            let (body, boundaries) = build(context).into_parts();
            definitions = boundaries;

            (Vec::<(&str, NixValue)>::new(), body)
        });

        Self {
            function,
            origin: Origin::caller("NixOS module context"),
            definitions,
        }
    }

    /// Embeds this module in a larger deferred expression without compiling it separately.
    /// NixOS definition labels and expression origins remain in the expression.
    /// A standalone `NixosArtifact` additionally retains structured boundary metadata;
    /// compile through `NixosModule` when that diagnostic sidecar is needed.
    pub fn into_expression(self) -> NixValue {
        self.function
    }
}

impl NixosModule {
    /// Authors a module using the configuration, library and packages supplied by NixOS.
    /// The callback runs once in Rust with symbolic values; NixOS evaluates them later.
    /// See [`Self::with_arguments`] for additional caller-supplied dependencies.
    ///
    /// ```
    /// use rusix::{nix_record, nixos::{ModuleBody, NixosModule}};
    /// let module = NixosModule::with_context(|context| {
    ///     ModuleBody::new(nix_record! {
    ///         "environment": nix_record! {
    ///             "systemPackages": rusix::interop::raw::NixValue::list([
    ///                 context.pkgs.select("git"),
    ///             ]),
    ///         },
    ///     })
    /// });
    /// ```
    #[track_caller]
    pub fn with_context(build: impl FnOnce(ModuleContext) -> ModuleBody) -> Self {
        Self::with_arguments([] as [&str; 0], build)
    }

    /// Authors a module with explicitly named dependencies in addition to `config`, `lib`, `pkgs`.
    /// NixOS inspects these names to supply module arguments. Extra caller arguments
    /// remain accepted and unused values remain lazy. Invalid or duplicate argument
    /// names, including repetitions of standard names, are rejected at compilation.
    /// References escaping the callback are also rejected by Rusix's scope validation.
    ///
    /// ```
    /// use rusix::{interop::NixPath, nixos::{ModuleBody, NixosModule}};
    /// let module = NixosModule::with_arguments(["projectSource"], |context| {
    ///     let source: NixPath = context.argument("projectSource");
    ///     ModuleBody::empty().import_value(source.join_relative("module.nix").unwrap())
    /// });
    /// // The Nix caller supplies projectSource through specialArgs for import resolution.
    /// ```
    #[track_caller]
    pub fn with_arguments(
        extra: impl IntoIterator<Item = impl Into<String>>,
        build: impl FnOnce(ModuleContext) -> ModuleBody,
    ) -> Self {
        let mut module = Self::empty();
        module
            .contextual_modules
            .push(ContextualModule::with_arguments(extra, build));
        module
    }
}
