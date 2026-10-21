//! A staged host generation must be verified after booting the same writable nested-VM image.

use crate::common;
use anyhow::Result;
use serde_json::{Value, json};
use std::{fs, process::Command};

/// The real orchestration reuses one image, requires a pending generation and refuses verification of a different running system.
#[test]
fn staged_boot_flow_reuses_image_and_checks_generation() -> Result<()> {
    for damage in ["", "missing-source", "not-pending", "wrong-running"] {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path();
        let scripts = root.join("peers");
        fs::create_dir_all(&scripts)?;
        common::peer_link(&scripts, "run-nested-gui-smoke.sh")?;
        fs::create_dir(root.join("bin"))?;
        common::peer_link(&root.join("bin"), "nix")?;
        fs::create_dir(root.join("image"))?;
        fs::create_dir(root.join("logs"))?;
        let mut reports = json!({
            "stage": {"schema": "airlock.config-vms-smoke.v1", "status": "passed",
                "dom0_system_apply": {"status": "passed", "host_system_source": "/nix/store/new-host", "host_system_switch_action": "boot", "reboot_required": true},
                "config_status": {"pending_reboot": {"required": true}, "host_generation": {"active": false, "staged_source": "/nix/store/new-host", "pending_reboot": {"required": true}}}},
            "verify": {"schema": "airlock.config-boot-verify.v1", "status": "passed", "expected_host_system_source": "/nix/store/new-host",
                "host_system_profile_resolved_target": "/nix/store/new-host", "run_current_system_resolved_target": "/nix/store/new-host", "pending_reboot": {"required": false},
                "status_json": {"host_generation": {"active": true, "current_source": "/nix/store/new-host", "pending_reboot": {"required": false}}}}
        });
        match damage {
            "missing-source" => {
                reports["stage"]["dom0_system_apply"]
                    .as_object_mut()
                    .unwrap()
                    .remove("host_system_source");
            }
            "not-pending" => {
                reports["stage"]["config_status"]["pending_reboot"]["required"] = json!(false)
            }
            "wrong-running" => {
                reports["verify"]["run_current_system_resolved_target"] =
                    json!("/nix/store/old-host")
            }
            _ => {}
        }
        fs::write(root.join("reports.json"), serde_json::to_vec(&reports)?)?;
        let mut command = Command::new(common::binary("AIRLOCK_DEV_BIN", "airlock-dev"));
        command
            .arg("--repository-root")
            .arg(root)
            .args(["nested", "boot-flow"])
            .env(
                "AIRLOCK_NESTED_SMOKE_COMMAND",
                scripts.join("run-nested-gui-smoke.sh"),
            );
        command
            .env("AIRLOCK_USE_HOST_NIX_STORE", "1")
            .env("AIRLOCK_TEST_FLOW", root)
            .env("AIRLOCK_LOG_DIR", root.join("logs"))
            .env("AIRLOCK_VM_DIR", root.join("vm"))
            .env(
                "PATH",
                format!(
                    "{bin}:{path}",
                    bin = root.join("bin").display(),
                    path = std::env::var("PATH")?
                ),
            );
        let output = common::run(&mut command, None)?;
        assert_eq!(
            output.status.success(),
            damage.is_empty(),
            "{damage}: {errors}",
            errors = String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&fs::read(
            root.join("logs/vm-config-boot-activate-flow.json"),
        )?)?;
        assert_eq!(
            report["status"],
            if damage.is_empty() {
                "passed"
            } else {
                "failed"
            }
        );
        let calls = fs::read_to_string(root.join("calls.jsonl"))?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let phases: Vec<_> = calls
            .iter()
            .map(|call| call["phase"].as_str().unwrap())
            .collect();
        if ["missing-source", "not-pending"].contains(&damage) {
            assert_eq!(phases, ["stage"]);
        } else {
            assert_eq!(phases, ["stage", "verify"]);
        }
        if damage.is_empty() {
            assert_eq!(
                fs::read_to_string(root.join("events"))?,
                "resolve\nstage\nverify\n"
            );
            assert_eq!(calls[0]["AIRLOCK_VM_DIR"], calls[1]["AIRLOCK_VM_DIR"]);
            assert_eq!(
                calls[0]["AIRLOCK_VM_IMAGE_ATTR"],
                calls[1]["AIRLOCK_VM_IMAGE_ATTR"]
            );
            assert_eq!(calls[0]["AIRLOCK_VM_FRESH"], "1");
            assert_eq!(calls[1]["AIRLOCK_VM_FRESH"], "0");
            assert!(
                calls
                    .iter()
                    .all(|call| call["AIRLOCK_VM_SKIP_BASE_BUILD"] == "1")
            );
            assert_eq!(report["expected_host_system_source"], "/nix/store/new-host");
            let result = fs::read_dir(root.join("vm"))?
                .find_map(|entry| {
                    let path = entry.ok()?.path();
                    path.file_name()?
                        .to_str()?
                        .ends_with("-result")
                        .then_some(path)
                })
                .unwrap();
            assert_eq!(fs::canonicalize(result)?, root.join("image"));
        }
    }
    Ok(())
}
