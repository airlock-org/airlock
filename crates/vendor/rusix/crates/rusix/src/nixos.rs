//! Describe and combine NixOS system configuration from Rust.
//!
//! A NixOS module contributes settings to a larger system configuration. An
//! *option* is a configurable field with a declared type, default and documentation.
//! Multiple modules can define the same option; NixOS combines those definitions
//! according to its type and their priorities, for example joining lists.
//!
//! [`NixosModule`] collects contributions and imports. [`OptionDecl`] describes
//! the public options others can configure. [`OptionRef`] refers to an option’s
//! value after all modules have been combined. Rust describes each of these;
//! NixOS performs evaluation, type checking and merging later.
use crate::interop::raw::AttrPath;
use crate::interop::raw::NixFunctionExt;
use crate::interop::{ModuleRef, Nixpkgs, PackageRef, raw::NixValue};
use crate::ir::{IntoNode, ReferencedExpression};
use crate::{Config, Expr, IntoConfig, Node, Origin, ValueKind};
use std::marker::PhantomData;

mod context;

mod schema;

pub use context::{ContextualModule, ModuleBody, ModuleContext};

pub use schema::{OptionDecl, OptionType};

/// Describe a rule that NixOS should require to be true.
/// The result is the standard `{ assertion = condition; message = message; }`
/// record for NixOS’s `assertions` list. On a false condition, NixOS reports the
/// message when assertions are checked. Rust does not check the condition.
///
/// The condition and message can be expressions that Nix evaluates later. Their
/// Rust locations are preserved, but the message is unchanged for compatibility.
/// Use [`NixosModule::assertion`] for a concrete message that also identifies the
/// Rust rule when its condition is false. Nix checks actual boolean/string types.
///
/// ```
/// use rusix::nixos;
/// let rule = nixos::assertion(true, "the service requires a valid port");
/// // Add this record to a configuration contribution’s assertions list.
/// ```
#[track_caller]
pub fn assertion(condition: impl Into<NixValue>, message: impl Into<NixValue>) -> NixValue {
    NixValue::record([("assertion", condition.into()), ("message", message.into())])
}

/// Combine several groups of NixOS definitions using `lib.mkMerge`.
/// NixOS combines their settings according to option types and priorities later.
/// This constructs module-system metadata, not a Rust map merge or a generic
/// Nix attribute-set union.
#[track_caller]
pub fn merge(values: impl IntoIterator<Item = NixValue>) -> NixValue {
    Nixpkgs::new()
        .function("mkMerge")
        .call(NixValue::list(values))
}

impl NixValue {
    /// Include these NixOS definitions only when `condition` evaluates to true.
    /// This represents `lib.mkIf condition definitions`; NixOS processes the
    /// condition while combining modules. Unlike [`Self::if_else`], it describes
    /// conditional configuration definitions rather than selecting an ordinary value.
    #[track_caller]
    pub fn when(self, condition: impl Into<Self>) -> Self {
        Nixpkgs::new()
            .function("mkIf")
            .apply([condition.into(), self])
    }

    /// Choose how these definitions compete with definitions from other NixOS modules.
    /// For example, `Default` allows an ordinary definition to replace this value.
    /// NixOS chooses and merges definitions later. For an entire module’s direct
    /// settings, use [`NixosModule::priority`].
    #[track_caller]
    pub fn priority(self, priority: DefinitionPriority) -> Self {
        let library = Nixpkgs::new();
        match priority {
            DefinitionPriority::Normal => self,
            DefinitionPriority::Default => library.function("mkDefault").call(self),
            DefinitionPriority::Force => library.function("mkForce").call(self),
            DefinitionPriority::Override(value) => {
                library.function("mkOverride").apply([value.into(), self])
            }
        }
    }

    /// Place this definition before ordinary list or line-based text definitions.
    /// This uses NixOS `lib.mkBefore`; it changes merge order, not which override
    /// priority wins.
    #[track_caller]
    pub fn before(self) -> Self {
        Nixpkgs::new().function("mkBefore").call(self)
    }

    /// Place this definition after ordinary list or line-based text definitions.
    /// This uses NixOS `lib.mkAfter`; it changes merge order, not which override
    /// priority wins.
    #[track_caller]
    pub fn after(self) -> Self {
        Nixpkgs::new().function("mkAfter").call(self)
    }
}

/// A reference to a NixOS option’s value after all modules have been combined.
///
/// NixOS options are configurable fields such as `services.example.port`.
/// Several modules can set them, and NixOS applies their types and priorities
/// to produce the final configuration. This reference lets another expression
/// use that final value, including overrides supplied by ordinary Nix modules.
///
/// Rust never reads the referenced value. [`Self::into_expr`] creates an
/// [`Expr<T>`] that Nix evaluates later. `T` states the Rust author’s expected
/// type; Rusix does not prove it matches the declared NixOS option. NixOS
/// remains responsible for option existence, actual types and merging.
///
/// ```
/// use rusix::nixos::OptionRef;
/// let port = OptionRef::<i64>::new("services.example.port");
/// let command = port.into_expr().to_text().with_prefix("example --port=");
/// // Another NixOS module can change the port without rerunning this Rust code.
/// ```
///
/// Supported typed expressions are `i64`, `bool` and `String`; use
/// [`crate::interop::raw::AsNixValue::as_value`] for other expected shapes. References require compilation
/// as a NixOS module. Their Rust location is recorded when constructed.
/// [`crate::options`] generates these references from a finite structural declaration.
#[derive(Clone, Debug)]
pub struct OptionRef<T> {
    /// Field names identifying the NixOS option whose final value is used.
    path: AttrPath,
    /// Rust location that declared this dependency, used if evaluation fails.
    origin: Origin,
    /// Expected value category for Rust composition, not proof of the NixOS type.
    ty: PhantomData<T>,
}

impl<T> OptionRef<T> {
    /// Refer to an option using a dotted path, such as `services.example.port`.
    /// This records a dependency without reading the option or checking its type.
    /// Use [`Self::from_segments`] for a field name containing a literal dot.
    #[track_caller]
    pub fn new(path: &str) -> Self {
        Self::from_segments(path.split('.'))
    }

    /// Refer to an option using literal field names.
    /// `["services", "example", "custom.key"]` keeps the last name as one field,
    /// including its dot. No option value is read or checked in Rust.
    #[track_caller]
    pub fn from_segments(parts: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let path = AttrPath::segments(parts);
        Self {
            origin: Origin::caller(format!("NixOS option reference {}", path.parts().join("."))),
            path,
            ty: PhantomData,
        }
    }

    /// Represent the option lookup as an [`Expr<T>`] with its original Rust location.
    /// The value still belongs to Nix evaluation. Only `Expr<bool>`, `Expr<i64>` and
    /// `Expr<String>` currently support configuration generation; use
    /// [`crate::interop::raw::AsNixValue::as_value`] for other expected shapes.
    pub fn into_expr(self) -> Expr<T> {
        Expr::new(ValueKind::OptionReference(self.path), self.origin)
    }
}

impl<T> crate::interop::raw::AsNixValue for OptionRef<T> {
    fn as_value(&self) -> NixValue {
        NixValue::from_node(Node {
            origin: self.origin.clone(),
            kind: ValueKind::OptionReference(self.path.clone()),
        })
    }
}

/// A NixOS source file to load, together with the Rust location that requested it.
/// NixOS adds that file’s settings and option declarations to the combined
/// configuration. Normally created by [`NixosModule::import`].
#[derive(Clone, Debug)]
pub struct Import {
    /// Relative file under the evaluator's nixpkgs root; absolute/parent paths are rejected.
    pub path: String,
    /// Rust import boundary used when failures cross into the imported module.
    pub origin: Origin,
}

/// A rule whose condition NixOS checks and whose failure names the Rust rule.
/// The condition is evaluated by Nix later, not by Rust. Normally created by
/// [`NixosModule::assertion`]; the stored message includes source attribution
/// when generated for NixOS.
#[derive(Clone, Debug)]
pub struct Assertion {
    /// Diagnostic identity of this assertion, not a declared NixOS option name.
    pub name: String,
    /// Boolean expression NixOS must find true when assertions are checked.
    pub condition: Node,
    /// Explanation to report if the rule is false, alongside the Rust location.
    pub message: String,
    /// Rust operation that introduced the assertion.
    pub origin: Origin,
}

/// A group of configuration contributions to combine into a NixOS module.
///
/// A NixOS module supplies settings or declares configurable options. NixOS
/// combines modules into one system configuration, using each option’s type and
/// priority to resolve multiple definitions. This Rust type collects those
/// settings, declarations, imports and assertions without running NixOS.
///
/// Each [`Self::add`] preserves an independent contribution, so list merging,
/// overrides and errors involving several Rust locations remain meaningful.
/// [`OptionRef`] dependencies follow the final values supplied by both Rust and
/// ordinary Nix modules. Constructing this object does not evaluate options,
/// build packages or activate a system.
#[derive(Clone, Debug)]
pub struct NixosModule {
    /// Settings contributed directly by this module; its priority applies to these settings.
    pub config: Config,
    /// Declarations of configurable fields, their NixOS types, defaults and documentation.
    pub options: Config,
    /// Expressions for modules produced by Nix helpers, such as renamed-option adapters.
    pub generated_imports: Vec<(NixValue, Origin)>,
    /// Bound module functions with source metadata for their Rust-authored contributions.
    pub contextual_modules: Vec<ContextualModule>,
    /// Pinned-tree imports, each retaining its introducing Rust operation.
    pub imports: Vec<Import>,
    /// Assertion contributions, merged into NixOS's ordinary assertion list.
    pub assertions: Vec<Assertion>,
    /// Separately contributed modules, each with its own settings and priority.
    pub modules: Vec<NixosModule>,
    /// Override policy for this module's direct bindings, not its children or imports.
    pub priority: DefinitionPriority,
    /// Existing NixOS module handles paired with the Rust locations that imported them.
    pub opaque_imports: Vec<(ModuleRef, Origin)>,
}

/// How a NixOS definition competes with other definitions of the same option.
/// NixOS first keeps definitions with the winning priority, then merges them
/// according to the option type. Lower numeric priorities win. For example,
/// `Default` provides a fallback, while an ordinary definition can replace it.
/// This differs from before/after ordering of list elements.
#[derive(Clone, Copy, Debug, Default)]
pub enum DefinitionPriority {
    /// Ordinary definitions, left unwrapped (NixOS's normal override priority is 100).
    #[default]
    Normal,
    /// `mkDefault` (1000): ordinary definitions take precedence over these defaults.
    Default,
    /// `mkForce` (50): wins over ordinary/default definitions, subject to lower overrides.
    Force,
    /// `mkOverride` with an explicit priority; lower numbers win in NixOS.
    Override(u16),
}

impl DefinitionPriority {
    /// Numeric wrapper priority used by lowering; `None` leaves ordinary definitions unwrapped.
    pub fn override_priority(self) -> Option<u16> {
        match self {
            Self::Normal => None,
            Self::Default => Some(1000),
            Self::Force => Some(50),
            Self::Override(priority) => Some(priority),
        }
    }
}

impl NixosModule {
    /// Start a composition with no direct bindings, imports, assertions or children.
    #[track_caller]
    pub fn empty() -> Self {
        Self::new(Config::new())
    }

    /// Wrap one existing binding contribution without changing its origins.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            options: Config::new(),
            generated_imports: vec![],
            contextual_modules: vec![],
            imports: vec![],
            assertions: vec![],
            modules: vec![],
            priority: DefinitionPriority::Normal,
            opaque_imports: vec![],
        }
    }

    /// Add a Rust component’s settings as an independent NixOS contribution.
    /// Rust converts the component now, but Nix evaluates its expressions and
    /// combines its settings with other contributions later. Lists can merge and
    /// conflicting values can identify every contributing source.
    ///
    /// Duplicate paths inside one Config still fail Rusix validation. To give a
    /// whole contribution a priority, wrap it with [`Self::new`] and
    /// [`Self::priority`], then add it with [`Self::module`]. Tracked conversions
    /// record the author’s call location.
    ///
    /// ```
    /// use rusix::{ nixos::NixosModule};
    /// #[rusix::config]
    /// mod configuration {
    ///     #[rusix(root)]
    ///     pub struct Layer {
    ///         // Settings for files made available in the NixOS system environment.
    ///         pub environment: Environment,
    ///     }
    ///
    ///     pub struct Environment {
    ///         // Package subdirectories, such as /share, linked into the system environment.
    ///         pub paths_to_link: Vec<String>,
    ///     }
    /// }
    /// use configuration::{Environment, Layer};
    /// let module = NixosModule::empty()
    ///     .add(Layer { environment: Environment { paths_to_link: vec!["/share".into()] } })
    ///     .add(Layer { environment: Environment { paths_to_link: vec!["/lib".into()] } });
    /// // NixOS merges both list definitions; Rusix does not choose a winner.
    /// ```
    #[track_caller]
    // Component addition is an authoring operation; no arithmetic/+ API is intended.
    #[allow(clippy::should_implement_trait)]
    pub fn add<T: IntoConfig>(self, value: T) -> Self {
        self.module(Self::new(value.into_config()))
    }

    /// Add an already assembled child, preserving its imports, assertions and priority.
    pub fn module(mut self, module: NixosModule) -> Self {
        self.modules.push(module);
        self
    }

    /// Set the override priority of this module’s directly contributed settings.
    /// NixOS applies it when competing definitions are combined. Child modules keep
    /// their own priorities, and assertions retain ordinary list merging.
    pub fn priority(mut self, priority: DefinitionPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Include a NixOS module file from the pinned nixpkgs source tree.
    /// `path` is relative to that tree. Use [`Self::import_ref`] for handles obtained
    /// from package collections or local files. Rusix validates the path, while
    /// NixOS evaluates the file’s settings and declarations later.
    #[track_caller]
    pub fn import(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        self.imports.push(Import {
            origin: Origin::caller(format!("import {path}")),
            path,
        });
        self
    }

    /// Declare configurable NixOS fields, their types, defaults and documentation.
    /// Unlike [`Self::add`], this defines which options are available rather than
    /// setting their current values. Nested Rust fields determine option paths;
    /// leaves should be [`OptionDecl`] values. Ordinary Nix modules can then assign
    /// those options. NixOS checks types and performs merging, independently of
    /// Rust’s own type system.
    ///
    /// ```
    /// use rusix::{ nixos::{NixosModule, OptionDecl, OptionType}};
    ///
    /// #[rusix::config]
    /// mod schema {
    ///     use rusix::nixos::OptionDecl;
    ///
    ///     #[rusix(root)]
    ///     pub struct Root {
    ///         // Type, default and documentation for a configurable NixOS option named example.
    ///         pub example: OptionDecl,
    ///     }
    /// }
    /// let module = NixosModule::empty().declare(schema::Root {
    ///     example: OptionDecl::new(OptionType::named("bool"))
    ///         .default(false).description("Enable the example."),
    /// });
    /// ```
    #[track_caller]
    pub fn declare<T: IntoConfig>(mut self, schema: T) -> Self {
        let mut child = Self::empty();
        child.options = schema.into_config();
        self.modules.push(child);
        self
    }

    /// Add a NixOS module computed by a Nix expression, such as a helper function call.
    /// NixOS evaluates the expression and checks the resulting module structure.
    /// The Rust import location is recorded separately from ordinary Nix definitions
    /// that later configure the module’s options.
    #[track_caller]
    pub fn import_value(mut self, module: NixValue) -> Self {
        let origin = Origin::caller("import generated NixOS module");
        self.generated_imports.push((module, origin));
        self
    }

    /// Include an existing NixOS module referenced by [`ModuleRef`].
    /// NixOS evaluates its contents and combines them with other contributions.
    /// Errors in external Nix code can be attributed to this Rust import location.
    #[track_caller]
    pub fn import_ref(mut self, module: ModuleRef) -> Self {
        let origin = Origin::caller(format!(
            "import opaque module {}",
            module.reference().origin.purpose
        ));
        self.opaque_imports.push((module, origin));
        self
    }

    /// Add packages to the system-wide software available through NixOS’s
    /// `environment.systemPackages` option. This describes the package list without
    /// building or installing it; NixOS checks each reference is actually a package.
    ///
    /// This adds one direct setting. Repeated calls on the same module conflict in
    /// Rusix validation; use separate module contributions when lists should merge.
    /// Only [`PackageRef`] values are accepted, rather than module handles:
    ///
    /// ```compile_fail
    /// # use rusix::{nixos::NixosModule, interop::Nixpkgs};
    /// let module = NixosModule::empty().system_packages(vec![Nixpkgs::new().module("services/networking/ssh/sshd.nix")]);
    /// ```
    #[track_caller]
    pub fn system_packages(mut self, packages: Vec<PackageRef>) -> Self {
        self.config = self
            .config
            .set_dynamic("environment.systemPackages", packages);
        self
    }

    /// Add a rule whose failure message identifies this Rust call.
    /// NixOS requires the boolean condition to be true when assertions are checked.
    /// Rust does not read the condition; evaluation methods decide when to check it.
    /// `name` labels the rule in diagnostics and `message` explains a false result.
    #[track_caller]
    pub fn assertion(
        mut self,
        name: impl Into<String>,
        condition: Expr<bool>,
        message: impl Into<String>,
    ) -> Self {
        let name = name.into();
        let origin = Origin::caller(format!("assertion assertions.{name}"));
        self.assertions.push(Assertion {
            name,
            condition: condition.into_node(origin.clone()),
            message: message.into(),
            origin,
        });
        self
    }
}

mod compile;

pub(crate) use compile::pin;
pub use compile::{
    Boundary, DRIVER, NIXPKGS_REVISION, NixosArtifact, compile_module, compile_module_with_options,
    evaluation_source,
};
