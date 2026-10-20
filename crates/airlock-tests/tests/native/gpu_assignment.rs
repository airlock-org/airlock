//! A declared GPU is selected from authoritative Qubes inventory without taking another VM's ownership.

use crate::common::{self, peers::Peers};
use anyhow::Result;
use serde_json::{Value, json};
use std::{fs, os::unix::fs::symlink};

const BDF: &str = "0000:09:03.0";

const PORT: &str = "00_03.1-00_00.0";

/// One persistent Qubes device identity, distinct from its physical PCI address.
fn assignment() -> Value {
    json!({"backend":"dom0","port":PORT,"mode":"required","device_id":"*","options":{}})
}

/// A dom0 or VM inventory record with separately observed assignments and live attachments.
fn domain(name: &str, running: bool, assignments: Value, attached: Value) -> Value {
    json!({"name":name,"running":running,"assignments":assignments,"attached":attached})
}

/// Native selection inputs and a disposable host controller reserved to pciback.
struct Selection {
    // Owns all sysfs attributes and subprocess environments for this scenario.
    peers: Peers,
    // Reviewed physical device and topology-port identity.
    selection: Value,
    // Desired persistent PCI assignment for sys-gui.
    topology: Value,
    // Authoritative upstream inventory, including other VMs' ownership.
    inventory: Value,
}

impl Selection {
    /// Use a different vendor and location from the physical lab so no default hardware can satisfy checks.
    fn new() -> Result<Self> {
        let peers = Peers::new()?;
        let device = peers.path("pci").join(BDF);
        fs::create_dir_all(&device)?;
        for (field, value) in [
            ("vendor", "0x8086"),
            ("device", "0x1234"),
            ("class", "0x030000"),
        ] {
            fs::write(device.join(field), value)?;
        }
        fs::create_dir_all(peers.path("drivers/pciback"))?;
        symlink("../../drivers/pciback", device.join("driver"))?;
        Ok(Self {
            peers,
            selection: json!({"bdf":BDF,"port":PORT,"vendor":"8086","device":"1234"}),
            topology: json!({"domains":{"sys-gui":{"prefs":{"virt_mode":"hvm","autostart":false},"features":{"video-model":"none"},
                "devices":{"pci":[{"backend":"dom0","id":PORT,"mode":"required","match":"port","options":{}}]}}}}),
            inventory: json!({"resolved":BDF,"domains":[domain("dom0",false,json!([]),json!([]))]}),
        })
    }

    /// Validate one phase through the production JSON boundary without changing topology or ownership.
    fn run(&self, phase: &str) -> Result<airlock_vm_service::support::Output> {
        let mut command = self
            .peers
            .command(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))?;
        command
            .args(["gpu", "qubes"])
            .env("AIRLOCK_GPU_PCI", self.peers.path("pci"));
        common::run(
            &mut command,
            Some(serde_json::to_vec(
                &json!({"operation":"check","selection":self.selection,
            "topology":self.topology,"inventory":self.inventory,"phase":phase}),
            )?),
        )
    }

    /// Require one explicit policy refusal while preserving its actionable native diagnostic.
    fn rejects(&self, phase: &str, reason: &str) -> Result<()> {
        let result = self.run(phase)?;
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(reason),
            "{reason}: {stderr}",
            stderr = String::from_utf8_lossy(&result.stderr)
        );
        Ok(())
    }
}

/// Fresh bootstrap can precede VM creation, while startup requires the exact persistent assignment.
#[test]
fn gpu_bootstrap_and_start_have_distinct_persistence_requirements() -> Result<()> {
    let mut selected = Selection::new()?;
    let report: Value = serde_json::from_slice(&selected.run("bootstrap")?.checked()?)?;
    assert_eq!(report["persistent_assignment"], false);
    selected.rejects("start", "bootstrap has not persisted")?;
    for running in [false, true] {
        selected.inventory["domains"] = json!([
            domain("dom0", false, json!([]), json!([])),
            domain("sys-gui", running, json!([assignment()]), json!([]))
        ]);
        let report: Value = serde_json::from_slice(&selected.run("start")?.checked()?)?;
        assert_eq!(report["persistent_assignment"], true);
    }
    Ok(())
}

/// Physical identity, reservation, Qubes resolution and desired topology must all agree.
#[test]
fn gpu_selection_rejects_hardware_and_topology_mismatches() -> Result<()> {
    for (field, value, reason) in [
        ("vendor", "0x1002", "vendor mismatch"),
        ("device", "0x9999", "device mismatch"),
        ("class", "0x020000", "not a display"),
    ] {
        let selected = Selection::new()?;
        fs::write(selected.peers.path("pci").join(BDF).join(field), value)?;
        selected.rejects("bootstrap", reason)?;
    }
    let mut missing = Selection::new()?;
    missing.selection["bdf"] = json!("0000:08:03.0");
    missing.rejects("bootstrap", "is missing")?;
    let selected = Selection::new()?;
    let driver = selected.peers.path("pci").join(BDF).join("driver");
    fs::remove_file(&driver)?;
    fs::create_dir(selected.peers.path("drivers/i915"))?;
    symlink("../../drivers/i915", &driver)?;
    selected.rejects("bootstrap", "not reserved")?;
    let mut resolved = Selection::new()?;
    resolved.inventory["resolved"] = json!("0000:08:00.0");
    resolved.rejects("bootstrap", "does not resolve")?;
    for (field, value) in [
        ("id", json!("00_01.0")),
        ("mode", json!("auto-attach")),
        ("options", json!({"no-strict-reset":true})),
    ] {
        let mut selected = Selection::new()?;
        selected.topology["domains"]["sys-gui"]["devices"]["pci"][0][field] = value;
        selected.rejects("bootstrap", "topology does not contain")?;
    }
    Ok(())
}

/// Stale assignments, live experiments and another VM's ownership are refused without replacement.
#[test]
fn gpu_selection_cannot_take_or_replace_existing_ownership() -> Result<()> {
    for change in [
        json!({"port":"00_02.0"}),
        json!({"device_id":"specific-device"}),
        json!({"mode":"auto-attach"}),
        json!({"options":{"permissive":"true"}}),
        json!({"backend":"other"}),
    ] {
        let mut selected = Selection::new()?;
        let mut item = assignment();
        item.as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        selected.inventory["domains"] = json!([
            domain("dom0", false, json!([]), json!([])),
            domain("sys-gui", false, json!([item]), json!([]))
        ]);
        selected.rejects("bootstrap", "conflicting PCI assignment")?;
    }
    for kind in ["assignments", "attached"] {
        let mut selected = Selection::new()?;
        let mut other = domain("other", false, json!([]), json!([]));
        other[kind] = json!([assignment()]);
        selected.inventory["domains"]
            .as_array_mut()
            .unwrap()
            .push(other);
        selected.rejects("bootstrap", "already assigned/attached to other")?;
    }
    let mut selected = Selection::new()?;
    selected.inventory["domains"]
        .as_array_mut()
        .unwrap()
        .push(domain("sys-gui", true, json!([]), json!([assignment()])));
    selected.rejects(
        "bootstrap",
        "running without the declared persistent assignment",
    )?;
    let mut extra = assignment();
    extra["port"] = json!("00_01.0");
    selected.inventory["domains"][1] =
        domain("sys-gui", true, json!([assignment()]), json!([extra]));
    selected.rejects("bootstrap", "conflicting attached")?;
    Ok(())
}

/// Disabling physical ownership avoids GPU resolution but still rejects stale PCI state in another profile.
#[test]
fn disabled_gpu_profile_needs_inventory_without_resolving_hardware() -> Result<()> {
    let mut selected = Selection::new()?;
    selected.selection["enable"] = json!(false);
    selected.selection["bdf"] = json!("0000:ff:00.0");
    let report: Value = serde_json::from_slice(&selected.run("bootstrap")?.checked()?)?;
    assert_eq!(report["enabled"], false);
    selected.rejects("start", "ownership is disabled")?;
    for kind in ["assignments", "attached"] {
        let mut vm = domain("sys-gui", false, json!([]), json!([]));
        vm[kind] = json!([assignment()]);
        selected.inventory["domains"] = json!([vm]);
        selected.rejects("bootstrap", "PCI state remains from another profile")?;
    }
    Ok(())
}
