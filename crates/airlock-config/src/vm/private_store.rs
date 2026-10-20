//! Route Nix commands to writable private stores without weakening build-worker admission.

use super::WorkerRole;
use airlock_build::{Application, Workspace};
use rusix::{
    interop::{
        NixPath, PackageFunction,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
};

/// Export the private-store package with explicit settings and target packages.
pub(super) fn constructor() -> PackageFunction {
    PackageFunction::from_function_attrs(["lib", "pkgs", "settings"], |args| {
        let package = package(
            &args.clone().select("lib"),
            &args.clone().select("pkgs"),
            &args.select("settings"),
        );

        (Vec::<(&str, NixValue)>::new(), package)
    })
}

/// Wrap Nix so build workers require an admitted pool view unless the caller selects a store explicitly.
pub(super) fn package(lib: &NixValue, pkgs: &NixValue, settings: &NixValue) -> NixValue {
    let role = settings.clone().select("role");
    let root = settings.clone().select("configNixStoreRoot");
    let selected = NixValue::builtin("elem")
        .call(role.clone())
        .call(NixValue::list([
            WorkerRole::Development.identity().into(),
            WorkerRole::Controller.identity().into(),
        ]));
    let native_workspace = Workspace::new(NixPath::from_source_text(
        pkgs.clone()
            .select("airlockAppsSource")
            .to_text()
            .into_expr::<String>(),
    ));
    let native = Application::VmService
        .build(pkgs, &native_workspace)
        .as_expression();
    let runtime_path = lib
        .clone()
        .select("makeBinPath")
        .call(NixValue::list([pkgs.clone().select("util-linux")]));
    let wrapper = airlock_build::native::writer(pkgs, false)
        .call("airlock-config-nix")
        .call(crate::commands::contract(
            pkgs,
            "private-store",
            [
                ("service", native),
                ("path", runtime_path),
                ("nix", pkgs.clone().select("nix")),
                ("storeRoot", root),
                ("pinned", NixValue::if_else(selected, "--pinned", "")),
            ],
        ));

    pkgs.clone()
        .select("runCommand")
        .call("airlock-config-nix")
        .call(nix_record! {})
        .call(nix_text!(
            r#"
            mkdir -p "$out/bin"
            ln -s {wrapper} "$out/bin/airlock-config-nix"
            ln -s {wrapper} "$out/bin/nix"
        "#,
            wrapper = wrapper,
        ))
}
