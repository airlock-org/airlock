//! Startup firmware stays bound to one Qubes assignment and one protected artifact through XML conversion.

use crate::common::{self, peers::Peers};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
};

/// Construct a single synthetic ATOM ROM and an ACPI wrapper for the fixture's physical address.
fn firmware() -> Vec<u8> {
    let mut rom = vec![0_u8; 512];
    rom[..3].copy_from_slice(&[0x55, 0xaa, 1]);
    rom[0x48..0x4a].copy_from_slice(&0x80_u16.to_le_bytes());
    rom[0x84..0x88].copy_from_slice(b"ATOM");
    rom[0x18..0x1a].copy_from_slice(&0xa0_u16.to_le_bytes());
    rom[0xa0..0xa4].copy_from_slice(b"PCIR");
    for (offset, value) in [(0xa4, 0x1002_u16), (0xa6, 0x1681), (0xb0, 1)] {
        rom[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }
    rom[511] = 0_u8.wrapping_sub(rom.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)));
    let mut table = vec![0_u8; 104];
    table[..4].copy_from_slice(b"VFCT");
    table[8] = 1;
    for (offset, value) in [
        (4, 616_u32),
        (52, 76),
        (76, 9),
        (80, 3),
        (84, 0),
        (96, 1),
        (100, 512),
    ] {
        table[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    table[88..90].copy_from_slice(&0x1002_u16.to_le_bytes());
    table[90..92].copy_from_slice(&0x1681_u16.to_le_bytes());
    table.extend(rom);
    table[9] = 0_u8.wrapping_sub(table.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte)));
    table
}

/// Native startup operations share only this private hardware tree and its protected firmware output.
struct Startup {
    // Owns synthetic ACPI/sysfs, table files and operation-specific command environments.
    peers: Peers,
    // Fresh upstream-resolved assignment passed again at every render and verification gate.
    vm: Value,
}

impl Startup {
    /// Seed the real parser's input without reading a physical ROM or live PCI configuration.
    fn new() -> Result<Self> {
        let peers = Peers::new()?;
        super::gpu::hardware(&peers)?;
        fs::write(peers.path("VFCT"), firmware())?;
        Ok(Self {
            peers,
            vm: super::gpu::vm(),
        })
    }

    /// Call one actual native hook with all hardware and writable output redirected into a user namespace.
    fn run(&self, request: Value) -> Result<airlock_vm_service::support::Output> {
        let mut command = self
            .peers
            .command(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))?;
        command
            .args(["gpu", "qubes"])
            .env("AIRLOCK_GPU_PCI", self.peers.path("pci"))
            .env("AIRLOCK_GPU_VFCT", self.peers.path("VFCT"))
            .env("AIRLOCK_GPU_OUTPUT", self.peers.path("output"));
        common::run(
            &mut common::namespace(&command, &self.peers.path(""))?,
            Some(serde_json::to_vec(&request)?),
        )
    }

    /// Prepare a fresh table for the VM's bus-zero slot eighteen.
    fn prepare(&self) -> Result<Value> {
        Ok(serde_json::from_slice(
            &self
                .run(json!({"operation":"prepare","vm":self.vm,"slot":18}))?
                .checked()?,
        )?)
    }

    /// Map a namespace table identity back to the fixture for intentional corruption scenarios.
    fn path(&self, report: &Value) -> Result<PathBuf> {
        let path = report["table_path"]
            .as_str()
            .context("prepared table path")?
            .strip_prefix("/var/lib/airlock/")
            .context("table escaped fixture")?;
        Ok(self.peers.path(path))
    }

    /// Render through the production freshness and protected-file gates.
    fn render(&self, report: &Value, xml: &str) -> Result<airlock_vm_service::support::Output> {
        self.run(json!({"operation":"render","vm":self.vm,"slot":18,"report":report,"xml":xml}))
    }

    /// Verify native Xen configuration against the same current assignment and firmware table.
    fn verify(&self, report: &Value, native: &str) -> Result<airlock_vm_service::support::Output> {
        self.run(
            json!({"operation":"verify","vm":self.vm,"slot":18,"report":report,"native":native}),
        )
    }
}

/// Preserve upstream disk and PCI security flags while supplying exactly one custom ACPI table.
fn xml(report: &Value) -> String {
    format!(
        r#"<domain type="xen"><uuid>{uuid}</uuid>
      <os><type>hvm</type><loader type="rom">hvmloader</loader></os>
      <devices><disk type="block"><readonly/></disk>
        <hostdev type="pci" managed="yes" writeFiltering="yes">
          <source><address domain="0x0000" bus="0x09" slot="0x03" function="0x0"/></source>
        </hostdev>
      </devices></domain>"#,
        uuid = report["domain_uuid"].as_str().unwrap()
    )
}

/// Every startup produces fresh protected firmware while keeping the ROM and upstream device flags unchanged.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_gpu_startup_prepares_fresh_firmware_and_preserves_upstream_xml() -> Result<()> {
    let startup = Startup::new()?;
    let report = startup.prepare()?;
    let path = startup.path(&report)?;
    let table = fs::read(&path)?;
    assert_eq!(
        &table[76..88],
        [
            0_u32.to_le_bytes(),
            18_u32.to_le_bytes(),
            0_u32.to_le_bytes()
        ]
        .concat()
    );
    assert_eq!(report["host_bdf"], "0000:09:03.0");
    assert_eq!(report["guest_bdf"], "0000:00:12.0");
    assert_eq!(report["rom_modified"], false);
    assert_eq!(fs::metadata(path)?.permissions().mode() & 0o777, 0o400);
    assert_ne!(startup.prepare()?["table_path"], report["table_path"]);
    let rendered: String =
        serde_json::from_slice(&startup.render(&report, &xml(&report))?.checked()?)?;
    for fragment in [
        r#"managed="yes""#,
        r#"writeFiltering="yes""#,
        r#"bus="0x09""#,
        r#"slot="0x12""#,
        "<readonly/>",
        report["table_path"].as_str().unwrap(),
    ] {
        assert!(rendered.contains(fragment), "{fragment}");
    }
    let metadata = r#"<metadata><state xmlns="urn:application">unchanged</state></metadata>"#;
    let source = xml(&report).replace("</uuid>", &format!("</uuid>{metadata}"));
    let rendered: String = serde_json::from_slice(&startup.render(&report, &source)?.checked()?)?;
    assert!(rendered.contains(metadata));
    Ok(())
}

/// Conflicting definitions, malformed XML and foreign namespaces cannot produce a partial VM definition.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_gpu_xml_conflicts_fail_closed() -> Result<()> {
    let startup = Startup::new()?;
    let report = startup.prepare()?;
    let original = xml(&report);
    for source in [
        original.replace("bus=\"0x09\"", "bus=\"0x04\""),
        original.replace(
            report["domain_uuid"].as_str().unwrap(),
            "b7e27839-b68f-4c21-8b91-6861db3ec112",
        ),
        original.replace("</os>", "<acpi/></os>"),
        original.replace("</hostdev>", r#"<address type="pci"/></hostdev>"#),
        original.replace(
            "</devices>",
            r#"<controller><address type="pci" slot="0x12"/></controller></devices>"#,
        ),
        format!(r#"<!DOCTYPE domain [<!ENTITY x SYSTEM "file:///etc/passwd">]>{original}"#),
        original.replace("</devices>", r#"<hostdev type="pci"/></devices>"#),
        original.trim_end_matches("</domain>").to_owned(),
        original.replace(
            r#"<domain type="xen">"#,
            r#"<domain type="xen" xmlns="urn:other-domain">"#,
        ),
    ] {
        assert!(!startup.render(&report, &source)?.status.success());
    }
    Ok(())
}

/// Both startup gates reject redirected or writable tables, and content changes cannot preserve freshness.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_gpu_startup_gates_reject_unsafe_or_modified_artifacts() -> Result<()> {
    for (kind, stage) in [
        ("symlink", "render"),
        ("fifo", "verify"),
        ("directory", "render"),
        ("hardlink", "verify"),
        ("writable", "render"),
        ("changed", "verify"),
    ] {
        let startup = Startup::new()?;
        let report = startup.prepare()?;
        let path = startup.path(&report)?;
        if matches!(kind, "writable" | "changed") {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            if kind == "changed" {
                let mut bytes = fs::read(&path)?;
                bytes.extend(b"changed");
                fs::write(&path, bytes)?;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o400))?;
            }
        } else {
            let original = path.with_file_name("original.bin");
            fs::rename(&path, &original)?;
            match kind {
                "symlink" => symlink("original.bin", &path)?,
                "fifo" => rustix::fs::mknodat(
                    rustix::fs::CWD,
                    &path,
                    rustix::fs::FileType::Fifo,
                    rustix::fs::Mode::RUSR,
                    0,
                )?,
                "directory" => fs::create_dir(&path)?,
                _ => fs::hard_link(original, &path)?,
            }
        }
        let result = if stage == "render" {
            startup.render(&report, &xml(&report))?
        } else {
            startup.verify(&report, &native(&report, "0000:09:03.0@12"))?
        };
        assert!(!result.status.success(), "{kind} at {stage}");
    }
    Ok(())
}

/// Native Xen configuration must retain the sole GPU's explicit VM slot and exact ACPI artifact.
fn native(report: &Value, pci: &str) -> String {
    format!(
        r#"pci = ["{pci}"]
acpi_firmware = "{table}"
"#,
        table = report["table_path"].as_str().unwrap()
    )
}

/// Unpatched conversion, extra PCI devices and changed assignments cannot silently reach startup.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_native_gpu_conversion_rechecks_slot_firmware_and_assignment() -> Result<()> {
    let mut startup = Startup::new()?;
    let report = startup.prepare()?;
    assert!(
        !startup
            .verify(&report, &native(&report, "0000:09:03.0"))?
            .status
            .success()
    );
    startup
        .verify(&report, &native(&report, "0000:09:03.0@12"))?
        .checked()?;
    let extra = native(&report, "0000:09:03.0@12").replace(
        r#""0000:09:03.0@12""#,
        r#""0000:09:03.0@12", "0000:01:00.0""#,
    );
    assert!(!startup.verify(&report, &extra)?.status.success());
    startup.vm["assignments"][0]["devices"][0]["port_id"] = json!("different-port");
    let result = startup.verify(&report, &native(&report, "0000:09:03.0@12"))?;
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("assignment changed"));
    Ok(())
}
