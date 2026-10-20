//! Rust values returned by the overlay and its build-attribute update.
//! Each struct becomes a Nix attribute set using the usual Rusix field naming.
use rusix::{
    Expr, IntoConfig, IntoRusixValue,
    interop::{NixList, Overlay, Package},
};

/// Package definitions replaced by the overlay; other packages stay unchanged.
#[derive(IntoRusixValue)]
pub struct Changes {
    /// Curl with the DICT protocol disabled.
    pub curl: Package,
}

/// Builder attributes replaced by overrideAttrs; other recipe fields stay unchanged.
#[derive(IntoRusixValue)]
pub struct ConfigureChanges {
    /// Existing configure flags followed by the flag that disables DICT.
    pub configure_flags: NixList<Expr<String>>,
}

/// Generated Nix containing the overlay and a lookup from the customized package set.
#[derive(IntoConfig)]
pub struct Output {
    /// The deferred overlay function itself.
    pub overlay: Overlay,
    /// Path of the customized curl build recipe, selected without building it.
    pub curl_derivation: Expr<String>,
}
