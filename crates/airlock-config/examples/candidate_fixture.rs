//! Exercise candidate VM creation, host configuration and an explicit service restart end to end.

use airlock_config::{
    model::{GiB, Machine, Memory, MiB, Vm},
    runtime::{Activation, Candidate, ServiceAction},
    rusix::{
        nix_record, nix_text,
        nixos::{ModuleBody, NixosModule},
    },
};
use std::path::Path;

/// Emit the smoke candidate from inputs supplied by the integration harness.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings: serde_json::Value = serde_json::from_slice(&std::fs::read("smoke-inputs.json")?)?;
    let required = |key: &str| -> Result<String, Box<dyn std::error::Error>> {
        settings[key]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| format!("smoke input {key} must be a string").into())
    };
    let name = required("created")?;
    let message = required("message")?;
    let action = required("action")?;
    let peer = settings["peer"].as_str();
    let mut machine = Machine::default();

    let network = Vm::network("sys-net", GiB(2))?;
    machine.add(&network)?;

    let firewall = Vm::firewall("sys-firewall", &network)?;
    machine.add(&firewall)?;

    let home = Vm::personal("usr-home", &firewall, GiB(16))?;
    machine.add(&home)?;

    let gui = Vm::gui("sys-gui", &home)?;
    machine.add(&gui)?;

    let controller = Vm::controller("sys-ctrl", &firewall, GiB(8))?;
    controller.set_root_image(MiB(128));
    machine.add(&controller)?;

    let mut created_vms = Vec::new();
    for (name, role, message) in std::iter::once((
        name.as_str(),
        if peer.is_some() {
            "nix-closure-smoke"
        } else {
            "created-from-config"
        },
        message.as_str(),
    ))
    .chain(peer.map(|name| {
        (
            name,
            "nix-closure-peer",
            settings["peerMessage"].as_str().unwrap_or(&message),
        )
    })) {
        let created = Vm::isolated(name, GiB(2))?;
        created.set_root_image(MiB(128));
        created.set_feature("airlock-created-from-config", "1")?;
        created.set_feature("airlock-created-from-config-message", message)?;
        created.set_memory(Memory::Ballooned {
            initial: MiB(1024),
            maximum: MiB(2048),
        });
        created.set_vcpus(1);
        created.add_module(NixosModule::with_context(|_| {
            ModuleBody::new(nix_record! {
                "environment": nix_record! {
                    "etc": nix_record! {
                        "airlock/domain-role": nix_record! {
                            "text": format!("{role}\n"),
                        },
                        "airlock/applied-message": nix_record! {
                            "text": format!("{message}\n"),
                        },
                    },
                },
            })
        }));
        machine.add(&created)?;
        created_vms.push(created);
    }

    machine.set_host(&gui, &firewall, &controller);
    let mut candidate = Candidate::new(machine.finish()?)?;
    candidate.set_message(message.clone());
    if peer.is_some() {
        for vm in &created_vms {
            candidate.allow_host_call("airlock.NixStoreRuntimeCheck", vm)?;
        }

        return candidate.export(Path::new("."));
    }

    candidate.add_host_package("hello");
    candidate.add_host_file(
        "airlock/local-dom0-extra-module",
        format!("{message}-extra\n"),
    )?;
    candidate.add_host_file(
        "airlock/local-dom0-host-configuration",
        format!("{message}-configuration\n"),
    )?;
    candidate.add_host_module(NixosModule::with_context(|context| {
        ModuleBody::new(nix_record! {
            "systemd": nix_record! {
                "services": nix_record! {
                    "airlock-local-extra-module": nix_record! {
                        "description": "Airlock local extra module fixture",
                        "serviceConfig": nix_record! {
                            "Type": "oneshot",
                            "RemainAfterExit": true,
                        },
                        "script": nix_text!(
                            r#"
                {coreutils}/bin/install -d -m 0755 /var/lib/airlock/system
                printf '%s\n' {message} > /var/lib/airlock/system/local-extra-module-restarted
            "#,
                            coreutils = context.pkgs.select("coreutils"),
                            message = context.lib.select("escapeShellArg").call(message),
                        ),
                    },
                },
            },
        })
    }));
    match action.as_str() {
        "switch" => {
            candidate.set_activation(Activation::Switch);
            candidate.add_service_action(
                "airlock-local-extra-module",
                ServiceAction::Restart,
                "restart local extra module fixture",
            )?;
        }
        "boot" => candidate.set_activation(Activation::Boot),
        _ => return Err("smoke activation must be switch or boot".into()),
    }

    candidate.export(Path::new("."))
}
