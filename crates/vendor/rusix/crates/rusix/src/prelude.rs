//! Common interfaces for typed package and configuration authoring.
//!
//! Dynamic operations live in [`crate::interop::raw`]; IR construction lives in
//! [`crate::ir`]. Neither is included in this prelude.
//!
//! ```
//! use rusix::prelude::*;
//!
//! let format = NixCallable::from_function(|name: Expr<String>| {
//!     nix_text!("name={name}", name = name)
//! });
//!
//! #[derive(IntoConfig)]
//! struct Output {
//!     // Text produced by the described Nix function when this output is evaluated.
//!     message: Expr<String>,
//! }
//!
//! let output = Output { message: format.call("openssl") };
//! // Typed calls construct deferred Nix expressions; they do not evaluate them.
//! ```
pub use crate::interop::{
    FinalAttrs, IntoNixExpression, NixAttrs, NixCallable, NixExpression, NixLibrary, NixList,
    NixNullable, NixOverridable, NixPath, Nixpkgs, Overlay, Overridable, Package, PackageFunction,
    PackageRef, Platform, Stdenv, ToNixText,
};
pub use crate::nixos::{NixosModule, OptionRef};
pub use crate::{
    Config, ConfigValue, Expr, IntoConfig, IntoRusixValue, ValidationError, args, config,
    nix_record, nix_text, options,
};
