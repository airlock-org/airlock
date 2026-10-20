//! Compose workstation packages and USB installation artifacts from the machine flake.

use airlock_build::{Application, Workspace};
use rusix::{
    interop::{
        NixExpression, NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
};

/// Keep image-only inputs outside production machine authoring and the runtime control plane.
pub(super) fn outputs() -> NixValue {
    NixValue::function_attrs_open(
        ["self", "airlock-machine", "nixpkgs", "nixos-qubes"],
        |inputs| {
            let source = inputs.clone().select("self");
            let machine = inputs.clone().select("airlock-machine");
            let workspace = Workspace::new(
                NixPath::from_expression(source.clone())
                    .join_relative("crates")
                    .unwrap(),
            );
            let usb = usb(&inputs, &workspace);
            let outputs = usb.bind(|usb| {
                let packages = super::systems(|system| {
                    let pkgs = NixValue::builtin("import")
                        .call(inputs.clone().select("nixpkgs"))
                        .call(nix_record! {
                            "system": system.clone(),
                        });
                    pkgs.bind(|pkgs| {
                        let base = NixValue::builtin("removeAttrs")
                            .call(
                                NixValue::builtin("getAttr")
                                    .call(system)
                                    .call(machine.clone().select("packages")),
                            )
                            .call(NixValue::list([
                                "airlock-machine-release-usb-lab".into(),
                                "airlock-machine-release-usb-gui-lab".into(),
                            ]));
                        let image = usb.clone().select("config.system.build");
                        let packages = base.merge_attrs(nix_record! {
                            "airlock-sys-ctrl-gui": Application::ControllerGui.build(&pkgs, &workspace),
                            "airlock-dev": Application::Development.build(&pkgs, &workspace),
                            "airlock-image": Application::Image.build(&pkgs, &workspace),
                            "airlock-qubes-probe": Application::QubesProbe.build(&usb.clone().select("pkgs"), &workspace),
                            "airlock-qubes": airlock_build::adapters::qubes(&usb.clone().select("pkgs"), &workspace),
                            "airlock-prepare-runtime": crate::image::runtime_tool(
                                &usb.clone().select("pkgs"),
                                &workspace,
                            ),
                            "airlock-usb-builder-input-closure": image
                                .clone()
                                .select("usbPreparedGuestStore.builderInputClosure"),
                            "airlock-usb-builder-nix": image.clone()
                                .select("usbPreparedGuestStore.builderNix"),
                            "airlock-usb-dev-image": image.select("image"),
                            "airlock-usb-dom0-console-image": source.clone().select(
                                "nixosConfigurations.usb-dom0-console.config.system.build.image",
                            ),
                        });
                        let snapshot_path =
                            NixValue::builtin("getEnv").call("AIRLOCK_HOST_REPAIR_SNAPSHOT");
                        let repair = NixValue::builtin("fromJSON")
                            .call(NixValue::builtin("readFile").call(snapshot_path.clone()))
                            .bind(|snapshot| {
                                let apps = Workspace::new(NixPath::from_expression(
                                    machine.clone().select("inputs.airlock-apps"),
                                ));

                                crate::image::repair_constructor(&apps).call(nix_record! {
                                    "baseline": NixValue::builtin("getFlake")
                                        .call(snapshot.clone().select("baselineFlake")),
                                    "machine": machine.clone(),
                                    "repairRevision": snapshot.clone().select("repairRevision"),
                                    "snapshot": snapshot,
                                })
                            });

                        packages.merge_attrs(NixValue::if_else(
                            snapshot_path.equals(""),
                            nix_record! {},
                            nix_record! {
                                "airlock-machine-host-repair": repair,
                            },
                        ))
                    })
                });
                let configurations =
                    machine
                        .clone()
                        .select("nixosConfigurations")
                        .merge_attrs(nix_record! {
                            "usb-dev": usb,
                            "usb-dom0-console": inputs.clone()
                                .select("nixpkgs.lib.nixosSystem")
                                .call(nix_record! {
                                    "system": "x86_64-linux",
                                    "modules": NixValue::list([
                                        machine.clone().select("nixosModules.dom0"),
                                        crate::image::console_module(),
                                    ]),
                                }),
                        });

                nix_record! {
                    "lib": machine.clone().select("lib"),
                    "overlays": machine.clone().select("overlays"),
                    "nixosModules": machine.clone().select("nixosModules"),
                    "checks": machine.clone().select("checks"),
                    "airlock": machine.select("airlock"),
                    "nixosConfigurations": configurations,
                    "packages": packages,
                }
            });

            (Vec::<(&str, NixValue)>::new(), outputs)
        },
    )
}

/// Prepare shared VM objects once and substitute their descriptors into the physical machine.
fn usb(inputs: &NixValue, workspace: &Workspace) -> NixValue {
    let machine = inputs.clone().select("airlock-machine");
    let source = inputs.clone().select("self");
    let nixpkgs = inputs.clone().select("nixpkgs");
    let seed_path = NixValue::builtin("getEnv").call("AIRLOCK_MACHINE_SEED_ARCHIVE");
    let seed = NixValue::if_else(
        seed_path.clone().equals(""),
        NixValue::null(),
        NixValue::builtin("path").call(nix_record! {
            "path": seed_path,
            "name": "airlock-machine-seed.tar.gz",
        }),
    );
    let offline_path = NixValue::builtin("getEnv").call("AIRLOCK_MACHINE_OFFLINE_INPUTS");
    let offline = NixValue::if_else(
        offline_path.clone().equals(""),
        NixValue::null(),
        NixValue::builtin("fromJSON").call(NixValue::builtin("readFile").call(offline_path)),
    );
    let extra = nix_record! {
        "airlockMachineSeedArchive": seed.clone(),
        "airlockMachineOfflineInputs": offline.clone(),
    };
    let entries = NixValue::builtin("removeAttrs")
        .call(
            machine
                .clone()
                .select("lib.mkMachineGuestEntries")
                .call("x86_64-linux")
                .call(extra.clone()),
        )
        .call(NixValue::list([
            "sys-gui-hybrid".into(),
            "sys-gui-standalone".into(),
        ]));
    let pkgs = NixValue::builtin("import")
        .call(nixpkgs.clone())
        .call(nix_record! {
            "system": "x86_64-linux",
            "overlays": NixValue::list([
                inputs.clone().select("nixos-qubes.overlays.default"),
                machine.clone().select("overlays.airlockApps"),
                machine
                    .clone()
                    .select("overlays.airlockQubesGuiWindowState"),
                machine.clone().select("overlays.airlockImageBuildTools"),
            ]),
            "config": nix_record! {
                "allowUnfreePredicate": super::allow_unfree(&nixpkgs.select("lib")),
            },
        });

    entries.bind(|entries| {
        crate::image::prepared_store(&pkgs, workspace, entries.clone(), offline.clone()).bind(
            |prepared| {
                let stores = NixValue::builtin("mapAttrs")
                    .call(NixValue::function(|name| {
                        NixValue::function(|_| {
                            nix_text!(
                                "{descriptors}/{name}",
                                descriptors = prepared.clone().select("descriptors"),
                                name = name,
                            )
                            .into()
                        })
                    }))
                    .call(entries.clone());
                let machine_source = NixValue::if_else(
                    offline
                        .clone()
                        .equals(NixValue::null())
                        .equals(false)
                        .and(offline.clone().has_attr("revision")),
                    crate::topology::normalize::require(
                        offline.clone().select("lock_hash").equals(
                            NixValue::builtin("hashFile")
                                .call("sha256")
                                .call(nix_text!("{machine}/flake.lock", machine = machine.clone())),
                        ),
                        "Offline inputs must match the selected machine lock.",
                        nix_record! {
                            "revision": offline.clone().select("revision"),
                            "lockHash": offline.select("lock_hash"),
                        },
                    ),
                    NixValue::null(),
                );

                machine
                    .select("lib.mkMachineConfiguration")
                    .call(nix_record! {
                        "system": "x86_64-linux",
                        "storeSources": stores,
                        "includeRelease": false,
                        "includeAlternativeGuiProfiles": false,
                        "extraGuestSpecialArgs": extra,
                        "extraSpecialArgs": nix_record! {
                            "airlockUsbPreparedStore": prepared,
                            "airlockMachineSource": machine_source,
                            "airlockSourceRevision": super::source_identity(&source).select("revision"),
                            "airlockSourceTimestamp": super::source_identity(&source).select("timestamp"),
                            "airlockUsbGuestEntries": entries,
                            "airlockMachineSeedArchive": seed,
                        },
                        "extraModules": NixValue::list([crate::image::usb_module(workspace)]),
                    })
            },
        )
    })
}
