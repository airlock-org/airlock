//! Expose standalone application builds and an explicitly entered desktop development environment.

use airlock_build::{Application, Workspace, adapters};
use rusix::{
    interop::{
        NixExpression, NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record,
};

/// Describe application outputs without importing generated package catalogues back into authoring.
pub(super) fn outputs() -> NixValue {
    NixValue::function_attrs(["self", "nixpkgs"], |inputs| {
        let workspace = Workspace::new(NixPath::from_expression(inputs.clone().select("self")));
        let packages = super::systems(|system| {
            NixValue::builtin("import")
                .call(inputs.clone().select("nixpkgs"))
                .call(nix_record! {
                    "system": system,
                })
                .bind(|pkgs| {
                    let mut packages = [
                        ("airlock-sys-ctrl-gui", Application::ControllerGui),
                        ("airlock-dom0-service", Application::Dom0),
                        ("airlock-dev", Application::Development),
                        ("airlock-image", Application::Image),
                        ("airlock-cli", Application::Cli),
                        ("airlock-diagnostics", Application::Diagnostics),
                        ("airlock-sys-ctrl-service", Application::ControllerService),
                        ("airlock-vm-service", Application::VmService),
                        ("airlock-vm-init", Application::VmInit),
                        ("airlock-sys-gui-setup", Application::GuiSetup),
                    ]
                    .map(|(name, application)| {
                        (name, application.build(&pkgs, &workspace).as_expression())
                    })
                    .to_vec();
                    packages.push((
                        "airlock-qubes",
                        adapters::qubes(&pkgs, &workspace).as_expression(),
                    ));
                    packages.push((
                        "airlock-sys-gui-service",
                        airlock_sys_gui_service::authoring::native(&pkgs, &workspace.root())
                            .as_expression(),
                    ));
                    packages.push((
                        "airlock-authoring-dependencies",
                        crate::vm::authoring::dependencies(&pkgs, &workspace.root()),
                    ));
                    packages.push((
                        "default",
                        Application::ControllerGui
                            .build(&pkgs, &workspace)
                            .as_expression(),
                    ));

                    NixValue::record(packages)
                })
        });
        let shells = super::systems(|system| {
            NixValue::builtin("import")
                .call(inputs.select("nixpkgs"))
                .call(nix_record! {
                    "system": system,
                })
                .bind(|pkgs| nix_record! {
                    "verification": super::verification::shell(&pkgs),
                    "desktop": pkgs.clone().select("mkShell").call(nix_record! {
                        "nativeBuildInputs": NixValue::list([pkgs.clone().select("pkg-config")]),
                        "PYO3_PYTHON": rusix::nix_text!(
                            "{python}/bin/python3",
                            python = pkgs.clone().select("python3"),
                        ),
                        "buildInputs": NixValue::list([
                            pkgs.clone().select("gtk4"),
                            pkgs.clone().select("libadwaita"),
                            pkgs.clone().select("python3"),
                            pkgs.select("udev"),
                        ]),
                    }),
                })
        });

        (
            Vec::<(&str, NixValue)>::new(),
            nix_record! {
                "packages": packages,
                "devShells": shells,
            },
        )
    })
}
