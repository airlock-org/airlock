//! Generated USB images preserve source provenance, independent recovery and narrow development control paths.

use super::{
    configuration::{inspect, text},
    *,
};

/// Check list membership or a generated command fragment without imposing unrelated order.
fn includes(value: &Value, expected: &str) {
    assert!(
        match value {
            Value::String(value) => value.contains(expected),
            Value::Array(value) => value.contains(&json!(expected)),
            _ => false,
        },
        "missing {expected}: {value}"
    );
}

/// Image validation runs before assembly, recovery logging survives failure and the runtime closure does not retain its pool payload twice.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources"]
fn prepared_image_validates_sealed_pool_before_assembly() -> Result<()> {
    let nix = context()?;
    let report = inspect(
        &nix,
        "prepared_store/sealed_store",
        json!({"root": nix.prepared}),
    )?;
    assert_eq!(report["mode"], "go-rwx");
    includes(&report["hostOrdering"], "airlock-host-store.service");
    includes(&report["earlyLogs"], "airlock-boot-journal");
    assert_eq!(report["recoveryLogs"], true);
    includes(&report["recoveryRecorder"], "airlock-boot-journal");
    assert_eq!(report["journalConfig"]["Storage"], "persistent");
    assert_eq!(report["journalConfig"]["SyncIntervalSec"], "1s");
    assert!(text(&report["daemonConfig"]).starts_with("store = local-overlay://?"));
    includes(&report["daemonConfig"], "local%%3Froot%%3D");
    let dependencies = report["dependencies"].as_array().unwrap();
    assert!(!dependencies.contains(&report["runtimeSeed"]));
    assert!(dependencies.contains(&report["validation"]));
    let command = text(&report["imageCommand"]);
    let mut previous = 0;
    for token in [
        "mount $rootDisk $mountPoint",
        "Installing prepared Qubes runtime state",
        "switch-to-configuration boot",
        "chown -R root:qubes /var/lib/qubes",
        "umount -R /mnt",
    ] {
        let index = command
            .find(token)
            .with_context(|| format!("image command missing {token}"))?;
        assert!(index >= previous);
        previous = index;
    }
    includes(
        &report["imageCommand"],
        "env NIX_REMOTE=local NIX_CONFIG='store = local'",
    );
    crate::image_seed::validation_hook(text(&report["preVM"]), text(&report["checkCommand"]))
}

/// The lab keeps a single firewall chain, explicit controller authority and boot supervision without host networking services.
#[test]
#[ignore = "explicit package builds: report reads realized RPC and policy scripts"]
fn package_usb_lab_keeps_network_dependencies_and_development_authority_explicit() -> Result<()> {
    let report =
        crate::packaged::inspect_configuration("usb_lab/network_and_development_access", ".")?;
    for prerequisite in [
        "airlock-setup-config.service",
        "airlock-start-gui-gpu.service",
        "airlock-start-network-providers.service",
    ] {
        includes(&report["autostart"]["after"], prerequisite);
        includes(&report["autostart"]["requires"], prerequisite);
    }
    assert_eq!(
        report["autostart"]["wantedBy"],
        json!(["multi-user.target"])
    );
    assert!(
        text(&report["autostart"]["command"]).ends_with("boot-start --require-running sys-gui")
    );
    includes(
        &report["autostart"]["sync"][0],
        "airlock-dom0-service boot-supervisor complete",
    );
    assert_eq!(report["retireDomains"], json!([]));
    assert_eq!(report["trashRetentionDays"], 30);
    includes(
        &report["retiredPolicy"],
        "* * @anyvm @tag:airlock-retired deny",
    );
    let supervisor = &report["bootSupervisor"];
    assert_eq!(supervisor["enabled"], true);
    includes(&supervisor["prepare"]["wantedBy"], "multi-user.target");
    for dependency in ["xenconsoled.service", "qubesd.service"] {
        includes(&supervisor["prepare"]["before"], dependency);
    }
    assert!(text(&supervisor["recoveryCondition"]).ends_with("/recovery-required.json"));
    includes(
        &supervisor["failureCapture"],
        "airlock-dom0-service boot-supervisor fail",
    );
    includes(
        &supervisor["failureFinally"],
        "airlock-boot-failure-console.service",
    );
    assert_eq!(supervisor["tty1"], false);
    assert_eq!(supervisor["autoTty1"], false);
    assert_eq!(supervisor["failureConsole"]["TTYPath"], "/dev/tty1");
    for (service, timeout) in [("network", 300), ("gui", 300), ("autostart", 1800)] {
        assert_eq!(
            supervisor[service]["serviceConfig"]["TimeoutStartSec"],
            timeout
        );
        includes(
            &supervisor[service]["serviceConfig"]["ExecCondition"][0],
            "test ! -e /run/airlock/boot-supervisor/recovery-required.json",
        );
        assert_eq!(
            supervisor[service]["unitConfig"]["OnFailure"],
            json!([if service == "gui" {
                "airlock-boot-failure@%n.service"
            } else {
                "airlock-boot-background-failure.service"
            }])
        );
    }
    for key in ["default_netvm", "devNetvm", "ctrlNetvm", "homeNetvm"] {
        assert_eq!(report[key], "sys-firewall");
    }
    assert_eq!(report["firewallNetvm"], "sys-net");
    assert!(report["netNetvm"].is_null());
    for key in [
        "devProvider",
        "hostNetworkManager",
        "hostTailscale",
        "hostSsh",
        "labGuiHasDom0Terminal",
    ] {
        assert_eq!(report[key], false);
    }
    for key in ["netProvider", "firewallProvider"] {
        assert_eq!(report[key], true);
    }
    for key in ["devMemory", "devMaxmem", "ctrlMemory", "ctrlMaxmem"] {
        assert_eq!(report[key], 8192);
    }
    assert_eq!(report["ctrlBuildJobs"], 1);
    assert_eq!(report["devAutoPci"], "0");
    assert_eq!(report["netAutoPci"], "1");
    assert_eq!(report["devPci"], json!([]));
    assert_eq!(report["netPci"], json!([]));
    assert_eq!(report["callers"], json!(["usr-assistant"]));
    includes(
        &report["policy"],
        "airlock.DevDom0Shell + usr-assistant dom0 allow",
    );
    assert!(!text(&report["policy"]).contains("airlock.DevDom0Shell + sys-gui dom0 allow"));
    for service in ["airlock.CaptureImport", "airlock.CaptureOpen"] {
        includes(
            &report["capturePolicy"],
            &format!("{service} * @anyvm @anyvm deny"),
        );
    }
    includes(
        &report["tailscalePolicy"],
        "airlock.DevTailscaleUp + sys-gui usr-assistant allow user=root",
    );
    includes(
        &report["tailscalePolicy"],
        "airlock.DevTailscaleUp * @anyvm @anyvm deny",
    );
    let temporary = tempfile::tempdir()?;
    let mut enrollment = report["tailscaleHandler"].clone();
    assert_eq!(enrollment["args"], json!(["runtime", "tailscale-up"]));
    // Execute the actual native guard with an untrusted caller or service argument.
    enrollment["program"] = json!(common::binary(
        "AIRLOCK_VM_SERVICE_BIN",
        "airlock-vm-service"
    ));
    for (source, argument) in [("usr-browser", ""), ("sys-gui", "unexpected")] {
        let output = common::run(
            common::launcher(temporary.path(), &enrollment)?
                .env("QREXEC_REMOTE_DOMAIN", source)
                .env("QREXEC_SERVICE_ARGUMENT", argument),
            None,
        )?;
        assert_eq!(output.status.code(), Some(1));
    }
    assert_eq!(report["poolAdmission"]["caller"], "sys-ctrl");
    for target in report["poolAdmission"]["targets"].as_array().unwrap() {
        includes(
            &report["poolAdmissionPolicy"],
            &format!(
                "airlock.Pool.Admit +{target} sys-ctrl dom0 allow",
                target = text(target)
            ),
        );
    }
    assert_eq!(report["poolAdmissionMode"], "0555");
    assert_eq!(
        report["dev"]["configNixStoreRoot"],
        "/rw/airlock-builder/merged"
    );
    assert_eq!(report["dev"]["providesNetwork"], false);
    assert_eq!(report["dev"]["networkManager"], false);
    assert_eq!(report["dev"]["dom0Shell"], true);
    assert_eq!(report["dev"]["tailscaleSsh"], json!(["--ssh"]));
    Ok(())
}

/// Parent and standalone machine images preserve machine lock identity independently from their own boot-source labels.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources"]
fn usb_image_source_and_boot_provenance_are_distinct() -> Result<()> {
    let nix = context()?;
    let identity: Value = serde_json::from_slice(&fs::read(
        nix.prepared.join("airlock-machine/airlock-source.json"),
    )?)?;
    let lock_hash = format!(
        "{hash:x}",
        hash = Sha256::digest(fs::read(nix.prepared.join("airlock-machine/flake.lock"))?)
    );
    for (flake, configuration) in [
        (nix.prepared.clone(), "usb-dev"),
        (nix.prepared.join("airlock-machine"), "machine"),
    ] {
        let report = inspect(
            &nix,
            "usb_lab/source_metadata",
            json!({"flake": flake, "configuration": configuration}),
        )?;
        assert_eq!(
            report["source"],
            json!({"schema": "airlock.machine-source.v1", "revision": identity["revision"], "lockHash": lock_hash})
        );
        let boot = inspect(
            &nix,
            "usb_lab/boot_source_metadata",
            json!({"repo": flake, "configuration": configuration}),
        )?;
        assert_eq!(boot["normal"], boot["recovery"]);
        let source: Value = serde_json::from_slice(&fs::read(flake.join("airlock-source.json"))?)?;
        assert_eq!(
            boot["normal"],
            json!({"revision": source["revision"], "timestamp": source["timestamp"]})
        );
        assert_eq!(boot["consoleMode"], "1");
    }
    Ok(())
}
