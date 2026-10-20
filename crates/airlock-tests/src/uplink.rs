//! Run the real systemd/udev dummy-interface regression only inside its explicitly marked disposable NixOS VM.

use airlock_vm_service::support;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

/// Execute one short VM-local command and preserve failed tool diagnostics.
fn run(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    support::run(
        Command::new(program).args(args),
        None,
        Duration::from_secs(10),
    )?
    .checked()
}

/// Read one systemd property without combining independent unit-health decisions.
fn property(unit: &str, name: &str) -> Result<String> {
    Ok(String::from_utf8(run(
        "systemctl",
        &["show", unit, &format!("--property={name}"), "--value"],
    )?)?
    .trim()
    .to_owned())
}

/// Wait for udev-triggered state transitions under a short deadline.
fn wait(unit: &str, expected: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while property(unit, "ActiveState")? != expected {
        ensure!(Instant::now() < deadline, "{unit} did not reach {expected}");
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Own only a named disposable dummy interface and its matching test unit.
struct Interface {
    // Fixed private interface name matched only by the test VM's udev rules.
    name: &'static str,
    // Unit whose pending start must be cancelled before draining udev on cleanup.
    unit: String,
}

impl Drop for Interface {
    fn drop(&mut self) {
        let _ = run("systemctl", &["--no-block", "stop", &self.unit]);
        let _ = run("udevadm", &["settle", "--timeout=5"]);
        let _ = run("ip", &["link", "delete", self.name]);
        let _ = run("udevadm", &["settle", "--timeout=5"]);
    }
}

impl Interface {
    /// Observe actual interface flags and address configuration after a real kernel/udev event.
    fn address(&self) -> Result<Value> {
        let values: Value =
            serde_json::from_slice(&run("ip", &["-j", "address", "show", "dev", self.name])?)?;
        Ok(values[0].clone())
    }
}

/// Require explicit test-VM identity before creating or removing the regression's dummy interfaces.
fn main() -> Result<()> {
    ensure!(
        std::env::args().nth(1).as_deref() == Some("--inside-test-vm")
            && Path::new("/etc/airlock-uplink-test").is_file(),
        "run only in the explicitly prepared uplink test VM"
    );
    let ordinary = Interface {
        name: "altest0",
        unit: "qubes-network-uplink@altest0.service".into(),
    };
    let mut invocations = BTreeSet::new();
    for _ in 0..3 {
        run("ip", &["link", "add", ordinary.name, "type", "dummy"])?;
        run("udevadm", &["settle", "--timeout=5"])?;
        wait(&ordinary.unit, "active")?;
        let invocation = property(&ordinary.unit, "InvocationID")?;
        ensure!(
            !invocation.is_empty() && invocations.insert(invocation),
            "recreated interface did not start a new service invocation"
        );
        let address = ordinary.address()?;
        ensure!(
            address["flags"]
                .as_array()
                .context("interface flags")?
                .contains(&json!("UP")),
            "uplink remained down"
        );
        ensure!(
            address["addr_info"]
                .as_array()
                .context("interface addresses")?
                .iter()
                .any(|entry| entry["local"] == "192.0.2.2"),
            "uplink address missing"
        );
        run("ip", &["link", "delete", ordinary.name])?;
        run("udevadm", &["settle", "--timeout=5"])?;
        wait(&ordinary.unit, "inactive")?;
    }
    drop(ordinary);

    let blocking = Interface {
        name: "alblock0",
        unit: "airlock-blocking-uplink@alblock0.service".into(),
    };
    run("ip", &["link", "add", blocking.name, "type", "dummy"])?;
    let _ = run("udevadm", &["settle", "--timeout=3"]);
    ensure!(
        property(&blocking.unit, "ActiveState")? != "active",
        "synchronous device-dependent control unexpectedly configured"
    );
    ensure!(
        !blocking.address()?["addr_info"]
            .as_array()
            .context("control interface addresses")?
            .iter()
            .any(|entry| entry["local"] == "192.0.2.2"),
        "blocking control configured its address"
    );
    let journal = String::from_utf8(run(
        "journalctl",
        &["-b", "-u", &blocking.unit, "--no-pager"],
    )?)?;
    let jobs = String::from_utf8(run("systemctl", &["list-jobs", "--no-pager"])?)?;
    ensure!(
        journal.to_lowercase().contains("dependency")
            || (jobs.contains(&blocking.unit)
                && jobs.contains("sys-subsystem-net-devices-alblock0.device")),
        "negative control failed without expected device dependency evidence"
    );
    drop(blocking);
    println!(
        "{}",
        json!({"schema": "airlock.uplink-hotplug-test.v1", "status": "passed", "invocations": invocations.len(), "blocking_control": "refused"})
    );
    Ok(())
}
