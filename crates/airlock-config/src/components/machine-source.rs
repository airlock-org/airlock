//! Resolve admitted application and machine sources for shared configuration components.

use rusix::{
    interop::{
        NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nixos::ModuleContext,
};

/// Machine source supplied through NixOS specialArgs so imports can use it during collection.
pub(super) const ARGUMENT: &str = "airlockSupport";

/// Airlock repository paths used by components sharing a NixOS module context.
pub(super) trait MachineFiles {
    /// Refers to a machine file using a validated repository-relative path.
    fn file(&self, relative: &str) -> NixValue;
}

impl MachineFiles for ModuleContext {
    /// Resolves an Airlock asset without freezing a developer checkout into generated Nix.
    fn file(&self, relative: &str) -> NixValue {
        self.argument::<NixPath>(ARGUMENT)
            .join_relative(relative)
            .expect("implementation paths are static and relative")
            .as_expression()
    }
}

/// Resolves the admitted Cargo workspace containing Airlock's implementations and desktop assets.
pub(super) fn application_source(inputs: &ModuleContext) -> NixPath {
    NixPath::from_source_text(
        inputs
            .pkgs
            .clone()
            .select("airlockAppsSource")
            .to_text()
            .into_expr::<String>(),
    )
}

/// Supplies the GUI service with application and machine sources from this module's caller.
pub(super) fn service_sources(
    inputs: &ModuleContext,
) -> airlock_sys_gui_service::authoring::Sources {
    airlock_sys_gui_service::authoring::Sources::new(
        application_source(inputs),
        inputs.argument(ARGUMENT),
    )
}
