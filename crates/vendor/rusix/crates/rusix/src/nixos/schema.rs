//! Declare which NixOS configuration fields a module accepts.
//!
//! A declaration supplies a field’s type, default and documentation; it does not
//! assign its current configuration value. NixOS’s `lib.mkOption` and `lib.types`
//! perform the actual checking and merging. Rust only describes those declarations.
use crate::interop::raw::{NixFunctionExt, NixpkgsExt};
use crate::{
    IntoRusixValue, RusixValue, ValidationError,
    interop::{Nixpkgs, raw::NixValue},
};

/// A rule NixOS uses to validate and merge configuration values.
/// For example, `lib.types.port` requires a valid port number, while `listOf`
/// checks elements and combines list definitions from several modules.
///
/// This wrapper describes a real NixOS type object, not a Rust value type.
/// Construction does not check configuration in Rust; NixOS applies validation,
/// conversion and merge rules when evaluating the modules.
#[derive(Clone, Debug)]
pub struct OptionType(
    /// The expression for NixOS’s type object, including its checking and merging functions.
    NixValue,
);

impl OptionType {
    /// Choose a type from NixOS’s `lib.types`, such as `bool`, `str`, `path`,
    /// `port` or `package`. For example, `named("port")` describes `lib.types.port`.
    /// Nix checks whether the type exists; Rusix does not maintain a type catalogue.
    #[track_caller]
    pub fn named(name: &str) -> Self {
        Self(Nixpkgs::new().lib_value(&format!("types.{name}")))
    }

    /// Wrap an existing NixOS type expression, including a custom or composed type.
    /// The expression should produce the object NixOS uses to check and merge
    /// option values; Rust does not evaluate or verify it.
    pub fn opaque(value: NixValue) -> Self {
        Self(value)
    }

    /// Return the expression for this NixOS type object as a [`NixValue`].
    /// Use it when passing types to other Nix helpers; Rust does not evaluate the type.
    pub fn as_value(&self) -> NixValue {
        self.0.clone()
    }

    /// Allow Nix null in addition to this type's values.
    #[track_caller]
    pub fn null_or(self) -> Self {
        Self(Nixpkgs::new().function("types.nullOr").call(self.0))
    }

    /// Require a list whose elements match this type.
    /// NixOS checks each element and combines surviving list definitions in order.
    #[track_caller]
    pub fn list_of(self) -> Self {
        Self(Nixpkgs::new().function("types.listOf").call(self.0))
    }

    /// Require a Nix attribute set whose named values match this type.
    /// Keys remain open-ended. NixOS merges definitions per key using the value type.
    #[track_caller]
    pub fn attrs_of(self) -> Self {
        Self(Nixpkgs::new().function("types.attrsOf").call(self.0))
    }

    /// Require a Nix function whose results are checked using this type.
    /// This describes `lib.types.functionTo`; Rust does not call the function or
    /// inspect its arguments.
    #[track_caller]
    pub fn function_to(self) -> Self {
        Self(Nixpkgs::new().function("types.functionTo").call(self.0))
    }

    /// Accept a value matching one of these NixOS types.
    /// This uses `lib.types.oneOf`; NixOS selects the matching type and applies
    /// its validation and merge rules later.
    #[track_caller]
    pub fn one_of(types: impl IntoIterator<Item = Self>) -> Self {
        Self(
            Nixpkgs::new()
                .function("types.oneOf")
                .call(NixValue::list(types.into_iter().map(|t| t.0))),
        )
    }

    /// Accept `source` values by converting them before checking this target type.
    /// `coercion` describes a Nix function that converts one source value. NixOS
    /// performs conversion and merging through `lib.types.coercedTo`; Rust does
    /// not run that function.
    #[track_caller]
    pub fn coerced_from(self, source: Self, coercion: NixValue) -> Self {
        Self(
            Nixpkgs::new()
                .function("types.coercedTo")
                .apply([source.0, coercion, self.0]),
        )
    }

    /// Declare a nested configuration with its own options and defaults.
    /// NixOS calls this a *submodule*: values supplied by several modules are
    /// combined and checked against these nested declarations.
    ///
    /// `options` supplies a structural tree of [`OptionDecl`] values. `freeform`
    /// allows additional, undeclared fields checked using that type; without it,
    /// NixOS rejects undeclared names. Rust returns errors for invalid structural
    /// conversion, while NixOS checks definitions and applies nested defaults later.
    #[track_caller]
    pub fn submodule(
        options: impl IntoRusixValue,
        freeform: Option<Self>,
    ) -> Result<Self, ValidationError> {
        let mut fields = vec![("options", options.try_into_nix_value()?)];
        if let Some(ty) = freeform {
            fields.push(("freeformType", ty.0));
        }

        Ok(Self(
            Nixpkgs::new()
                .function("types.submodule")
                .call(NixValue::record(fields)),
        ))
    }
}

/// Declare a configurable NixOS field and describe its public interface.
///
/// A NixOS option has a type, documentation and possibly a default. Modules can
/// then assign values to that option, and NixOS checks and combines them. This
/// type describes the declaration; it does not set a current value or replace
/// Rust’s type system.
///
/// Place declarations in a structural Rust tree and pass it to
/// [`super::NixosModule::declare`]. Nested fields determine the option names.
/// Defaults and documentation metadata may include expressions Nix evaluates
/// later. Calling the same metadata setter twice replaces that property in
/// this declaration, not definitions supplied by other modules.
#[derive(Clone, Debug)]
pub struct OptionDecl {
    /// The expression for the NixOS option declaration: its type, default,
    /// description and other metadata, evaluated later by NixOS.
    value: NixValue,
}

impl OptionDecl {
    /// Start a declaration with a real NixOS type and no default.
    #[track_caller]
    pub fn new(ty: OptionType) -> Self {
        Self::from_value(
            Nixpkgs::new()
                .function("mkOption")
                .call(NixValue::record([("type", ty.0)])),
        )
    }

    /// Preserve an option generated by an existing Nix declaration helper.
    pub fn from_value(value: NixValue) -> Self {
        Self { value }
    }

    /// Declare a standard boolean enable option using NixOS `lib.mkEnableOption`.
    /// It supplies a false default, a true example and a description based on the
    /// feature name supplied here.
    #[track_caller]
    pub fn enable(description: &str) -> Self {
        Self::from_value(Nixpkgs::new().function("mkEnableOption").call(description))
    }

    #[track_caller]
    fn metadata(mut self, name: &str, value: NixValue) -> Self {
        self.value = Nixpkgs::new()
            .function("mergeAttrs")
            .apply([self.value, NixValue::record([(name, value)])]);
        self
    }

    /// Supply the option value used when no stronger definition is present.
    /// The default may be an expression evaluated later by Nix. Ordinary NixOS
    /// definitions can override it; Rust does not choose the winning value.
    #[track_caller]
    pub fn default(self, value: impl Into<NixValue>) -> Self {
        self.metadata("default", value.into())
    }

    /// Describe a default for NixOS documentation without assigning a default value.
    /// This is useful when another contribution computes the effective default.
    /// It is distinct from [`Self::default`], which supplies an actual value.
    #[track_caller]
    pub fn default_text(self, value: impl Into<NixValue>) -> Self {
        self.metadata("defaultText", value.into())
    }

    /// Attach a sample option value for NixOS documentation.
    /// The example does not configure the option. NixOS `lib.literalExpression`
    /// values can display sample Nix code rather than an evaluated value.
    #[track_caller]
    pub fn example(self, value: impl Into<NixValue>) -> Self {
        self.metadata("example", value.into())
    }

    /// Describe the public option using NixOS's markdown documentation conventions.
    #[track_caller]
    pub fn description(self, value: impl Into<NixValue>) -> Self {
        self.metadata("description", value.into())
    }

    /// Mark this option as internal to the module interface.
    #[track_caller]
    pub fn internal(self) -> Self {
        self.metadata("internal", true.into())
    }

    /// Let NixOS reject multiple definitions of this read-only option.
    #[track_caller]
    pub fn read_only(self) -> Self {
        self.metadata("readOnly", true.into())
    }
}

impl IntoRusixValue for OptionDecl {
    #[track_caller]
    fn into_value(self) -> RusixValue {
        self.value.into_value()
    }
}
