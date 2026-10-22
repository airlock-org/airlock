//! Install the sender contract and GNOME completion integration owned by the GUI service.

use super::Deployment;
use crate::command::{
    CAPTURE_ENQUEUE, CAPTURE_OPEN, CAPTURE_SHOW, CaptureAction, DesktopCommand, SCREEN_CAPTURE,
};
use airlock_machine_core::services::CaptureSenderConfig;
use rusix::{
    installation::Application, interop::raw::NixValue, nix_record, nix_text, nixos::ModuleBody,
};

/// Installed sender contract shared by completion callbacks and login retries.
const CONFIG_PATH: &str = "/etc/airlock/capture-sender.json";

impl Deployment {
    /// Declares the sender files, GNOME callbacks and login retry for checked installation.
    /// The caller selects the target scope and collision policy through Rusix composition.
    pub fn capture_sender(&self, config: &CaptureSenderConfig) -> Application {
        let helper = self.executable();
        let patch_commands = nix_text!(
            r#"
                substituteInPlace {screenshots} {screencasts} \
                  --replace-fail '@captureHelper@' '{helper}' \
                  --replace-fail '@captureCommand@' '{command}' \
                  --replace-fail '@captureEnqueue@' '{enqueue}' \
                  --replace-fail '@captureConfig@' '{config}'
                substituteInPlace {screenshots} \
                  --replace-fail '@captureOpen@' '{open}' \
                  --replace-fail '@captureShow@' '{show}'
            "#,
            screenshots = "js/ui/screenshot.js",
            screencasts = "js/dbusServices/screencast/screencastService.js",
            helper = helper.clone(),
            command = SCREEN_CAPTURE,
            enqueue = CAPTURE_ENQUEUE,
            open = CAPTURE_OPEN,
            show = CAPTURE_SHOW,
            config = CONFIG_PATH,
        );

        let overlay = NixValue::function(|_final| {
            NixValue::function(|previous| {
                nix_record! {
                    "gnome-shell": previous.select("gnome-shell").override_attrs(
                        NixValue::function(|old| {
                            nix_record! {
                                "patches": NixValue::concat_lists([
                                    old.clone().attr_or("patches", NixValue::list([])),
                                    NixValue::list([self.sources.machine_file(
                                        "patches/gnome-shell/\
                                         0001-completed-capture-handoff.patch"
                                    )]),
                                ]),
                                "postPatch": NixValue::concat_text([
                                    old.attr_or("postPatch", ""),
                                    patch_commands.clone().into(),
                                ]),
                            }
                        })
                    ),
                }
            })
        });

        let retry = nix_text!(
            r#"
                [Desktop Entry]
                Type=Application
                Name=Deliver pending screen captures
                Exec={command}
                OnlyShowIn=GNOME;
                NoDisplay=true
            "#,
            command = self.desktop_command(&DesktopCommand::ScreenCapture {
                config: Some(CONFIG_PATH.into()),
                action: CaptureAction::Retry { target: None },
            }),
        );

        self.installation()
            .etc_text(
                CONFIG_PATH
                    .strip_prefix("/etc/")
                    .expect("sender config is under /etc"),
                serde_json::to_string(config).expect("capture identities serialize"),
            )
            .etc_text("xdg/autostart/airlock-capture-retry.desktop", retry)
            .module(ModuleBody::new(nix_record! {
                "nixpkgs": nix_record! { "overlays": NixValue::list([overlay]) },
            }))
    }
}
