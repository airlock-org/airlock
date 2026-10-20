//! Synthetic hardware and firmware drive the actual GPU CLI without accessing host devices.

use crate::common::{self, peers::Peers};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
};

/// PCI identity deliberately differs from the physical lab's address.
const BDF: &str = "0000:09:03.0";

/// Snapshot of a single required Qubes-resolved GPU assignment.
pub(super) fn vm() -> Value {
    json!({"name":"graphics","uuid":"a7e27839-b68f-4c21-8b91-6861db3ec112","virt_mode":"hvm",
        "features":{"video_model":"none","uefi":false,"linux_stubdom":true,"qemu_extra_args":""},
        "assignments":[{"required":true,"backend_qid":0,"devices":[{"host_bdf":BDF,"port_id":"arbitrary-topology"}]}]})
}

/// Issue one native hook request against a disposable sysfs root.
fn hook(peers: &Peers, request: Value) -> Result<airlock_vm_service::support::Output> {
    let mut command = peers.command(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))?;
    command
        .args(["gpu", "qubes"])
        .env("AIRLOCK_GPU_PCI", peers.path("pci"))
        .env("AIRLOCK_AMD_VBIOS_CONFIG", peers.path("amd-vbios.json"));
    common::run(&mut command, Some(serde_json::to_vec(&request)?))
}

/// Create a valid display-controller identity already reserved to Xen's PCI backend.
pub(super) fn hardware(peers: &Peers) -> Result<PathBuf> {
    let device = peers.path("pci").join(BDF);
    fs::create_dir_all(&device)?;
    for (name, value) in [
        ("vendor", "0x1002"),
        ("device", "0x1681"),
        ("class", "0x030000"),
    ] {
        fs::write(device.join(name), value)?;
    }
    fs::create_dir_all(peers.path("drivers/pciback"))?;
    symlink("../../drivers/pciback", device.join("driver"))?;
    Ok(device)
}

/// Settings are optional, but enabling firmware disallows hotplug while the VM is active.
#[test]
fn configured_firmware_slots_gate_hotplug() -> Result<()> {
    let peers = Peers::new()?;
    let request = json!({"operation":"settings","name":"sys-gui"});
    assert_eq!(
        serde_json::from_slice::<Value>(&hook(&peers, request.clone())?.checked()?)?,
        Value::Null
    );
    fs::write(
        peers.path("amd-vbios.json"),
        serde_json::to_vec(&json!({"domains":{"sys-gui":{"guestSlot":16}}}))?,
    )?;
    assert_eq!(
        serde_json::from_slice::<Value>(&hook(&peers, request.clone())?.checked()?)?,
        json!({"guestSlot":16})
    );
    for (name, running, accepted) in [
        ("sys-gui", false, true),
        ("usr-home", true, true),
        ("sys-gui", true, false),
    ] {
        assert_eq!(
            hook(
                &peers,
                json!({"operation":"hotplug","name":name,"running":running})
            )?
            .status
            .success(),
            accepted
        );
    }
    fs::write(
        peers.path("amd-vbios.json"),
        r#"{"domains":{"sys-gui":{"guestSlot":1}}}"#,
    )?;
    assert!(!hook(&peers, request)?.status.success());
    Ok(())
}

/// Validation follows upstream assignments and rejects ambiguous devices or unsupported VM layouts.
#[test]
fn gpu_validation_uses_resolved_assignment_and_supported_layout() -> Result<()> {
    let peers = Peers::new()?;
    let device = hardware(&peers)?;
    let request = json!({"operation":"validate","vm":vm(),"slot":16});
    let selected: Value = serde_json::from_slice(&hook(&peers, request.clone())?.checked()?)?;
    assert_eq!(selected["host_bdf"], BDF);
    assert_eq!(selected["port_id"], "arbitrary-topology");

    for change in [
        "missing",
        "optional",
        "multiple",
        "backend",
        "unavailable",
        "segment",
        "pvh",
        "video",
        "uefi",
        "stubdom",
        "qemu",
    ] {
        let mut request = request.clone();
        match change {
            "missing" => request["vm"]["assignments"] = json!([]),
            "optional" => request["vm"]["assignments"][0]["required"] = json!(false),
            "multiple" => {
                request["vm"]["assignments"] =
                    json!([vm()["assignments"][0], vm()["assignments"][0]])
            }
            "backend" => request["vm"]["assignments"][0]["backend_qid"] = json!(4),
            "unavailable" => request["vm"]["assignments"][0]["devices"] = json!([{}]),
            "segment" => {
                request["vm"]["assignments"][0]["devices"][0]["host_bdf"] = json!("0001:09:03.0")
            }
            "pvh" => request["vm"]["virt_mode"] = json!("pvh"),
            "video" => request["vm"]["features"]["video_model"] = json!("vga"),
            "uefi" => request["vm"]["features"]["uefi"] = json!(true),
            "stubdom" => request["vm"]["features"]["linux_stubdom"] = json!(false),
            "qemu" => request["vm"]["features"]["qemu_extra_args"] = json!("-device something"),
            _ => unreachable!(),
        }
        assert!(!hook(&peers, request)?.status.success(), "{change}");
    }
    for slot in [json!(0), json!(7), json!(32), json!(true), json!("16")] {
        let mut request = request.clone();
        request["slot"] = slot;
        assert!(!hook(&peers, request)?.status.success());
    }
    for (field, value, original) in [
        ("vendor", "0x10de", "0x1002"),
        ("class", "0x040300", "0x030000"),
    ] {
        fs::write(device.join(field), value)?;
        assert!(!hook(&peers, request.clone())?.status.success());
        fs::write(device.join(field), original)?;
    }
    fs::remove_file(device.join("driver"))?;
    fs::create_dir_all(peers.path("drivers/amdgpu"))?;
    symlink(peers.path("drivers/amdgpu"), device.join("driver"))?;
    assert!(!hook(&peers, request)?.status.success());
    assert_eq!(
        fs::canonicalize(device.join("driver"))?
            .file_name()
            .unwrap(),
        "amdgpu"
    );
    Ok(())
}

/// A FIFO, directory or redirected firmware file must fail without blocking or opening hardware.
#[test]
fn offline_firmware_rejects_nonregular_inputs() -> Result<()> {
    let peers = Peers::new()?;
    let source = peers.path("VFCT");
    fs::create_dir(&source)?;
    for fifo in [false, true] {
        if fifo {
            fs::remove_dir(&source)?;
            rustix::fs::mknodat(
                rustix::fs::CWD,
                &source,
                rustix::fs::FileType::Fifo,
                rustix::fs::Mode::RUSR,
                0,
            )?;
        }
        let output = common::run(
            Command::new(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))
                .args(["gpu", "firmware", "--input"])
                .arg(&source)
                .args([
                    "--sha256",
                    &"0".repeat(64),
                    "--host-bdf",
                    "0000:04:00.0",
                    "--guest-bdf",
                    "0000:00:10.0",
                    "--device",
                    "1681",
                ]),
            None,
        )?;
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("regular file"));
    }
    Ok(())
}

/// Find a single private operation lock without depending on its hashed directory spelling.
pub(super) fn operation_lock(directory: &Path, name: &str) -> Result<PathBuf> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_name() == name {
            return Ok(entry.path());
        }
        if entry.file_type()?.is_dir()
            && let Ok(path) = operation_lock(&entry.path(), name)
        {
            return Ok(path);
        }
    }
    anyhow::bail!("operation lock not found: {name}")
}

/// A contending local GPU command cannot replace the first attempt's failure evidence.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_framebuffer_contention_preserves_first_report() -> Result<()> {
    let peers = Peers::new()?;
    let mut command = peers.command(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))?;
    command
        .args(["gpu", "framebuffer", "--gpu", BDF, "--report"])
        .arg(peers.path("logs/handoff.json"))
        .env("AIRLOCK_GPU_RUNTIME", peers.path("run"))
        .env("AIRLOCK_GPU_SYSFS", peers.path("missing-sysfs"));
    assert!(
        !common::run(&mut common::namespace(&command, &peers.path(""))?, None)?
            .status
            .success()
    );
    let before = fs::read(peers.path("logs/handoff.json"))?;
    let lock = fs::File::open(operation_lock(
        &peers.path("run"),
        "operation-framebuffer.lock",
    )?)?;
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
    let result = common::run(&mut common::namespace(&command, &peers.path(""))?, None)?;
    ensure!(
        !result.status.success(),
        "contending handoff unexpectedly succeeded"
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("lock"));
    assert_eq!(fs::read(peers.path("logs/handoff.json"))?, before);
    Ok(())
}

/// A software renderer or failed API initialization is not evidence of hardware rendering or isolation.
#[test]
fn rendering_probe_distinguishes_hardware_initialization_from_stronger_claims() -> Result<()> {
    for (kind, code, accepted) in [
        ("CPU", 0, false),
        ("VIRTUAL_GPU", 0, false),
        ("DISCRETE_GPU", 0, true),
        ("INTEGRATED_GPU", 0, true),
        ("DISCRETE_GPU", 1, false),
        ("OTHER", 0, false),
    ] {
        let mut peers = Peers::new()?;
        peers.rules(
            "graphics-info",
            json!([{"stdout":format!("deviceType = PHYSICAL_DEVICE_TYPE_{kind}\n"),"exit":code}]),
        )?;
        fs::create_dir(peers.path("pci"))?;
        let mut command = peers.command(common::binary("AIRLOCK_VM_BIN", "airlock-vm-service"))?;
        command
            .args(["gpu-probe", "--require-hardware"])
            .env("AIRLOCK_VULKANINFO", peers.path("bin/graphics-info"))
            .env("AIRLOCK_EGLINFO", peers.path("bin/graphics-info"))
            .env("AIRLOCK_GPU_PCI", peers.path("pci"))
            .env("AIRLOCK_GPU_DRM", peers.path("dri"));
        let output = common::run(&mut command, None)?;
        assert_eq!(output.status.success(), accepted);
        let report: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["hardware_api_initialized"], accepted);
        for field in [
            "rendering_verified",
            "exclusive_ownership_verified",
            "cross_vm_graphics_verified",
        ] {
            assert_eq!(report[field], false, "{field}");
        }
    }
    let mut peers = Peers::new()?;
    peers.rules("graphics-info", json!([{}]))?;
    let display = peers.path("pci/0000:01:00.0");
    fs::create_dir_all(&display)?;
    for (field, value) in [
        ("class", "0x030000"),
        ("vendor", "0x10de"),
        ("device", "0x2204"),
    ] {
        fs::write(display.join(field), value)?;
    }
    symlink("/sys/bus/pci/drivers/nvidia", display.join("driver"))?;
    fs::create_dir(peers.path("pci/0000:01:00.1"))?;
    fs::write(peers.path("pci/0000:01:00.1/class"), "0x040300")?;
    let mut command = peers.command(common::binary("AIRLOCK_VM_BIN", "airlock-vm-service"))?;
    command
        .arg("gpu-probe")
        .env("AIRLOCK_VULKANINFO", peers.path("missing"))
        .env("AIRLOCK_EGLINFO", peers.path("bin/graphics-info"))
        .env("AIRLOCK_GPU_PCI", peers.path("pci"))
        .env("AIRLOCK_GPU_DRM", peers.path("dri"));
    let report = peers.json(&mut command)?;
    assert_eq!(report["hardware_api_initialized"], false);
    assert!(report["probes"]["vulkan"]["returncode"].is_null());
    assert!(report["probes"]["vulkan"]["error"].is_string());
    assert_eq!(report["pci_display_devices"].as_array().unwrap().len(), 1);
    assert_eq!(report["pci_display_devices"][0]["address"], "0000:01:00.0");
    assert_eq!(report["pci_display_devices"][0]["driver"], "nvidia");
    Ok(())
}

/// Preflight refuses absent evidence and a bus reset without function-level-reset selection.
#[test]
fn amd_preflight_rejects_missing_evidence_and_bus_only_reset() -> Result<()> {
    let peers = Peers::new()?;
    for bus in [false, true] {
        if bus {
            let device = peers.path("sys/bus/pci/devices/0000:04:00.0");
            fs::create_dir_all(&device)?;
            fs::write(device.join("vendor"), "0x1002\n")?;
            fs::write(device.join("device"), "0x1681\n")?;
            fs::write(device.join("reset_method"), "bus\n")?;
        }
        let output = common::run(
            Command::new(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))
                .args(["gpu", "preflight"])
                .env("AIRLOCK_GPU_SYSFS", peers.path("sys"))
                .env("AIRLOCK_GPU_PROC", peers.path("proc")),
            None,
        )?;
        assert!(!output.status.success());
        let report: Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(report["assignment_gate_passed"], false);
        let expected = if bus {
            "function-level-reset-not-selected-review-required"
        } else {
            "not-xen-dom0"
        };
        assert!(
            report["failures"]
                .as_array()
                .unwrap()
                .contains(&json!(expected))
        );
    }
    Ok(())
}
