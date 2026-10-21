//! Retained configuration transport keeps validation, source materialization, activation and audit boundaries.

mod fixture;

use crate::common;
use anyhow::Result;
use fixture::Config;
use serde_json::{Value, json};
use std::{fs, io::Read, os::unix::fs::symlink, path::PathBuf, process::Command};

/// Empty compatibility overrides use the installed defaults just as the former shell launchers did.
#[test]
fn configuration_empty_settings_retain_the_installed_defaults() -> Result<()> {
    let mut config = Config::new()?;
    config.pack()?;
    let mut command = config.command("apply", &["--apply"])?;
    command
        .env("AIRLOCK_CONFIG_APPLY_STAGE", "")
        .env("AIRLOCK_CONFIG_DEPLOYMENT_ARCHIVE_SERVICE", "");
    let report = common::json(&mut command, None)?;
    assert_eq!(report["stage"], "full");
    assert_eq!(
        report["deployment_archive_service"],
        "airlock.Config.ArchiveDeployment"
    );
    assert_eq!(report["deployment"]["status"], "passed");
    Ok(())
}

/// Both source resolution and direct activation reject executable paths that replace or leave the admitted profile.
#[test]
fn configuration_profile_activation_cannot_escape_its_source() -> Result<()> {
    for direct in [false, true] {
        for absolute in [false, true] {
            let mut config = Config::new()?;
            let profile = config.path("declared-profile");
            fs::create_dir(&profile)?;
            let outside = if absolute {
                config
                    .path("candidate/profile/bin/airlock-dom0-profile-activate")
                    .to_string_lossy()
                    .into_owned()
            } else {
                "../candidate/profile/bin/airlock-dom0-profile-activate".into()
            };
            config.system["dom0"]["profile"]["source"] = json!(profile);
            config.system["dom0"]["profile"]["activation_command"] = json!(outside);
            let output = if direct {
                config.write("host.json", &config.system)?;
                common::run(
                    config
                        .native(&["config", "host"])?
                        .arg("--system")
                        .arg(config.path("host.json")),
                    None,
                )?
            } else {
                config.pack()?;
                common::run(&mut config.command("apply", &["--apply"])?, None)?
            };
            assert!(
                !output.status.success(),
                "escaped profile activation was accepted"
            );
            assert!(!config.path("state/profile-activated").exists());
            assert!(
                config
                    .path("profiles/airlock-system")
                    .symlink_metadata()
                    .is_err()
            );
            assert!(
                config
                    .calls()?
                    .iter()
                    .all(|call| call["command"] != "nix-env")
            );
        }
    }
    Ok(())
}

/// Live and boot activation persist different host state, and failed switching retains its full deployment audit.
#[test]
fn configuration_activation_persists_live_boot_and_failed_outcomes() -> Result<()> {
    for (action, fail) in [("switch", false), ("boot", false), ("switch", true)] {
        let mut config = Config::new()?;
        config.system["dom0"]["profile"]["source"] = json!("profile");
        config.system["dom0"]["nixos"]["system"] = json!({"enable": true, "source": "dom0-system", "source_ref": null, "switch_action": action});
        config.system["dom0"]["systemd"]["services"] = json!({"airlock-test-restart": {"action": "restart"}, "airlock-test-reload": {"action": "reload"}});
        if action == "boot" {
            let reason = "install next dom0 boot generation";
            config.system["plan"] = json!({"live_actions": [], "reboot_actions": [{"type": "dom0-nixos-system", "switch_action": "boot", "reason": reason}], "reboot_required": true, "reboot_reasons": [reason]});
        }
        if fail {
            config.data["fail"] = json!("switch-to-configuration");
        }
        config.pack()?;
        let result = config.apply(!fail)?;
        let status = if fail { "failed" } else { "passed" };
        assert_eq!(result["schema"], "airlock.config-apply.v1");
        assert_eq!(result["status"], status);
        assert_eq!(result["candidate_commit"], config.data["commit"]);
        let applied = &result["system_apply_result"];
        assert_eq!(applied["status"], status);
        assert_eq!(applied["profile_status"], "passed");
        assert_eq!(
            applied["host_system_status"],
            if fail { "skipped" } else { "passed" }
        );
        let source = config.path(&format!(
            "state/sources/{commit}/dom0.nixos.system",
            commit = config.data["commit"].as_str().unwrap()
        ));
        assert_eq!(fs::read_link(config.path("profiles/host-system"))?, source);
        assert_eq!(
            fs::read_to_string(config.path("state/profile-activated"))?,
            "activated\n"
        );
        assert_eq!(result["deployment"]["status"], status);
        assert_eq!(result["deployment_archive"]["status"], "passed");
        assert_eq!(config.read("state/last-deployment.json")?["status"], status);
        assert_eq!(
            config.read("state/last-config-apply.json")?["status"],
            status
        );
        assert!(config.path("state/current-topology.json").is_file());
        let mut archived = Vec::new();
        for member in tar::Archive::new(fs::File::open(config.path("deployment.tar"))?).entries()? {
            archived.push(member?.path()?.into_owned());
        }
        for member in [
            "deployment.json",
            "system-apply.json",
            "systemd-actions.json",
            "topology.json",
            "system.json",
        ] {
            assert!(
                archived.iter().any(|path| path.ends_with(member)),
                "missing {member}"
            );
        }
        let vault = config.vault()?;
        assert_eq!(vault["deployment_status"], status);
        assert_eq!(
            config.path("vault-archive/latest-deployment").exists(),
            !fail
        );
        assert_eq!(config.path("vault-archive/current").exists(), !fail);
        if fail {
            assert_eq!(result["reason"], "system apply failed");
            assert_eq!(
                applied["reason"],
                "dom0 NixOS switch-to-configuration switch failed"
            );
            assert!(!config.path("state/current-deployment.json").exists());
            assert!(!config.path("state/current-config-apply.json").exists());
            assert!(!config.path("current-system").exists());
        } else {
            assert_eq!(
                config.read("state/current-deployment.json")?["status"],
                "passed"
            );
            assert_eq!(applied["host_system_switch_action"], action);
            assert_eq!(applied["reboot_required"], action == "boot");
            assert_eq!(
                fs::read_to_string(config.path("state/host-switch-action"))?,
                action
            );
            if action == "switch" {
                assert_eq!(fs::read_link(config.path("current-system"))?, source);
            } else {
                assert!(!config.path("current-system").exists());
            }
            let calls = config.calls()?;
            for operation in ["restart", "reload"] {
                assert!(
                    calls
                        .iter()
                        .any(|call| call["command"] == "systemctl" && call["args"][0] == operation)
                );
            }
        }
    }
    Ok(())
}

/// Setup failure before a receipt is published archives fresh unknown/skipped outcomes, never earlier success.
#[test]
fn configuration_failed_setup_archives_fresh_outcomes() -> Result<()> {
    let mut config = Config::new()?;
    config.pack()?;
    let previous = config.apply(true)?;
    config.data["fail"] = json!("airlock-setup-config");
    let failed = config.apply(false)?;
    assert_ne!(failed["deployment_id"], previous["deployment_id"]);
    assert_eq!(failed["setup"]["status"], "unknown");
    assert_eq!(failed["system_apply_result"]["status"], "skipped");
    assert_eq!(failed["deployment"]["status"], "failed");
    assert_eq!(failed["deployment_archive"]["status"], "passed");
    assert_eq!(
        config.read("state/current-deployment.json")?["status"],
        "passed"
    );
    let payload = fs::read(config.path("deployment.tar"))?;
    for change_apply in [false, true] {
        let mut output = tar::Builder::new(Vec::new());
        for entry in tar::Archive::new(payload.as_slice()).entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            if path == std::path::Path::new("deployment.json")
                || (change_apply && path == std::path::Path::new("apply.json"))
            {
                let mut document: Value = serde_json::from_slice(&content)?;
                document["status"] = json!("passed");
                content = serde_json::to_vec(&document)?;
            }
            let mut header = entry.header().clone();
            header.set_size(content.len() as u64);
            header.set_cksum();
            output.append_data(&mut header, path, content.as_slice())?;
        }
        fs::write(config.path("deployment.tar"), output.into_inner()?)?;
        assert!(
            config.vault().is_err(),
            "failed setup accepted as a current deployment"
        );
        assert!(!config.path("vault-archive/latest-deployment").exists());
        assert!(!config.path("vault-archive/current").exists());
    }

    fs::write(config.path("deployment.tar"), payload)?;
    let vault = config.vault()?;
    assert_eq!(vault["deployment_status"], "failed");
    assert!(!config.path("vault-archive/latest-deployment").exists());
    assert!(!config.path("vault-archive/current").exists());
    Ok(())
}

/// Domain references are validated before setup, including the existing stock firewall's upstream dependency.
#[test]
fn configuration_domain_references_gate_archive_and_apply() -> Result<()> {
    for archive in [false, true] {
        for (domains, reason) in [
            (vec!["dom0", "dev"], "topology references missing domains"),
            (
                vec!["dom0", "dev", "sys-firewall"],
                "topology references unusable network domains",
            ),
            (vec!["dom0", "dev", "sys-firewall", "sys-net"], ""),
        ] {
            let mut config = Config::new()?;
            config.topology["domains"]["dev"] = json!({"class": "StandaloneVM", "template": null, "label": "green", "netvm": "sys-firewall", "prefs": {}, "backend": {"type": "qubes-standalone", "template": null}});
            config.data["domains"] = json!(domains);
            config.pack()?;
            let result = if archive {
                let output = common::run(
                    &mut config.command(
                        "archive",
                        &[
                            "--candidate-tar",
                            config.path("staged.tar").to_str().unwrap(),
                            "--validate-only",
                        ],
                    )?,
                    None,
                )?;
                assert_eq!(output.status.success(), reason.is_empty());
                config.read("logs/config-archive.json")?
            } else {
                config.apply(reason.is_empty())?
            };
            assert_eq!(
                result["status"],
                if reason.is_empty() {
                    "passed"
                } else {
                    "failed"
                }
            );
            let references = &result["domain_references_result"];
            assert_eq!(references["status"], result["status"]);
            if reason.is_empty() {
                assert!(
                    references["references"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|reference| reference["kind"] == "netvm"
                            && reference["domain"] == "dev"
                            && reference["target"] == "sys-firewall")
                );
                if !archive {
                    assert!(config.path("setup-topology.json").is_file());
                }
            } else {
                assert_eq!(result["reason"], reason);
                assert!(!config.path("setup-topology.json").exists());
                let missing = references["missing"].as_array().unwrap();
                assert!(missing.iter().any(|reference| reference["target"]
                    == if reason.contains("missing domains") {
                        "sys-firewall"
                    } else {
                        "sys-net"
                    }));
            }
        }
    }
    Ok(())
}

/// Source references build only their selected outputs, and direct sources survive temporary checkout cleanup.
#[test]
fn configuration_materialization_preserves_template_and_closure_boot_sources() -> Result<()> {
    for references in [false, true] {
        let mut config = Config::new()?;
        let source_root = if references {
            config.path("built")
        } else {
            config.path("candidate/sources")
        };
        for name in ["system", "kernel", "root", "store", "template"] {
            fs::create_dir_all(source_root.join(name))?;
        }
        fs::write(source_root.join("system/init"), "init")?;
        fs::write(source_root.join("root/root.img"), "root image")?;
        fs::write(source_root.join("kernel/vmlinuz"), "kernel")?;
        fs::write(
            source_root.join("store/store-paths"),
            "/nix/store/test-system\n",
        )?;
        fs::write(source_root.join("store/manifest.json"), "{}")?;
        fs::write(source_root.join("template/root.img"), "template disk")?;
        fs::write(
            source_root.join("template/template.conf"),
            "name=test-template\n",
        )?;
        let artifact = |name: &str| {
            if references {
                json!({"source": null, "source_ref": {"type": "flake-output", "attr": format!("packages.x86_64-linux.{name}"), "subpath": ""}})
            } else {
                json!({"source": format!("sources/{name}"), "source_ref": null})
            }
        };
        let mut backend = json!({"type": "nix-closure", "kernel": {"name": "fixture-kernel"}, "state": {"persistent": "work-private"}});
        for name in ["system", "root", "store", "kernel"] {
            backend[name] = artifact(name);
        }
        backend["kernel"]["name"] = json!("fixture-kernel");
        backend["store"]["mode"] = json!("shared-closure");
        backend["store"]["device"] = json!("/dev/xvde");
        backend["store"]["read_only"] = json!(true);
        config.topology["domains"]["work"] = json!({"class": "StandaloneVM", "template": null, "label": "blue", "netvm": null, "backend": backend});
        let mut template = artifact("template");
        template["import"] = json!(true);
        template["skip_start"] = json!(true);
        config.topology["templates"]["test-template"] = template;
        let mut kernel = artifact("kernel");
        kernel["install"] = json!(true);
        config.topology["kernels"]["fixture-kernel"] = kernel;
        if references {
            for name in ["system", "root", "store", "template", "kernel"] {
                config.data["outputs"][format!("packages.x86_64-linux.{name}")] =
                    json!(source_root.join(name));
            }
        } else {
            // Direct fixture sources are part of the committed bundle selected by the validation receipt.
            common::run(
                Command::new("git")
                    .arg("-C")
                    .arg(config.path("candidate"))
                    .args(["add", "."]),
                None,
            )?
            .checked()?;
            common::run(
                Command::new("git")
                    .arg("-C")
                    .arg(config.path("candidate"))
                    .args([
                        "-c",
                        "user.name=Configuration test",
                        "-c",
                        "user.email=test@example.invalid",
                        "-c",
                        "commit.gpgsign=false",
                        "commit",
                        "-qm",
                        "Committed artifact sources",
                    ]),
                None,
            )?
            .checked()?;
        }
        config.pack()?;
        config.apply(true)?;
        let topology = config.read("setup-topology.json")?;
        for name in ["system", "root", "store"] {
            let source = topology["domains"]["work"]["backend"][name]["source"]
                .as_str()
                .unwrap();
            assert!(std::path::Path::new(source).exists());
            assert!(topology["domains"]["work"]["backend"][name]["source_ref"].is_null());
            if !references {
                assert!(source.starts_with(config.path("state/sources").to_str().unwrap()));
            }
        }
        assert!(topology["templates"]["test-template"]["source"].is_string());
        assert!(topology["templates"]["test-template"]["source_ref"].is_null());
        let builds = config.read("logs/config-apply-nix-domain-builds.json")?;
        assert_eq!(
            builds["builds"].as_array().unwrap().len(),
            if references { 4 } else { 0 }
        );
        let system_builds = config.read("logs/config-apply-system-builds.json")?;
        assert_eq!(system_builds["builds"], json!([]));
        assert_eq!(
            config.read("logs/config-system-apply.json")?["profile_status"],
            "skipped"
        );
    }
    Ok(())
}

/// Boot verification compares the active system with the expected generation, rather than trusting a staged profile link.
#[test]
fn configuration_boot_verification_requires_the_running_generation() -> Result<()> {
    for running in ["/nix/store/new-system", "/nix/store/old-system"] {
        let config = Config::new()?;
        let pending = running.ends_with("old-system");
        let selected = config.path("new-system");
        let observed = config.path(if pending { "old-system" } else { "new-system" });
        fs::create_dir_all(&selected)?;
        fs::create_dir_all(&observed)?;
        for (link, target) in [
            (config.path("profiles/host-system"), &selected),
            (config.path("current-system"), &observed),
        ] {
            fs::create_dir_all(link.parent().unwrap())?;
            let _ = fs::remove_file(&link);
            std::os::unix::fs::symlink(target, &link)?;
        }
        config.write(
            "state/last-apply.json",
            &json!({"host_system_source":selected}),
        )?;
        config.write("state/current-plan.json",&json!({"reboot_required":true,"reboot_actions":[{"type":"dom0-nixos-system","switch_action":"boot","reason":"test host generation"}]}))?;
        let output = common::run(
            config
                .command("boot-verify", &[])?
                .env("AIRLOCK_EXPECTED_HOST_SYSTEM_SOURCE", &selected),
            None,
        )?;
        assert_eq!(output.status.success(), !pending);
        let result = config.read("logs/config-boot-verify.json")?;
        assert_eq!(result["schema"], "airlock.config-boot-verify.v1");
        assert_eq!(result["pending_reboot"]["required"], pending);
        if pending {
            assert_eq!(
                result["reason"],
                "running system does not point at expected generation"
            );
        } else {
            for field in [
                "expected_host_system_source",
                "last_apply_host_system_source",
                "host_system_profile_resolved_target",
                "run_current_system_resolved_target",
            ] {
                assert_eq!(result[field], json!(observed));
            }
        }
    }
    Ok(())
}

/// Control treats incoming archives as inert data and dispatches validation and activation through separate fixed helpers.
#[test]
fn configuration_control_cannot_execute_submitted_archive_content() -> Result<()> {
    let mut config = Config::new()?;
    config.pack()?;
    config.data["control"] = json!(true);
    let mut archive = tar::Builder::new(Vec::new());
    let malicious = format!(
        "touch {marker}\n",
        marker = config.path("must-not-execute").display()
    );
    let manifest = serde_json::to_vec(&json!({"payload": malicious}))?;
    for (name, content) in [
        ("config.bundle", fs::read(config.path("config.bundle"))?),
        ("manifest.json", manifest),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        archive.append_data(&mut header, name, content.as_slice())?;
    }
    let payload = archive.into_inner()?;
    for action in ["check", "apply"] {
        let result = common::json(
            &mut config.command("control", &[action])?,
            Some(payload.clone()),
        )?;
        assert_eq!(result["status"], "passed");
        assert_eq!(config.path("control-activated").exists(), action == "apply");
        assert_eq!(fs::read(config.path("control-candidate.tar"))?, payload);
        assert!(!config.path("must-not-execute").exists());
    }
    assert!(config.calls()?.iter().any(|call| {
        call["command"] == "airlock-config-archive-candidate"
            && call["args"]
                .as_array()
                .unwrap()
                .contains(&json!("--validate-only"))
    }));
    fs::remove_file(config.path("control-activated"))?;
    config.data["control_failure"] = json!(true);
    let output = common::run(&mut config.command("control", &["apply"])?, Some(payload))?;
    assert!(!output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout)?["status"],
        "failed"
    );
    assert!(!config.path("control-activated").exists());
    assert!(!config.path("must-not-execute").exists());
    let mut invalid = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(0);
    header.set_mode(0o644);
    header.set_cksum();
    invalid.append_data(&mut header, "unexpected.tar", std::io::empty())?;
    let output = common::run(
        &mut config.command("control", &["check"])?,
        Some(invalid.into_inner()?),
    )?;
    assert!(!output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout)?["schema"],
        "airlock.config-control.v1"
    );
    Ok(())
}

/// Host rollback follows its recorded previous generation and never falls through to an unrelated profile rollback.
#[test]
fn configuration_control_rollback_uses_the_recorded_host_generation() -> Result<()> {
    let config = Config::new()?;
    let previous = config.path("candidate/dom0-system");
    let current = config.path("current-host-system");
    fs::create_dir(&current)?;
    symlink(&current, config.path("profiles/host-system"))?;
    symlink(&current, config.path("current-system"))?;
    fs::write(
        config.path("state/previous-host-system"),
        previous.as_os_str().as_encoded_bytes(),
    )?;
    let result = common::json(&mut config.command("control", &["rollback"])?, None)?;
    assert_eq!(result["schema"], "airlock.config-rollback.v1");
    assert_eq!(result["status"], "passed");
    assert_eq!(result["method"], "state");
    assert_eq!(result["target_system"], json!(previous));
    assert_eq!(
        fs::read_link(config.path("profiles/host-system"))?,
        previous
    );
    assert_eq!(fs::read_link(config.path("current-system"))?, previous);
    assert_eq!(
        fs::read_to_string(config.path("state/host-switch-action"))?,
        "switch"
    );
    assert!(
        !config
            .calls()?
            .iter()
            .any(|call| call["command"] == "nix-env")
    );
    Ok(())
}

/// Status distinguishes boot declarations, applied topology, pending VM restarts and staged versus running host generations.
#[test]
fn configuration_status_reports_applied_state_and_consumes_boot_intent() -> Result<()> {
    let mut config = Config::new()?;
    config.data["qvm_rows"] = json!([
        "sys-config-vault|StandaloneVM|Running||black",
        "work|AppVM|Halted|template|green",
        "other|AppVM|Halted|template|red"
    ]);
    config.topology["domains"] = json!({"sys-config-vault": {"class": "StandaloneVM", "template": null, "backend": {"type": "nix-closure"}}, "work": {"class": "AppVM", "template": "template"}});
    config.topology["qrexec_policy"]["rules"] = json!([{"service": "airlock.PolicyProbe", "argument": "*", "source": "dom0", "target": "sys-config-vault", "action": "allow"}]);
    config.write("generation/qubes-topology.json", &config.topology)?;
    symlink("../generation", config.path("etc/static"))?;
    symlink(
        "static/qubes-topology.json",
        config.path("etc/qubes-topology.json"),
    )?;
    let mut applied = config.topology.clone();
    applied["domains"]["from-current"] = json!({"class": "AppVM", "template": "template"});
    config.write("state/current-topology.json", &applied)?;
    config.write("logs/config-apply.json", &json!({"schema": "airlock.config-apply.v1", "status": "passed", "candidate_commit": "abc123"}))?;
    let restart = json!({"type": "domain-pref", "action": "restart", "domain": "sys-config-vault", "field": "memory", "reason": "running VM requires restart"});
    config.write("logs/setup-config.json", &json!({"schema": "airlock.setup-config.v1", "status": "passed", "restart_required": true, "restart_actions": [restart], "live_actions": []}))?;
    config.write(
        "state/current-system.json",
        &json!({"schema": "airlock.system.v1", "dom0": {"profile": {"message": "from-test"}}}),
    )?;
    config.write("state/current-plan.json", &json!({"live_actions": [], "reboot_actions": [], "reboot_required": false, "reboot_reasons": []}))?;
    let host = config.path("host-system");
    let old = config.path("old-host-system");
    let profile = config.path("airlock-profile");
    for path in [&host, &old, &profile] {
        fs::create_dir(path)?;
    }
    symlink(&host, config.path("profiles/host-system"))?;
    symlink(&host, config.path("current-system"))?;
    symlink(&profile, config.path("profiles/airlock-system"))?;
    config.write("state/last-apply.json", &json!({"schema": "airlock.system-apply.v1", "status": "passed", "host_system_status": "passed", "host_system_source": host, "host_system_switch_action": "switch", "reboot_required": false}))?;
    for (name, status) in [
        ("current-deployment.json", "passed"),
        ("last-deployment.json", "failed"),
        ("current-config-apply.json", "passed"),
        ("last-config-apply.json", "failed"),
    ] {
        config.write(
            &format!("state/{name}"),
            &json!({"status": status, "deployment_id": name}),
        )?;
    }
    fs::create_dir(config.path("policy.d"))?;
    fs::write(
        config.path("policy.d/90-airlock.policy"),
        r"# Managed policy
    airlock.PolicyProbe * dom0 sys-config-vault allow
    airlock.PolicyProbe * @anyvm @anyvm deny
",
    )?;
    let status = || -> Result<Value> { common::json(&mut config.command("status", &[])?, None) };
    let result = status()?;
    assert_eq!(result["schema"], "airlock.config-status.v1");
    assert_eq!(result["status"], "applied");
    assert_eq!(result["topology"]["source"], "current");
    assert_eq!(result["topology"]["boot_present"], true);
    assert_eq!(result["topology"]["current_present"], true);
    assert!(
        result["topology"]["domain_names"]
            .as_array()
            .unwrap()
            .contains(&json!("from-current"))
    );
    assert_eq!(result["qvm"]["domains"].as_array().unwrap().len(), 3);
    assert_eq!(
        result["qvm"]["managed_domains"].as_array().unwrap().len(),
        2
    );
    assert_eq!(result["qrexec_policy"]["rule_count"], 2);
    assert_eq!(result["pending_vm_restarts"]["required"], true);
    assert_eq!(
        result["pending_vm_restarts"]["domains"],
        json!(["sys-config-vault"])
    );
    assert_eq!(result["setup_source"], "log");
    assert_eq!(result["pending_reboot"]["required"], false);
    assert_eq!(result["host_generation"]["active"], true);
    assert_eq!(result["host_generation"]["staged_source"], "");
    assert_eq!(result["host_generation"]["current_source"], json!(host));

    config.write("state/current-plan.json", &json!({"live_actions": [], "reboot_actions": [{"type": "dom0-nixos-system", "action": "boot", "reason": "next host boot"}], "reboot_required": true, "reboot_reasons": ["next host boot"]}))?;
    fs::remove_file(config.path("current-system"))?;
    symlink(&old, config.path("current-system"))?;
    let reboot = status()?;
    assert_eq!(reboot["pending_reboot"]["required"], true);
    assert_eq!(
        reboot["pending_reboot"]["reasons"],
        json!(["next host boot"])
    );
    assert_eq!(reboot["host_generation"]["active"], false);
    assert_eq!(reboot["host_generation"]["current_source"], json!(old));
    assert_eq!(reboot["host_generation"]["staged_source"], json!(host));
    assert_eq!(reboot["system"]["pending_reboot"]["required"], true);
    fs::remove_file(config.path("current-system"))?;
    symlink(&host, config.path("current-system"))?;
    let consumed = status()?;
    assert_eq!(consumed["pending_reboot"]["required"], false);
    assert_eq!(consumed["pending_reboot"]["actions"], json!([]));
    assert_eq!(consumed["pending_reboot"]["reasons"], json!([]));
    assert_eq!(consumed["host_generation"]["active"], true);
    assert_eq!(consumed["host_generation"]["staged_source"], "");
    assert_eq!(consumed["system"]["current_plan"]["reboot_required"], true);

    fs::remove_file(config.path("logs/setup-config.json"))?;
    config.write("state/current-setup.json", &json!({"schema": "airlock.setup-config.v1", "status": "passed", "restart_required": false, "restart_actions": []}))?;
    let current = status()?;
    assert_eq!(current["setup_source"], "current");
    assert!(current["setup_log"].is_null());
    assert_eq!(current["current_setup"]["status"], "passed");
    assert_eq!(current["pending_vm_restarts"]["required"], false);
    for field in ["actions", "domains", "reasons"] {
        assert_eq!(current["pending_vm_restarts"][field], json!([]));
    }
    Ok(())
}

/// Real staging export/import preserves the selected commit and refuses to overwrite editable work.
#[test]
fn configuration_staging_import_preserves_dirty_and_divergent_work() -> Result<()> {
    let mut fixture = Config::new()?;
    fixture.pack()?;
    let settings = fixture.path("configuration-runtime.json");
    let repo = fixture.path("candidate");
    let mut contract = json!({"user":"user","repo":repo,"archive":fixture.path("archive"),"validation":fixture.path("validation"),"inputs":fixture.path("inputs"),"store":fixture.path("store"),"public_inputs":fixture.path("public-inputs"),"seed":repo,"nix":"nix","role":"sys-config-staging","staging_path":std::env::var("PATH")?});
    fs::write(&settings, serde_json::to_vec(&contract)?)?;
    let native = common::binary("AIRLOCK_VM_BIN", "airlock-vm-service");
    let command = |action: &str| {
        let mut command = Command::new(&native);
        command
            .args(["configuration", "--config"])
            .arg(&settings)
            .arg(action);
        command
    };
    let exported = common::run(
        command("staging-export").env("AIRLOCK_CONFIG_REPO_PATH", ""),
        None,
    )?
    .checked()?;
    let work = fixture.path("editable");
    contract["repo"] = json!(work);
    fs::write(&settings, serde_json::to_vec(&contract)?)?;
    let imported = common::json(&mut command("staging-import"), Some(exported.clone()))?;
    assert_eq!(imported["import_status"], "imported");
    assert_eq!(imported["current_commit"], fixture.data["commit"]);
    fs::write(work.join("user-work"), "keep this edit\n")?;
    let dirty = common::json(&mut command("staging-import"), Some(exported.clone()))?;
    assert_eq!(dirty["import_status"], "skipped");
    assert_eq!(
        fs::read_to_string(work.join("user-work"))?,
        "keep this edit\n"
    );
    common::run(
        Command::new("git")
            .arg("-C")
            .arg(&work)
            .args(["add", "user-work"]),
        None,
    )?
    .checked()?;
    common::run(
        Command::new("git").arg("-C").arg(&work).args([
            "-c",
            "user.name=Staging test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-qm",
            "Retain user work",
        ]),
        None,
    )?
    .checked()?;
    let divergent = common::json(&mut command("staging-import"), Some(exported))?;
    assert_eq!(divergent["import_status"], "skipped");
    assert!(
        divergent["reason"]
            .as_str()
            .unwrap()
            .contains("different committed worktree")
    );
    assert_eq!(
        fs::read_to_string(work.join("user-work"))?,
        "keep this edit\n"
    );
    Ok(())
}
