//! Read-only XML conversion checks run only against an explicitly selected Xen/libvirt host.

use crate::common;
use anyhow::{Context, Result};
use quick_xml::{Reader, events::Event};
use std::process::Command;

/// Convert XML or native Xen configuration over the existing SSH diagnostic interface.
fn convert(direction: &str, input: &str) -> Result<airlock_vm_service::support::Output> {
    let host = std::env::var("AIRLOCK_VBIOS_LIBVIRT_SSH")
        .context("set AIRLOCK_VBIOS_LIBVIRT_SSH for read-only conversion tests")?;
    common::run(
        Command::new("ssh").args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            &host,
            "virsh",
            "-c",
            "xen:///",
            &format!("domxml-{direction}-native"),
            "xen-xl",
            "/dev/stdin",
        ]),
        Some(input.as_bytes().to_vec()),
    )
}

/// Build an inert HVM definition used only for conversion, never for VM creation or startup.
fn domain(address: &str) -> String {
    format!(
        r#"
        <domain type="xen"><name>airlock-vbios-conversion-only</name>
          <uuid>a7e27839-b68f-4c21-8b91-6861db3ec112</uuid>
          <memory unit="MiB">1024</memory><vcpu>1</vcpu>
          <os><type arch="x86_64" machine="xenfv">hvm</type><loader type="rom">hvmloader</loader></os>
          <devices><hostdev mode="subsystem" type="pci" managed="yes">
            <source><address domain="0x0000" bus="0x09" slot="0x03" function="0x0"/></source>
            {address}
          </hostdev></devices>
        </domain>
    "#
    )
}

/// Explicit representable VM slots survive conversion in both directions.
#[test]
#[ignore = "requires an explicitly selected Xen/libvirt SSH host"]
fn remote_explicit_gpu_slots_survive_native_roundtrip() -> Result<()> {
    for slot in [8, 16, 18, 31] {
        let address = format!(
            r#"<address type="pci" domain="0x0000" bus="0x00" slot="0x{slot:02x}" function="0x0"/>"#
        );
        let native = String::from_utf8(convert("to", &domain(&address))?.checked()?)?;
        assert!(native.contains(&format!("0000:09:03.0@{slot:02x}")));
        let xml = String::from_utf8(convert("from", &native)?.checked()?)?;
        let mut reader = Reader::from_str(&xml);
        let mut found = false;
        loop {
            match reader.read_event()? {
                Event::Empty(element) | Event::Start(element)
                    if element.name().as_ref() == b"address" =>
                {
                    let mut pci = false;
                    let mut actual = None;
                    for attribute in element.attributes() {
                        let attribute = attribute?;
                        if attribute.key.as_ref() == b"type" {
                            pci = attribute.value.as_ref() == b"pci";
                        }
                        if attribute.key.as_ref() == b"slot" {
                            actual = Some(String::from_utf8(attribute.value.to_vec())?);
                        }
                    }
                    if pci {
                        assert_eq!(
                            u32::from_str_radix(
                                actual
                                    .as_deref()
                                    .context("missing VM slot")?
                                    .trim_start_matches("0x"),
                                16
                            )?,
                            slot
                        );
                        found = true;
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
        assert!(found, "converted XML omitted the explicit PCI address");
    }
    Ok(())
}

/// Automatic placement stays automatic, while unsupported XML addresses and native slots are refused.
#[test]
#[ignore = "requires an explicitly selected Xen/libvirt SSH host"]
fn remote_conversion_refuses_unrepresentable_gpu_addresses() -> Result<()> {
    let automatic = String::from_utf8(convert("to", &domain(""))?.checked()?)?;
    assert!(automatic.contains("\"0000:09:03.0\""));
    assert!(!automatic.contains('@'));
    for (field, value) in [
        ("bus", "0x01"),
        ("function", "0x1"),
        ("domain", "0x0001"),
        ("slot", "0x00"),
    ] {
        let mut fields = std::collections::BTreeMap::from([
            ("type", "pci"),
            ("domain", "0x0000"),
            ("bus", "0x00"),
            ("slot", "0x10"),
            ("function", "0x0"),
        ]);
        fields.insert(field, value);
        let attributes = fields
            .iter()
            .map(|(key, value)| format!(r#"{key}="{value}""#))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            !convert("to", &domain(&format!("<address {attributes}/>")))?
                .status
                .success(),
            "{field}"
        );
    }
    for slot in ["00", "20", "-1", "10.1", "10junk"] {
        assert!(
            !convert(
                "from",
                &automatic.replace("0000:09:03.0", &format!("0000:09:03.0@{slot}"))
            )?
            .status
            .success(),
            "{slot}"
        );
    }
    Ok(())
}

/// Upstream event dispatch must run native guards before device side effects and clear stale startup state.
#[test]
#[ignore = "requires an explicitly selected Qubes host and AIRLOCK_QUBES_PROBE"]
fn remote_qubes_gpu_guards_precede_device_side_effects() -> Result<()> {
    let host = std::env::var("AIRLOCK_VBIOS_LIBVIRT_SSH").context("select a Qubes SSH host")?;
    let probe = std::env::var("AIRLOCK_QUBES_PROBE")
        .context("select the prepared upstream Qubes probe on the remote host")?;
    let remote = format!(
        "{probe} gpu-events",
        probe = airlock_tests::process::quote(&probe)
    );
    let report = common::json(
        Command::new("ssh").args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            &host,
            &remote,
        ]),
        None,
    )?;
    let records = report.as_array().context("Qubes dispatcher observations")?;
    assert_eq!(records.len(), 3);
    for record in records {
        assert!(record["error"].is_string());
        assert_eq!(record["reached"], false);
        assert_eq!(record["ordinary_reached"], true);
        if record["event"] == "domain-pre-start" {
            assert_eq!(record["stale_cleared"], true);
        }
    }
    Ok(())
}

/// Native callbacks use the real upstream dispatcher and retain the actual Qubes database lock.
#[test]
#[ignore = "explicitly prepare AIRLOCK_QUBES_PROBE_PACKAGE with upstream Qubes"]
fn external_tool_qubes_plugins_preserve_upstream_events_and_storage_lock() -> Result<()> {
    let probe = std::env::var_os("AIRLOCK_QUBES_PROBE_PACKAGE")
        .context("select the prepared upstream Qubes probe")?;
    let events = common::json(Command::new(&probe).arg("gpu-events"), None)?;
    for record in events.as_array().context("Qubes event observations")? {
        assert!(record["error"].is_string());
        assert_eq!(record["reached"], false);
        assert_eq!(record["ordinary_reached"], true);
    }
    let transaction = common::json(
        Command::new(&probe).arg("storage-transaction"),
        Some(b"{}".to_vec()),
    )?;
    assert_eq!(transaction["guards_under_lock"], 2);
    assert_eq!(transaction["private_size"], 1048576);
    assert_eq!(transaction["stale_rejected"], true);
    assert_eq!(transaction["lock_released"], true);
    Ok(())
}
