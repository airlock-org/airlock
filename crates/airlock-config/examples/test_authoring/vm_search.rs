//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test packaged vm search matches gnome and is enabled.
pub(super) fn search_extension(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(|flake| {
            flake
                .clone()
                .select_segments(["nixosConfigurations", "airlock-domain-sys-gui-usb-lab"])
                .bind(|vm| {
                    NixValue::builtin("head")
                        .call(
                            NixValue::builtin("filter")
                                .call(NixValue::function(|p| {
                                    p.clone()
                                        .attr_or("pname", NixValue::from(""))
                                        .equals(NixValue::from("airlock-gnome-vm-search"))
                                }))
                                .call(vm.clone().select_segments([
                                    "config",
                                    "environment",
                                    "systemPackages",
                                ])),
                        )
                        .bind(|extension| vm_search_search_extension_body(extension, vm.clone()))
                })
        })
}

/// Build the vm search test packaged vm search matches gnome and is enabled contribution using
/// the caller supplied deferred values.
fn vm_search_search_extension_body(extension: NixValue, vm: NixValue) -> NixValue {
    NixValue::concat_text([
        extension.clone().to_text(),
        NixValue::from("/share/gnome-shell/extensions/airlock-vm-search@airlock.local"),
    ])
    .bind(|directory| {
        NixValue::record([
            (
                "overrides",
                vm.clone().select_segments([
                    "config",
                    "services",
                    "desktopManager",
                    "gnome",
                    "extraGSettingsOverrides",
                ]),
            ),
            (
                "shellVersion",
                vm.clone()
                    .select_segments(["pkgs", "gnome-shell", "version"]),
            ),
            (
                "metadata",
                NixValue::builtin("fromJSON").call(
                    NixValue::builtin("readFile")
                        .call(add(directory.clone(), NixValue::from("/metadata.json"))),
                ),
            ),
            (
                "files",
                NixValue::builtin("attrNames")
                    .call(NixValue::builtin("readDir").call(directory.clone())),
            ),
            (
                "source",
                NixValue::builtin("readFile")
                    .call(add(directory.clone(), NixValue::from("/extension.js"))),
            ),
        ])
    })
}
