//! Expose application recipes to Nix consumers while Rust components use them directly.

use airlock_build::{Application, Workspace};
use rusix::{
    Config,
    interop::{
        NixPath,
        raw::{NixRepresentation, NixValue},
    },
};

/// Export the supported native builds with one explicit workspace and target package set.
pub(crate) fn configuration() -> Config {
    let packages = NixValue::function_attrs_open(["pkgs", "apps"], |arguments| {
        let pkgs = arguments.clone().select("pkgs");
        let workspace = Workspace::new(NixPath::from_expression(arguments.select("apps")));
        let mut packages = [
            ("cli", Application::Cli),
            ("diagnostics", Application::Diagnostics),
            ("development", Application::Development),
            ("dom0", Application::Dom0),
            ("controllerService", Application::ControllerService),
            ("controllerGui", Application::ControllerGui),
            ("vmService", Application::VmService),
            ("guiSetup", Application::GuiSetup),
            ("image", Application::Image),
            ("usb", Application::Usb),
            ("prepareRuntime", Application::PrepareRuntime),
        ]
        .map(|(name, application)| (name, application.build(&pkgs, &workspace).as_expression()))
        .to_vec();
        packages.push((
            "qubesPlugins",
            airlock_build::adapters::qubes(&pkgs, &workspace).as_expression(),
        ));

        (Vec::<(&str, NixValue)>::new(), NixValue::record(packages))
    });

    let tools = NixValue::function_attrs_open(["pkgs", "apps", "machine"], |arguments| {
        let pkgs = arguments.clone().select("pkgs");
        let workspace = Workspace::new(NixPath::from_expression(arguments.clone().select("apps")));
        let machine = NixPath::from_expression(arguments.select("machine"));
        let tools = airlock_build::tools::Tool::packages(&pkgs, &workspace, &machine);

        (Vec::<(&str, NixValue)>::new(), tools)
    });

    let upstream = NixValue::function(|pkgs| {
        rusix::nix_record! {
            "codex": airlock_build::upstream::codex(&pkgs),
            "inputProxy": airlock_build::upstream::input_proxy(&pkgs),
            "observation": airlock_build::upstream::observation(&pkgs),
        }
    });
    let overlays = NixValue::function(|source| {
        let machine = NixPath::from_source_text(source.to_text().into_expr::<String>());
        rusix::nix_record! {
            "codex": airlock_build::overlays::codex(),
            "guiWindowState": airlock_build::overlays::gui_window_state(&machine),
            "memoryAccounting": airlock_build::overlays::memory_accounting(&machine),
            "templateTools": airlock_build::overlays::template_tools(),
            "imageTools": airlock_build::overlays::image_tools(&machine),
        }
    });

    Config::new()
        .set_dynamic("packages", packages)
        .set_dynamic("images", crate::image::constructors())
        .set_dynamic("tools", tools)
        .set_dynamic("upstream", upstream)
        .set_dynamic("overlays", overlays)
}
