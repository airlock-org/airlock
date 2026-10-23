//! Enrollment failures and provider probes remain bounded and require the firewall to be healthy.

use crate::common::{self, peers::Peers};
use anyhow::Result;
use serde_json::{Value, json};
use std::{fs, process::Command};

/// Provide explicitly inert network executables while leaving all protocol and shell decisions in the real script.
fn fixture(config: Value) -> Result<tempfile::TempDir> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path();
    fs::create_dir(root.join("bin"))?;
    for name in [
        "ip",
        "getent",
        "curl",
        "tailscale",
        "timeout",
        "qvm-start",
        "qvm-check",
        "qvm-ls",
        "qrexec-client",
        "systemctl",
        "supervisor",
    ] {
        common::peer_link(&root.join("bin"), name)?;
    }
    let mut config = config;
    config["bash"] = json!(common::tool("bash")?);
    fs::write(root.join("network.json"), serde_json::to_vec(&config)?)?;
    Ok(temporary)
}

/// Configure only a child's finite tool namespace, without changing global test process environment.
fn command(temporary: &tempfile::TempDir, operation: &str) -> Result<Command> {
    let root = temporary.path();
    let mut command = if operation == "tailscale-up" {
        let mut command = Command::new(common::binary(
            "AIRLOCK_VM_SERVICE_BIN",
            "airlock-vm-service",
        ));
        command.args(["runtime", "tailscale-up"]);
        command
    } else {
        let mut command = Command::new(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"));
        command.arg(operation);
        command
    };
    command
        .env(
            "AIRLOCK_VM_SERVICE",
            common::binary("AIRLOCK_VM_SERVICE_BIN", "airlock-vm-service"),
        )
        .env("AIRLOCK_TEST_NETWORK_ROOT", root)
        .env(
            "PATH",
            format!(
                "{bin}:{path}",
                bin = root.join("bin").display(),
                path = std::env::var("PATH")?
            ),
        );
    Ok(command)
}

/// Route, DNS, HTTPS and login failures identify the failing step without hanging or enrolling prematurely.
#[test]
fn development_enrollment_reports_each_network_failure() -> Result<()> {
    for (failure, message) in [
        ("route", "No network route"),
        ("dns", "DNS is not working"),
        ("https", "Cannot reach Tailscale"),
        ("login", "login timed out"),
        ("", "Starting Tailscale login"),
    ] {
        let fixture = fixture(json!({"failure": failure}))?;
        let output = common::run(
            command(&fixture, "tailscale-up")?
                .env("QREXEC_REMOTE_DOMAIN", "sys-gui")
                .env("QREXEC_SERVICE_ARGUMENT", ""),
            None,
        )?;
        let diagnostics = format!(
            "{stdout}{stderr}",
            stdout = String::from_utf8_lossy(&output.stdout),
            stderr = String::from_utf8_lossy(&output.stderr)
        );
        assert!(diagnostics.contains(message), "{failure}: {diagnostics}");
        assert_eq!(output.status.success(), failure.is_empty());
        assert_eq!(fixture.path().join("enrolled").exists(), failure.is_empty());
    }
    Ok(())
}

/// Enrollment waits for its network path, then proceeds while unrelated apps still start.
#[test]
fn desktop_enrollment_waits_for_startup_and_explains_refusal() -> Result<()> {
    for outcome in [0, 126] {
        let mut peers = Peers::new()?;
        let pending = json!({
            "status":"working", "desktop_ready":true, "detail":"Preparing usr-assistant",
            "guests":[
                {"name":"sys-net","phase":"ready","applications":false},
                {"name":"sys-firewall","phase":"ready","applications":false},
                {"name":"usr-assistant","phase":"starting","applications":true}
            ]
        });
        let ready = json!({
            "status":"working", "desktop_ready":true, "detail":"Preparing usr-extra",
            "guests":[
                {"name":"sys-net","phase":"ready","applications":false},
                {"name":"sys-firewall","phase":"ready","applications":false},
                {"name":"usr-assistant","phase":"ready","applications":true},
                {"name":"usr-home","phase":"ready","applications":true},
                {"name":"usr-extra","phase":"starting","applications":true}
            ]
        });
        peers.rules("qvm-run", json!([
            {"contains":"airlock.BootStatus","exists":"status-seen","stdout":ready.to_string()},
            {"contains":"airlock.BootStatus","exists":"transport-seen","stdout":pending.to_string(),"write":{"status-seen":""}},
            {"contains":"airlock.BootStatus","exit":126,"stderr":"Request refused","write":{"transport-seen":""}},
            {"contains":"airlock.DevTailscaleUp","exit":outcome,"stdout":if outcome == 0 {"Login URL: https://login.example.test/fixture\n"} else {""},"stderr":if outcome == 126 {"Request refused"} else {""}}
        ]))?;
        let output = common::run(
            peers
                .command(common::binary(
                    "AIRLOCK_SYS_GUI_BIN",
                    "airlock-sys-gui-service",
                ))?
                .args(["tailscale-enrollment", "--startup-timeout", "10"])
                .env("AIRLOCK_QVM_RUN", peers.path("bin/qvm-run")),
            None,
        )?;
        assert_eq!(
            output.status.code(),
            Some(outcome),
            "stdout={stdout} stderr={stderr} calls={calls}",
            stdout = String::from_utf8_lossy(&output.stdout),
            stderr = String::from_utf8_lossy(&output.stderr),
            calls = peers.calls()?.len(),
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stdout.contains("Preparing usr-assistant"));
        assert!(stdout.contains("Network and usr-assistant are ready"));
        assert_eq!(stdout.contains("Login URL:"), outcome == 0);
        if outcome == 126 {
            assert!(stderr.contains("policy restriction or an interrupted control service"));
        }
        assert_eq!(
            peers
                .calls()?
                .iter()
                .filter(|call| call["args"]
                    .as_array()
                    .unwrap()
                    .contains(&json!("airlock.DevTailscaleUp")))
                .count(),
            1
        );
    }
    Ok(())
}

/// A timed-out or degraded boot must not trigger a guest start or enrollment request.
#[test]
fn desktop_enrollment_requires_successful_startup() -> Result<()> {
    for state in ["working", "degraded"] {
        let mut peers = Peers::new()?;
        peers.rules(
            "qvm-run",
            json!([{"contains":"airlock.BootStatus","stdout":json!({"status":state,"desktop_ready":true,"detail":"Preparing usr-home"}).to_string()}]),
        )?;
        let output = common::run(
            peers
                .command(common::binary(
                    "AIRLOCK_SYS_GUI_BIN",
                    "airlock-sys-gui-service",
                ))?
                .args(["tailscale-enrollment", "--startup-timeout", "1"])
                .env("AIRLOCK_QVM_RUN", peers.path("bin/qvm-run")),
            None,
        )?;
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("enrollment was not requested"));
        assert!(peers.calls()?.iter().all(|call| {
            !call["args"]
                .as_array()
                .unwrap()
                .contains(&json!("airlock.DevTailscaleUp"))
        }));
    }
    Ok(())
}

/// A live network provider is not ready when its firewall is inactive, even if another requested unit is healthy.
#[test]
fn provider_readiness_checks_each_required_service() -> Result<()> {
    for healthy in [false, true] {
        let fixture = fixture(json!({"healthy": healthy}))?;
        let root = fixture.path();
        let topology = root.join("topology.json");
        fs::write(
            &topology,
            serde_json::to_vec(
                &json!({"domains": {"sys-net": {"prefs": {"provides_network": true}, "backend": {"system": {"source": "/nix/store/expected"}}}}}),
            )?,
        )?;
        let output = common::run(
            command(&fixture, "start-qube-ready")?
                .env(
                    "AIRLOCK_DOM0_SERVICE",
                    common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"),
                )
                .arg("sys-net")
                .arg(topology)
                .env("AIRLOCK_BOOT_SERVICE", root.join("bin/supervisor"))
                .env("AIRLOCK_QUBE_READY_TIMEOUT", "1")
                .env("AIRLOCK_BOOT_ATTEMPT_LOG_DIR", root.join("logs")),
            None,
        )?;
        assert_eq!(
            output.status.success(),
            healthy,
            "{errors}",
            errors = String::from_utf8_lossy(&output.stderr)
        );
        if !healthy {
            assert!(
                fs::read_to_string(root.join("logs/start-sys-net.log"))?
                    .contains("Network provider unhealthy: qubes-firewall.service")
            );
        }
    }
    Ok(())
}

/// A worker answering qrexec on the right system is still unready until its build store passes.
#[test]
fn worker_readiness_checks_the_build_store_before_success() -> Result<()> {
    for healthy in [false, true] {
        let fixture = fixture(json!({"healthy":healthy}))?;
        let root = fixture.path();
        let topology = root.join("topology.json");
        fs::write(
            &topology,
            serde_json::to_vec(&json!({"domains":{"sys-ctrl":{
                "features":{"airlock-role":"sys-ctrl"}, "backend":{"system":{"source":"/nix/store/expected"}}
            }}}))?,
        )?;
        let output = common::run(
            command(&fixture, "start-qube-ready")?
                .arg("sys-ctrl")
                .arg(topology)
                .env("AIRLOCK_BOOT_SERVICE", root.join("bin/supervisor"))
                .env("AIRLOCK_QUBE_READY_TIMEOUT", "1")
                .env("AIRLOCK_BOOT_ATTEMPT_LOG_DIR", root.join("logs")),
            None,
        )?;
        assert_eq!(
            output.status.success(),
            healthy,
            "{errors}",
            errors = String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            fs::read_to_string(root.join("logs/start-sys-ctrl.log"))?
                .contains("boot-health build-store --domain sys-ctrl")
        );
    }
    Ok(())
}
