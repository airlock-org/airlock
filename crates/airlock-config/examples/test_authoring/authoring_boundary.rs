//! Export authoring scenarios for Nix integration through an explicitly prepared Cargo executable.

use airlock_config::{
    components::{desktop, screen_capture},
    model::{GiB, Machine, Vm},
};
use rusix::interop::raw::{NixRepresentation, NixValue};
use serde_json::Value;
use std::path::Path;

/// Export a complete snapshot whose variation is selected by data, never executable source.
pub(super) fn export(settings: &Value) -> Result<NixValue, String> {
    export_snapshot(settings).map_err(|error| error.to_string())?;
    Ok(NixValue::null())
}

/// Keep the same connected baseline across the bootstrap, diagnostic and capture checks.
fn export_snapshot(settings: &Value) -> Result<(), Box<dyn std::error::Error>> {
    let output = Path::new(
        settings["output"]
            .as_str()
            .ok_or("expected output directory")?,
    );
    let variant = settings["variant"]
        .as_str()
        .ok_or("expected export variant")?;

    let mut machine = Machine::default();
    let network = Vm::network("sys-net", GiB(2))?;
    machine.add(&network)?;
    let firewall = Vm::firewall("sys-firewall", &network)?;
    machine.add(&firewall)?;
    let home = Vm::personal("usr-home", &firewall, GiB(16))?;
    machine.add(&home)?;
    let gui = Vm::gui("sys-gui", &home)?;
    machine.add(&gui)?;
    let controller = Vm::controller("sys-ctrl", &firewall, GiB(32))?;
    machine.add(&controller)?;
    machine.set_host(&gui, &firewall, &controller);

    match variant {
        "bootstrap" => {
            controller.add_module(rusix::nixos::NixosModule::new(
                rusix::Config::new()
                    .set_dynamic("environment.sessionVariables.AIRLOCK_EXPORT", "present"),
            ));
            airlock_config::export_machine(
                &airlock_config::profiles::usb_lab::machine(false)?,
                &machine.finish()?,
                "test-revision",
                output,
            )?;
            return Ok(());
        }
        "source-conflict" => {
            let assistant = Vm::assistant("usr-assistant", &firewall, GiB(16))?;
            assistant.add_module(rusix::nixos::NixosModule::new(
                rusix::Config::new()
                    .set_dynamic("environment.sessionVariables.AIRLOCK_WORKSPACE", "fixture"),
            ));
            machine.add(&assistant)?;
        }
        "capture-renamed" => {
            gui.add_module(desktop::gnome());
            let target = Vm::personal("personal-files", &firewall, GiB(8))?;
            machine.add(&target)?;
            let display = Vm::gui("capture-display", &target)?;
            display.add_module(desktop::gnome());
            target.set_gui(&display);
            screen_capture::connect(&display, &target);
            machine.add(&display)?;
        }
        "capture-none" => gui.add_module(desktop::gnome()),
        "capture-shared" => {
            gui.add_module(desktop::gnome());
            screen_capture::connect(&gui, &home);
            let display = Vm::gui("capture-display", &home)?;
            display.add_module(desktop::gnome());
            screen_capture::connect(&display, &home);
            machine.add(&display)?;
        }
        _ => return Err("unknown export variant".into()),
    }

    airlock_config::export_snapshot(&machine.finish()?, output)?;
    Ok(())
}

/// Render command arguments and capture hooks through the real authoring and module APIs.
pub(super) fn commands(settings: &Value) -> Result<NixValue, String> {
    use airlock_machine_core::services::CaptureSenderConfig;
    use airlock_sys_gui_service::{
        authoring::{self, Sources},
        command::{CaptureAction, DesktopCommand},
    };
    use rusix::{
        installation::{self, CollisionChecks},
        interop::NixPath,
        nix_record,
        nixos::ContextualModule,
    };

    let package: NixValue = settings["package"]
        .as_str()
        .ok_or("expected package")?
        .into();
    let library: NixValue = settings["library"]
        .as_str()
        .ok_or("expected library")?
        .into();
    let argument = settings["argument"]
        .as_str()
        .ok_or("expected literal argument")?;

    let lib = NixValue::builtin("import").call(library);
    let pkgs = nix_record! {
        "writeShellApplication": NixValue::function(|_| nix_record! { "outPath": package }),
        "lib": lib.clone(),
    };
    let source = NixPath::from_expression("/unneeded-source".into());
    let service = authoring::deployment(&pkgs, &Sources::new(source.clone(), source));
    let command = DesktopCommand::ScreenCapture {
        config: Some(argument.into()),
        action: CaptureAction::CheckConfig,
    };

    let sender = installation::compose(
        "capture-hook-test",
        CollisionChecks::Enabled,
        [service.capture_sender(&CaptureSenderConfig::default())],
    )
    .map_err(|error| error.to_string())?;
    let module = ContextualModule::new(|_| sender).into_expression();
    let configured = lib.clone().select("evalModules").call(nix_record! {
        "modules": NixValue::list([
            module,
            nix_record! {
                "_module": nix_record! {
                    "freeformType": lib.select("types.attrsOf").call(
                        pkgs.clone().select("lib.types.anything")
                    ),
                },
            },
        ]),
    });
    let overlay = NixValue::builtin("head").call(configured.select("config.nixpkgs.overlays"));
    // Deliberately omit the final newline to exercise the boundary between build hooks.
    let previous = nix_record! {
        "gnome-shell": nix_record! {
            "overrideAttrs": NixValue::function(|update| update.call(nix_record! {
                "patches": NixValue::list(["upstream.patch".into()]),
                "postPatch": "printf '%s\\n' upstream",
            })),
        },
    };
    let capture = overlay.call(pkgs).call(previous).select("gnome-shell");

    Ok(nix_record! {
        "shell": service.shell_command(&command),
        "desktop": service.desktop_command(&command),
        "capture": capture,
    })
}
