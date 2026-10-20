//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test window state overlay patches common agent and daemon.
pub(super) fn window_state(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(qubes_gui_window_state_window_state_2)
}

/// Build the qubes gui window state test window state overlay patches common agent and daemon
/// contribution using the caller supplied deferred values.
fn qubes_gui_window_state_window_state_body(
    pkgs: NixValue,
    f: NixValue,
    patch_names: NixValue,
) -> NixValue {
    NixValue::record([
        (
            "common",
            patch_names
                .clone()
                .call(pkgs.clone().select_segments(["qubes-gui-common"])),
        ),
        (
            "agent",
            patch_names
                .clone()
                .call(pkgs.clone().select_segments(["qubes-gui-agent-linux"])),
        ),
        (
            "daemon",
            patch_names
                .clone()
                .call(pkgs.clone().select_segments(["qubes-gui-daemon"])),
        ),
        (
            "preparedGuestAgent",
            patch_names.clone().call(f.clone().select_segments([
                "nixosConfigurations",
                "airlock-domain-usr-home",
                "pkgs",
                "qubes-gui-agent-linux",
            ])),
        ),
        (
            "preparedGuiDaemon",
            patch_names.clone().call(f.clone().select_segments([
                "nixosConfigurations",
                "airlock-domain-sys-gui-usb-gui-lab",
                "pkgs",
                "qubes-gui-daemon",
            ])),
        ),
    ])
}

/// Build the qubes gui window state test window state overlay patches common agent and daemon
/// contribution using the caller supplied deferred values.
fn qubes_gui_window_state_window_state_2(f: NixValue) -> NixValue {
    NixValue::function(|package| {
        NixValue::builtin("map")
            .call(NixValue::builtin("baseNameOf"))
            .call(package.clone().select_segments(["patches"]))
    })
    .bind(|patch_names| {
        NixValue::builtin("import")
            .call(f.clone().select_segments(["inputs", "nixpkgs"]))
            .call(NixValue::record([
                ("system", NixValue::from("x86_64-linux")),
                (
                    "overlays",
                    NixValue::list([
                        f.clone()
                            .select_segments(["inputs", "nixos-qubes", "overlays", "default"]),
                        f.clone()
                            .select_segments(["overlays", "airlockQubesGuiWindowState"]),
                    ]),
                ),
            ]))
            .bind(|pkgs| {
                qubes_gui_window_state_window_state_body(pkgs, f.clone(), patch_names.clone())
            })
    })
}
