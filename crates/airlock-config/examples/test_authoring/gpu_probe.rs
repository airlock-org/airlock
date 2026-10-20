//! Rust-authored fixtures evaluated by the native integration suite.

use super::*;

/// Inspect evaluated configuration for test gpu lab is opt in and keeps xfce.
pub(super) fn gpu_lab(inputs: NixValue) -> NixValue {
    NixValue::builtin("getFlake")
        .call(NixValue::concat_text([inputs
            .clone()
            .select("root")
            .to_text()]))
        .bind(|flake| {
            flake
                .clone()
                .select_segments(["nixosConfigurations", "vm-gui-config-dev", "config"])
                .bind(|normal| {
                    flake
                        .clone()
                        .select_segments(["nixosConfigurations", "vm-gpu-lab", "config"])
                        .bind(|lab| {
                            flake
                                .clone()
                                .select_segments([
                                    "nixosConfigurations",
                                    "airlock-domain-sys-gpu",
                                    "config",
                                ])
                                .bind(|vm| gpu_probe_gpu_lab_body(vm, lab.clone(), normal.clone()))
                        })
                })
        })
}

/// Build the gpu probe test gpu lab is opt in and keeps xfce contribution using the caller
/// supplied deferred values.
fn gpu_probe_gpu_lab_body(vm: NixValue, lab: NixValue, normal: NixValue) -> NixValue {
    NixValue::record([
        (
            "normalHasGpu",
            normal
                .clone()
                .select_segments(["airlock", "qubes", "domains"])
                .has_attr("sys-gpu"),
        ),
        (
            "gpu",
            lab.clone()
                .select_segments(["airlock", "qubes", "topology", "domains", "sys-gpu"]),
        ),
        (
            "normalGui",
            normal
                .clone()
                .select_segments(["airlock", "qubes", "domains", "sys-gui"]),
        ),
        (
            "labGui",
            lab.clone()
                .select_segments(["airlock", "qubes", "domains", "sys-gui"]),
        ),
        (
            "normalSession",
            normal
                .clone()
                .select_segments(["services", "displayManager", "defaultSession"]),
        ),
        (
            "labSession",
            lab.clone()
                .select_segments(["services", "displayManager", "defaultSession"]),
        ),
        (
            "guiVm",
            lab.clone()
                .select_segments(["airlock", "qubes", "prefs", "default_guivm"]),
        ),
        (
            "hostName",
            vm.clone().select_segments(["networking", "hostName"]),
        ),
        (
            "graphics",
            vm.clone()
                .select_segments(["hardware", "graphics", "enable"]),
        ),
    ])
}
