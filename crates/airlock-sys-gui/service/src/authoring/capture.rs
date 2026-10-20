//! Install the sender contract and GNOME completion integration owned by the GUI service.

use super::Deployment;
use crate::command::{CaptureAction, DesktopCommand};
use airlock_machine_core::services::CaptureSenderConfig;
use rusix::{interop::raw::NixValue, nix_record, nix_text};

/// Installed sender contract shared by completion callbacks and login retries.
const CONFIG_PATH: &str = "/etc/airlock/capture-sender.json";

impl Deployment {
    /// Installs the final contract, patches GNOME completion callbacks and retries once at login.
    pub fn capture_sender(&self, config: &CaptureSenderConfig) -> NixValue {
        let helper = self.executable();
        let patch_commands = nix_text!(
            r#"
                substituteInPlace {screenshots} {screencasts} \
                  --replace-fail '@captureHelper@' '{helper}' \
                  --replace-fail '@captureConfig@' '{config}'
            "#,
            screenshots = "js/ui/screenshot.js",
            screencasts = "js/dbusServices/screencast/screencastService.js",
            helper = helper.clone(),
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
            command = self.desktop_command(&DesktopCommand::Capture {
                config: Some(CONFIG_PATH.into()),
                action: CaptureAction::Retry { target: None },
            }),
        );

        nix_record! {
            "nixpkgs": nix_record! { "overlays": NixValue::list([overlay]) },
            "environment": nix_record! { "etc": nix_record! {
                "airlock/capture-sender.json": nix_record! {
                    "text": serde_json::to_string(config).expect("capture identities serialize"),
                },
                "xdg/autostart/airlock-capture-retry.desktop": nix_record! { "text": retry },
            } },
        }
    }
}
