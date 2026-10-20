//! Explicit configuration-VM preflight, source staging, apply and post-apply checks in native Rust.

use super::common::{self, Session};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{fs, process::Command};

/// Representation retained by the existing smoke-report field.
enum Evidence {
    /// Captured command output retained as text.
    Text,
    /// Decoded structured result, or null when no valid result exists.
    Json,
    /// Evidence filename rather than its contents.
    Path,
}

/// Existing report fields, kept as data rather than repeated shell serialization.
const EVIDENCE: &[(&str, &str, Evidence)] = &[
    (
        "staging_uname",
        "config-vms-config-staging-uname.log",
        Evidence::Text,
    ),
    (
        "vault_uname",
        "config-vms-config-vault-uname.log",
        Evidence::Text,
    ),
    (
        "validator_uname",
        "config-vms-config-validator-uname.log",
        Evidence::Text,
    ),
    (
        "created_qube_uname",
        "config-vms-created-qube-uname.log",
        Evidence::Text,
    ),
    (
        "staging_user",
        "config-vms-config-staging-user.log",
        Evidence::Text,
    ),
    (
        "vault_user",
        "config-vms-config-vault-user.log",
        Evidence::Text,
    ),
    (
        "validator_user",
        "config-vms-config-validator-user.log",
        Evidence::Text,
    ),
    (
        "created_qube_user",
        "config-vms-created-qube-user.log",
        Evidence::Text,
    ),
    (
        "staging_backend",
        "config-vms-config-staging-backend.log",
        Evidence::Text,
    ),
    (
        "vault_backend",
        "config-vms-config-vault-backend.log",
        Evidence::Text,
    ),
    (
        "validator_backend",
        "config-vms-config-validator-backend.log",
        Evidence::Text,
    ),
    (
        "created_qube_backend",
        "config-vms-created-qube-backend.log",
        Evidence::Text,
    ),
    (
        "staging_store_volume",
        "config-vms-config-staging-store-volume.log",
        Evidence::Text,
    ),
    (
        "vault_store_volume",
        "config-vms-config-vault-store-volume.log",
        Evidence::Text,
    ),
    (
        "validator_store_volume",
        "config-vms-config-validator-store-volume.log",
        Evidence::Text,
    ),
    (
        "created_qube_store_volume",
        "config-vms-created-qube-store-volume.log",
        Evidence::Text,
    ),
    (
        "staging_store_mount",
        "config-vms-config-staging-store-mount.log",
        Evidence::Text,
    ),
    (
        "vault_store_mount",
        "config-vms-config-vault-store-mount.log",
        Evidence::Text,
    ),
    (
        "validator_store_mount",
        "config-vms-config-validator-store-mount.log",
        Evidence::Text,
    ),
    (
        "created_qube_store_mount",
        "config-vms-created-qube-store-mount.log",
        Evidence::Text,
    ),
    (
        "staging_persistence",
        "config-vms-config-staging-persistence.log",
        Evidence::Text,
    ),
    (
        "vault_persistence",
        "config-vms-config-vault-persistence.log",
        Evidence::Text,
    ),
    (
        "validator_persistence",
        "config-vms-config-validator-persistence.log",
        Evidence::Text,
    ),
    (
        "created_qube_persistence",
        "config-vms-created-qube-persistence.log",
        Evidence::Text,
    ),
    (
        "staging_appmenus_log",
        "config-vms-config-staging-appmenus.log",
        Evidence::Text,
    ),
    (
        "vault_appmenus_log",
        "config-vms-config-vault-appmenus.log",
        Evidence::Text,
    ),
    (
        "validator_appmenus_log",
        "config-vms-config-validator-appmenus.log",
        Evidence::Text,
    ),
    (
        "staging_init_log",
        "config-vms-config-staging-init.log",
        Evidence::Text,
    ),
    (
        "validator_init_log",
        "config-vms-config-validator-init.log",
        Evidence::Text,
    ),
    (
        "current_config_bootstrap",
        "config-vms-config-current-bootstrap.log",
        Evidence::Json,
    ),
    (
        "staging_update_log",
        "config-vms-config-staging-update.log",
        Evidence::Text,
    ),
    (
        "staging_update_script",
        "config-vms-config-staging-update.json",
        Evidence::Path,
    ),
    (
        "staging_repo_status",
        "config-vms-config-staging-repo-status.log",
        Evidence::Text,
    ),
    (
        "vault_repo_status",
        "config-vms-config-vault-repo-status.log",
        Evidence::Text,
    ),
    ("dom0_system", "config-vms-dom0-system.json", Evidence::Json),
    (
        "dom0_system_plan",
        "config-vms-dom0-system-plan.json",
        Evidence::Json,
    ),
    (
        "dom0_system_profile",
        "config-vms-dom0-profile.json",
        Evidence::Json,
    ),
    (
        "dom0_system_apply",
        "config-vms-dom0-system-apply.json",
        Evidence::Json,
    ),
    (
        "dom0_system_profile_link",
        "config-vms-dom0-system-profile-link.log",
        Evidence::Text,
    ),
    (
        "dom0_host_config_message",
        "config-vms-dom0-host-config-message.log",
        Evidence::Text,
    ),
    (
        "dom0_extra_module_message",
        "config-vms-dom0-extra-module-message.log",
        Evidence::Text,
    ),
    (
        "dom0_host_configuration_message",
        "config-vms-dom0-host-configuration-message.log",
        Evidence::Text,
    ),
    (
        "dom0_extra_module_restarted",
        "config-vms-dom0-extra-module-restarted.log",
        Evidence::Text,
    ),
    (
        "dom0_systemd_actions",
        "config-vms-dom0-systemd-actions.json",
        Evidence::Json,
    ),
    (
        "dom0_host_system_profile_link",
        "config-vms-dom0-host-system-profile-link.log",
        Evidence::Text,
    ),
    (
        "dom0_current_system_link",
        "config-vms-dom0-current-system-link.log",
        Evidence::Text,
    ),
    (
        "config_status",
        "config-vms-config-status.json",
        Evidence::Json,
    ),
    ("archive", "config-archive.json", Evidence::Json),
    ("archive_log", "config-vms-archive.log", Evidence::Text),
    ("apply", "config-apply.json", Evidence::Json),
    ("apply_log", "config-vms-apply.log", Evidence::Text),
    (
        "applied_topology",
        "config-vms-applied-topology.json",
        Evidence::Json,
    ),
    (
        "qrexec_policy_file",
        "config-vms-qrexec-policy.log",
        Evidence::Text,
    ),
    (
        "qrexec_policy_probe",
        "config-vms-qrexec-policy-probe.log",
        Evidence::Text,
    ),
    (
        "applied_feature",
        "config-vms-applied-feature.log",
        Evidence::Text,
    ),
    (
        "created_qube_feature",
        "config-vms-created-qube-feature.log",
        Evidence::Text,
    ),
    (
        "vault_archive_tree",
        "config-vms-config-vault-archive-tree.log",
        Evidence::Text,
    ),
    (
        "vault_deployment_tree",
        "config-vms-config-vault-deployment-tree.log",
        Evidence::Text,
    ),
    (
        "command_log",
        "config-vms-smoke.commands.log",
        Evidence::Path,
    ),
    ("qvm_ls_log", "config-vms-qvm-ls.log", Evidence::Path),
    ("journal_log", "config-vms-journal.log", Evidence::Path),
    ("libxl_log", "config-vms-libxl.log", Evidence::Path),
    ("qvm_prefs_log", "config-vms-qvm-prefs.log", Evidence::Path),
    (
        "staging_console_log",
        "config-vms-$STAGING-console.log",
        Evidence::Path,
    ),
    (
        "vault_console_log",
        "config-vms-$VAULT-console.log",
        Evidence::Path,
    ),
    (
        "validator_console_log",
        "config-vms-$VALIDATOR-console.log",
        Evidence::Path,
    ),
    (
        "created_qube_console_log",
        "config-vms-$CREATED_QUBE-console.log",
        Evidence::Path,
    ),
];

/// Append a typed smoke source and inputs as data, then commit them with existing guest tools.
pub(super) fn stage(
    session: &mut Session,
    guest: &str,
    inputs: &Value,
    message: &str,
) -> Result<()> {
    let user = common::setting("AIRLOCK_QUBE_USER", "user");
    ensure!(
        !user.is_empty()
            && user
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte)),
        "invalid smoke user"
    );
    let repo = format!("/home/{user}/airlock-config");
    let source = common::path("AIRLOCK_CONFIG_SMOKE_SOURCE", "");
    common::save(
        &session.file("config-vms-config-staging-update.json"),
        &json!({"source":source,"inputs":inputs,"commit_message":message}),
    )?;
    ensure!(
        source.is_file(),
        "missing Rust candidate source for smoke update"
    );
    session.remote(
        guest,
        "tee",
        &[&format!("{repo}/src/main.rs")],
        Some(fs::read(&source)?),
        "config-vms-staging-source.log",
    )?;
    session.remote(
        guest,
        "tee",
        &[&format!("{repo}/smoke-inputs.json")],
        Some(serde_json::to_vec(inputs)?),
        "config-vms-staging-inputs.log",
    )?;
    session.remote(
        guest,
        "nix",
        &[
            "--extra-experimental-features",
            "nix-command",
            "--extra-experimental-features",
            "flakes",
            "flake",
            "lock",
            "--impure",
            "--offline",
            &format!("path:{repo}"),
        ],
        None,
        "config-vms-config-staging-update.log",
    )?;
    session.remote(
        guest,
        "git",
        &[
            "-C",
            &repo,
            "add",
            "src",
            "Cargo.toml",
            "Cargo.lock",
            ".cargo",
            "config-inputs.json",
            "flake.lock",
            "smoke-inputs.json",
        ],
        None,
        "config-vms-staging-add.log",
    )?;
    session.remote(
        guest,
        "git",
        &[
            "-C",
            &repo,
            "-c",
            "user.name=Airlock Config Smoke",
            "-c",
            "user.email=airlock-config-smoke@localhost",
            "commit",
            "-m",
            message,
        ],
        None,
        "config-vms-config-staging-update.log",
    )?;
    Ok(())
}

/// Assert the native backend identity and read-only Qubes store volume before guest startup.
fn backend(session: &mut Session, guest: &str, label: &str) -> Result<()> {
    let classes = session.command(
        "qvm-ls",
        &["--raw-data", "--fields", "NAME,CLASS", "--no-spinner"],
        "config-vms-classes.log",
    )?;
    ensure!(
        classes
            .lines()
            .any(|line| line == format!("{guest}|StandaloneVM")),
        "{guest} is not a StandaloneVM"
    );
    let mut evidence = "class=StandaloneVM\n".to_owned();
    for (field, feature, expected) in [
        ("backend", "airlock-backend", "nix-closure"),
        ("store_device", "airlock-store-device", "/dev/xvde"),
        ("store_read_only", "airlock-store-read-only", "True"),
    ] {
        let value = session.command("qvm-features", &[guest, feature], "config-vms-feature.log")?;
        evidence.push_str(&format!("{field}={value}\n"));
        ensure!(value == expected, "{guest} has unexpected {field}: {value}");
    }
    let netvm = session.command("qvm-prefs", &[guest, "netvm"], "config-vms-netvm.log")?;
    evidence.push_str(&format!("netvm={netvm}\n"));
    ensure!(
        ["", "none", "None"].contains(&netvm.as_str()),
        "{guest} unexpectedly has a NetVM"
    );
    fs::write(
        session.file(&format!("config-vms-{label}-backend.log")),
        evidence,
    )?;
    let volume = session.command(
        "qvm-volume",
        &["info", &format!("{guest}:store")],
        &format!("config-vms-{label}-store-volume.log"),
    )?;
    ensure!(
        volume.lines().any(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some("rw") && words.next() == Some("False")
        }),
        "{guest} store volume is not read-only"
    );
    Ok(())
}

/// Check a private-home marker across a complete upstream shutdown/start cycle.
fn persistence(session: &mut Session, guest: &str, label: &str, message: &str) -> Result<()> {
    let user = common::setting("AIRLOCK_QUBE_USER", "user");
    let marker = format!("/home/{user}/.airlock-persistence-{guest}");
    session.remote(
        guest,
        "tee",
        &[&marker],
        Some(format!("{message}\n").into_bytes()),
        &format!("config-vms-{guest}-persistence-write.log"),
    )?;
    session.remote(guest, "sync", &[], None, "config-vms-sync.log")?;
    session.restart(guest)?;
    session.runtime_store(
        guest,
        &format!("config-vms-{guest}-store-mount-after-restart.log"),
    )?;
    let observed = session.remote(
        guest,
        "cat",
        &[&marker],
        None,
        &format!("config-vms-{label}-persistence.log"),
    )?;
    ensure!(
        observed == message,
        "{guest} persistent home state did not survive restart"
    );
    Ok(())
}

/// Recognize the complete materialized closure descriptor consumed by setup, independently of observed VM state.
fn closure(topology: &Value, name: &str) -> bool {
    let domain = &topology["domains"][name];
    let backend = &domain["backend"];
    domain["class"] == "StandaloneVM"
        && domain["template"].is_null()
        && backend["type"] == "nix-closure"
        && backend["template"].is_null()
        && ["system", "root", "store"].iter().all(|key| {
            backend[*key]["source"]
                .as_str()
                .is_some_and(|source| source.starts_with("/nix/store/"))
                && backend[*key]["source_ref"].is_null()
        })
        && backend["root"]["source"]
            .as_str()
            .is_some_and(|source| source.ends_with("/root.img"))
        && backend["store"]["mode"] == "shared-closure"
        && backend["store"]["view_backend"] == "erofs-objectmap"
        && backend["store"]["blob_device"] == "/dev/xvdf"
        && backend["store"]["mountpoint"] == "/nix/store"
        && backend["store"]["device"] == "/dev/xvde"
        && backend["store"]["read_only"] == true
}

/// Check the applied host contract, selected generation and live-versus-boot activation semantics.
fn host(
    session: &mut Session,
    message: &str,
    action: &str,
    reason: &str,
) -> Result<(Value, Value)> {
    let state = common::path("AIRLOCK_SYSTEM_STATE_DIR", "/var/lib/airlock/system");
    let mut values = Vec::new();
    for (file, log) in [
        ("current-system.json", "config-vms-dom0-system.json"),
        ("current-plan.json", "config-vms-dom0-system-plan.json"),
        ("current-profile.json", "config-vms-dom0-profile.json"),
        ("last-apply.json", "config-vms-dom0-system-apply.json"),
        (
            "last-systemd-actions.json",
            "config-vms-dom0-systemd-actions.json",
        ),
    ] {
        let value = fs::read(state.join(file))?;
        fs::write(session.file(log), &value)?;
        values.push(serde_json::from_slice::<Value>(&value)?);
    }
    let system = &values[0];
    let plan = &values[1];
    let profile = &values[2];
    let apply = &values[3];
    let actions = &values[4];
    ensure!(
        system["schema"] == "airlock.system.v1"
            && system["dom0"]["profile"]["message"] == message
            && system["dom0"]["nixos"]["system"]["enable"] == true
            && system["dom0"]["nixos"]["system"]["source_ref"].is_null()
            && system["dom0"]["nixos"]["system"]["switch_action"] == action,
        "applied dom0 system JSON did not match expected value"
    );
    ensure!(
        profile["schema"] == "airlock.dom0-profile.v1"
            && profile["message"] == message
            && profile["built_by"] == "sys-config-staging",
        "applied dom0 profile JSON did not match expected value"
    );
    ensure!(
        apply["schema"] == "airlock.system-apply.v1"
            && apply["status"] == "passed"
            && apply["profile_status"] == "passed"
            && apply["host_system_status"] == "passed"
            && apply["host_system_switch_action"] == action,
        "dom0 system apply JSON did not pass"
    );
    for (label, value) in [
        ("system", &system["plan"]),
        ("plan", plan),
        ("apply", apply),
    ] {
        if action == "switch" {
            ensure!(
                value["reboot_required"] == false
                    && value["reboot_actions"]
                        .as_array()
                        .is_some_and(Vec::is_empty)
                    && value["reboot_reasons"]
                        .as_array()
                        .is_some_and(Vec::is_empty),
                "{label} unexpectedly requires reboot"
            );
        } else {
            ensure!(
                value["reboot_required"] == true
                    && value["reboot_actions"]
                        .as_array()
                        .is_some_and(|actions| actions.len() == 1)
                    && value["reboot_actions"][0]["type"] == "dom0-nixos-system"
                    && value["reboot_actions"][0]["switch_action"] == "boot"
                    && value["reboot_reasons"] == json!([reason]),
                "{label} did not preserve the staged boot requirement"
            );
        }
    }
    plan_actions(&system["plan"], message, action)?;
    plan_actions(plan, message, action)?;
    if action == "switch" {
        ensure!(
            system["dom0"]["systemd"]["services"]["airlock-local-extra-module"]["action"]
                == "restart",
            "host system omitted local service restart"
        );
    } else {
        ensure!(
            system["dom0"]["systemd"]["services"] == json!({}),
            "boot-only system declared live service changes"
        );
    }
    let source = apply["host_system_source"]
        .as_str()
        .filter(|source| source.starts_with("/nix/store/"))
        .context("host source is not a Nix store path")?;
    let selected = session.command(
        "readlink",
        &["-f", "/nix/var/nix/profiles/system"],
        "config-vms-dom0-host-system-profile-link.log",
    )?;
    ensure!(
        selected == source,
        "host system profile does not point at applied source"
    );
    let running = session.command(
        "readlink",
        &["/run/current-system"],
        "config-vms-dom0-current-system-link.log",
    )?;
    if action == "switch" {
        ensure!(
            running == source,
            "current system link does not point at applied source"
        );
        ensure!(
            apply["systemd_status"] == "passed"
                && actions["status"] == "passed"
                && actions["actions"]
                    .as_array()
                    .is_some_and(|actions| actions
                        .iter()
                        .any(|action| action["name"] == "airlock-local-extra-module"
                            && action["action"] == "restart"
                            && action["status"] == "passed")),
            "live service actions did not pass"
        );
        for (file, expected, log) in [
            (
                "/etc/airlock/inside-config-applied-message",
                message.to_owned(),
                "config-vms-dom0-host-config-message.log",
            ),
            (
                "/etc/airlock/local-dom0-extra-module",
                format!("{message}-extra"),
                "config-vms-dom0-extra-module-message.log",
            ),
            (
                "/etc/airlock/local-dom0-host-configuration",
                format!("{message}-configuration"),
                "config-vms-dom0-host-configuration-message.log",
            ),
            (
                "/var/lib/airlock/system/local-extra-module-restarted",
                message.to_owned(),
                "config-vms-dom0-extra-module-restarted.log",
            ),
        ] {
            let observed = session.command("cat", &[file], log)?;
            ensure!(
                observed == expected,
                "inside-config marker did not match expected value: {file}"
            );
        }
    } else {
        ensure!(
            apply["systemd_status"] == "skipped"
                && actions["status"] == "skipped"
                && actions["actions"].as_array().is_some_and(Vec::is_empty),
            "boot-only selection unexpectedly ran live service actions"
        );
    }
    let status: Value = serde_json::from_str(&session.command(
        "airlock-config-status",
        &[],
        "config-vms-config-status.json",
    )?)?;
    ensure!(
        status["schema"] == "airlock.config-status.v1"
            && status["status"] == "applied"
            && status["qvm"]["available"] == true
            && status["qvm"]["status"] == "ok"
            && status["apply"]["status"] == "passed"
            && status["system"]["last_apply"]["status"] == "passed",
        "config status JSON did not match applied state"
    );
    ensure!(
        status["pending_reboot"]["required"] == (action == "boot"),
        "config status pending reboot differs from activation mode"
    );
    Ok((apply.clone(), status))
}

/// Match live-action planning independently of the host adapter's success receipt.
fn plan_actions(plan: &Value, message: &str, action: &str) -> Result<()> {
    let actions = plan["live_actions"]
        .as_array()
        .context("host live actions")?;
    ensure!(
        actions
            .iter()
            .any(|item| item["type"] == "dom0-profile" && item["message"] == message),
        "host plan omitted profile activation"
    );
    ensure!(
        actions
            .iter()
            .any(|item| item["type"] == "dom0-nixos-system" && item["action"] == action),
        "host plan omitted selected NixOS activation"
    );
    if action == "switch" {
        ensure!(
            actions.iter().any(|item| item["type"] == "systemd-service"
                && item["name"] == "airlock-local-extra-module"
                && item["action"] == "restart"),
            "host plan omitted local service restart"
        );
    }
    Ok(())
}

/// Audit the selected authority/result relationships, including dependency-generated restart evidence.
fn results(
    archive: &Value,
    apply: &Value,
    status: &Value,
    guests: &[&str],
    action: &str,
    reason: &str,
) -> Result<()> {
    let staging = guests[0];
    for (field, name) in [
        ("staging", guests[0]),
        ("validator", guests[1]),
        ("vault", guests[2]),
    ] {
        ensure!(
            archive[field] == name && apply[field] == name,
            "configuration result substituted a role identity"
        );
    }
    ensure!(
        archive["schema"] == "airlock.config-archive.v1"
            && archive["validation"]["schema"] == "airlock.config-validation.v1",
        "configuration archival schema differs"
    );
    for name in [
        "nix-eval-airlock-topology",
        "airlock-topology-json",
        "nix-eval-airlock-system",
        "airlock-system-json",
        "nix-system-source-refs-dry-run",
    ] {
        ensure!(
            archive["validation"]["checks"]
                .as_array()
                .is_some_and(|checks| checks
                    .iter()
                    .any(|check| check["name"] == name && check["status"] == "passed")),
            "validation check did not pass: {name}"
        );
    }
    for guest in guests {
        ensure!(
            status["topology"]["domain_names"]
                .as_array()
                .is_some_and(|names| names.contains(&json!(guest))),
            "status omitted a declared VM"
        );
    }
    for guest in &guests[..3] {
        ensure!(
            status["qvm"]["managed_domains"]
                .as_array()
                .is_some_and(|domains| domains.iter().any(|domain| domain["name"] == *guest)),
            "status omitted a managed configuration VM"
        );
    }
    ensure!(
        status["qrexec_policy"]["exists"] == true
            && status["deployment"]["current"]["status"] == "passed"
            && status["deployment"]["last_attempt"]["status"] == "passed"
            && status["deployment"]["current"]["candidate_commit"]
                == status["apply"]["candidate_commit"]
            && status["deployment"]["current_apply"]["status"] == "passed",
        "deployment status and candidate bindings disagree"
    );
    ensure!(
        status["pending_vm_restarts"]["actions"]
            .as_array()
            .is_some_and(|actions| actions
                .iter()
                .any(|action| action["domain"] == staging && action["type"] == "domain-feature")),
        "status lost staging feature restart evidence"
    );
    let host = &apply["system_apply_result"];
    ensure!(
        apply["schema"] == "airlock.config-apply.v1"
            && apply["archive"]["status"] == "passed"
            && apply["deployment"]["schema"] == "airlock.config-deployment.v1"
            && apply["deployment_archive"]["schema"] == "airlock.config-vault-deployment.v1"
            && apply["deployment"]["system_apply"] == *host,
        "deployment archival changed the applied host result"
    );
    ensure!(
        apply["setup"]["domains_changed"].as_u64().is_some()
            && apply["setup"]["features_changed"]
                .as_u64()
                .is_some_and(|count| count >= 1),
        "setup did not retain configuration-change evidence"
    );
    let required = action == "boot";
    for pending in [
        &status["pending_reboot"],
        &status["system"]["pending_reboot"],
    ] {
        ensure!(
            pending["required"] == required,
            "status reboot requirements disagree"
        );
        if required {
            ensure!(
                pending["actions"]
                    .as_array()
                    .is_some_and(|actions| actions.len() == 1)
                    && pending["reasons"] == json!([reason]),
                "status lost boot actions or reasons"
            );
        } else {
            ensure!(
                pending["actions"].as_array().is_some_and(Vec::is_empty)
                    && pending["reasons"].as_array().is_some_and(Vec::is_empty),
                "status retained consumed boot actions"
            );
        }
    }
    ensure!(
        status["system"]["host_system_profile_resolved_target"] == host["host_system_source"],
        "status selected a different host source"
    );
    if !required {
        ensure!(
            status["system"]["run_current_system_target"] == host["host_system_source"]
                && status["system"]["run_current_system_resolved_target"]
                    == host["host_system_source"],
            "status running generation differs from activation"
        );
    }
    Ok(())
}

/// Observe the vault's independent persisted documents and pointer agreement, rather than trusting dom0 reports alone.
fn vault_evidence(
    session: &mut Session,
    vault: &str,
    user: &str,
    expected_actions: &str,
) -> Result<()> {
    let repo = format!("/home/{user}/airlock-config-vault");
    session.remote(
        vault,
        "cat",
        &[&format!("{repo}/latest-candidate")],
        None,
        "config-vms-config-vault-archive-tree.log",
    )?;
    session.remote(
        vault,
        "find",
        &[
            &format!("{repo}/candidates"),
            "-maxdepth",
            "3",
            "-type",
            "f",
        ],
        None,
        "config-vms-config-vault-archive-tree.log",
    )?;
    let latest = session.remote(
        vault,
        "cat",
        &[&format!("{repo}/latest-deployment")],
        None,
        "config-vms-vault-latest.log",
    )?;
    let attempt = session.remote(
        vault,
        "cat",
        &[&format!("{repo}/latest-deployment-attempt")],
        None,
        "config-vms-vault-attempt.log",
    )?;
    ensure!(
        latest == attempt
            && !latest.is_empty()
            && latest != "."
            && latest != ".."
            && latest
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.:-".contains(&byte)),
        "vault deployment pointers disagree"
    );
    let directory = format!("{repo}/deployments/{latest}");
    session.remote(
        vault,
        "test",
        &["-d", &directory],
        None,
        "config-vms-vault-directory.log",
    )?;
    for (file, schema, expected) in [
        ("deployment.json", "airlock.config-deployment.v1", "passed"),
        ("apply.json", "airlock.config-apply.v1", "passed"),
        ("setup.json", "airlock.setup-config.v1", "passed"),
        ("topology.json", "airlock.qubes-topology.v1", ""),
        (
            "systemd-actions.json",
            "airlock.systemd-actions.v1",
            expected_actions,
        ),
    ] {
        let text = session.remote(
            vault,
            "cat",
            &[&format!("{directory}/{file}")],
            None,
            &format!("config-vms-vault-{file}"),
        )?;
        let value: Value = serde_json::from_str(&text)?;
        ensure!(
            value["schema"] == schema && (expected.is_empty() || value["status"] == expected),
            "vault deployment artifact differs: {file}"
        );
        if file == "deployment.json" {
            ensure!(
                value["systemd_actions"]["status"] == expected_actions,
                "vault deployment lost service-action status"
            );
        }
    }
    session.remote(
        vault,
        "find",
        &[&directory, "-maxdepth", "2", "-type", "f"],
        None,
        "config-vms-config-vault-deployment-tree.log",
    )?;
    Ok(())
}

/// Preserve the existing preflight/stage/apply/post-apply modes with native checks and typed source inputs.
pub(super) fn execute() -> Result<()> {
    let mut session = Session::new("config-vms-smoke", None, false)?;
    let staging = common::setting("AIRLOCK_CONFIG_STAGING_VM", "sys-config-staging");
    let validator = common::setting("AIRLOCK_CONFIG_VALIDATOR_VM", "sys-config-validator");
    let vault = common::setting("AIRLOCK_CONFIG_VAULT_VM", "sys-config-vault");
    let created = common::setting("AIRLOCK_CONFIG_CREATED_QUBE", "airlock-work-from-config");
    let user = common::setting("AIRLOCK_QUBE_USER", "user");
    let message = common::setting(
        "AIRLOCK_CONFIG_APPLY_MESSAGE",
        &format!("smoke-applied-{id}", id = std::process::id()),
    );
    let mode = common::setting("AIRLOCK_CONFIG_VMS_SMOKE_MODE", "full");
    let stage_name = common::setting("AIRLOCK_CONFIG_APPLY_STAGE", "full");
    let action = common::setting("AIRLOCK_CONFIG_DOM0_SYSTEM_SWITCH_ACTION", "switch");
    let reason = common::setting(
        "AIRLOCK_CONFIG_DOM0_SYSTEM_REASON",
        if action == "boot" {
            "install next dom0 boot generation"
        } else {
            "staged dom0 NixOS system generation"
        },
    );
    let mut record = json!({"schema":"airlock.config-vms-smoke.v1","reason":"staged Rust created a new qrexec-capable nix-closure VM","mode":mode,"config_apply_stage":stage_name,"staging":staging,"vault":vault,"validator":validator,"created_qube":created,"dom0_system_switch_action":action,"dom0_system_reason":reason});
    let result: Result<()> = (|| {
        ensure!(
            [
                "full",
                "preflight",
                "stage-candidate",
                "apply-candidate",
                "post-apply"
            ]
            .contains(&mode.as_str()),
            "unsupported AIRLOCK_CONFIG_VMS_SMOKE_MODE: {mode}"
        );
        ensure!(
            matches!(action.as_str(), "switch" | "boot"),
            "AIRLOCK_CONFIG_DOM0_SYSTEM_SWITCH_ACTION must be switch or boot"
        );
        for guest in [&staging, &validator, &vault, &created] {
            common::vm(guest)?;
        }
        session.control_domain()?;
        for guest in [&staging, &validator, &vault] {
            session.command("qvm-check", &[guest], "config-vms-domain-exists.log")?;
        }
        if matches!(mode.as_str(), "full" | "preflight") {
            for (label, guest) in [
                ("config-vault", &vault),
                ("config-staging", &staging),
                ("config-validator", &validator),
            ] {
                backend(&mut session, guest, label)?;
                session.start(guest)?;
                let uname = session.remote(
                    guest,
                    "uname",
                    &["-a"],
                    None,
                    &format!("config-vms-{label}-uname.log"),
                )?;
                ensure!(
                    uname.contains("Linux"),
                    "{guest} uname output did not contain Linux"
                );
                let guest_user = session.remote(
                    guest,
                    "id",
                    &["-un"],
                    None,
                    &format!("config-vms-{label}-user.log"),
                )?;
                ensure!(guest_user == user, "{guest} qrexec user was not {user}");
                session.runtime_store(guest, &format!("config-vms-{label}-store-mount.log"))?;
                let dom0_user = common::setting(
                    "AIRLOCK_DOM0_USER",
                    &common::setting("SUDO_USER", "airlock"),
                );
                let mut sync = if rustix::process::geteuid().is_root() {
                    let mut command = Command::new("runuser");
                    command.args(["-u", &dom0_user, "--", "qvm-sync-appmenus"]);
                    command
                } else {
                    Command::new("qvm-sync-appmenus")
                };
                session.run(
                    sync.arg(guest),
                    None,
                    &format!("config-vms-{label}-appmenus.log"),
                )?;
                persistence(&mut session, guest, label, &message)?;
            }
            if common::setting("AIRLOCK_CONFIG_VMS_IMPORT_INPUTS", "1") == "1" {
                session.command(
                    "airlock-config-inputs-import",
                    &[],
                    "config-vms-config-inputs-import.log",
                )?;
            }
            session.command(
                "airlock-config-bootstrap-current",
                &[],
                "config-vms-config-current-bootstrap.log",
            )?;
            for (guest, command, label) in [
                (&staging, "airlock-config-staging-init", "config-staging"),
                (
                    &validator,
                    "airlock-config-validator-init",
                    "config-validator",
                ),
            ] {
                session.remote(
                    guest,
                    command,
                    &[],
                    None,
                    &format!("config-vms-{label}-init.log"),
                )?;
            }
            for (guest, label, directory) in [
                (&vault, "config-vault", "airlock-config-vault"),
                (&staging, "config-staging", "airlock-config"),
            ] {
                let status = session.remote(
                    guest,
                    "git",
                    &[
                        "-C",
                        &format!("/home/{user}/{directory}"),
                        "status",
                        "--porcelain=v2",
                        "--branch",
                        "--untracked-files=all",
                    ],
                    None,
                    &format!("config-vms-{label}-repo-status.log"),
                )?;
                ensure!(
                    common::clean_main(&status),
                    "{guest} repository is not initialized cleanly"
                );
            }
        }
        if mode == "preflight" {
            return Ok(());
        }
        if matches!(mode.as_str(), "full" | "stage-candidate") {
            stage(
                &mut session,
                &staging,
                &json!({"created":created,"message":message,"action":action}),
                "test runtime config changes",
            )?;
        }
        if mode == "stage-candidate" {
            return Ok(());
        }
        if matches!(mode.as_str(), "full" | "apply-candidate") {
            session.run(
                Command::new("airlock-config-apply")
                    .args([
                        "--apply",
                        "--staging",
                        &staging,
                        "--validator",
                        &validator,
                        "--vault",
                        &vault,
                    ])
                    .env("AIRLOCK_CONFIG_APPLY_STAGE", &stage_name)
                    .env(
                        "AIRLOCK_CONFIG_APPLY_NIX_OFFLINE",
                        common::setting("AIRLOCK_CONFIG_APPLY_NIX_OFFLINE", "0"),
                    ),
                None,
                "config-vms-apply.log",
            )?;
        }
        if mode == "apply-candidate" {
            return Ok(());
        }
        let topology: Value =
            serde_json::from_slice(&fs::read(session.file("config-apply-topology.json"))?)?;
        common::save(&session.file("config-vms-applied-topology.json"), &topology)?;
        for guest in [&staging, &validator, &vault, &created] {
            ensure!(
                closure(&topology, guest),
                "applied topology does not define a native closure VM: {guest}"
            );
        }
        ensure!(
            topology["prefs"]["default_kernel"] == ""
                && topology["templates"] == json!({})
                && topology["kernels"] == json!({}),
            "applied topology did not preserve native-only boot resources"
        );
        for (guest, color, flag, role) in [
            (
                &vault,
                "black",
                "airlock-config-authority",
                "sys-config-vault",
            ),
            (
                &staging,
                "gray",
                "airlock-config-staging",
                "sys-config-staging",
            ),
            (
                &validator,
                "blue",
                "airlock-config-validator",
                "sys-config-validator",
            ),
        ] {
            let declaration = &topology["domains"][guest];
            ensure!(
                declaration["label"] == color
                    && declaration["netvm"].is_null()
                    && declaration["prefs"]["include_in_backups"] == true
                    && declaration["features"][flag] == "1",
                "configuration role declaration differs: {guest}"
            );
            if role != "sys-config-staging" {
                ensure!(
                    declaration["features"]["airlock-role"] == role,
                    "configuration role feature differs: {guest}"
                );
            }
        }
        ensure!(
            topology["domains"][&staging]["features"]["airlock-config-applied-message"] == message
                && topology["domains"][&created]["features"]["airlock-created-from-config"] == "1"
                && topology["domains"][&created]["features"]["airlock-created-from-config-message"]
                    == message,
            "applied test features differ from the staged message"
        );
        let policy = fs::read_to_string(
            common::path(
                "AIRLOCK_QUBES_POLICY_DIR",
                "/var/lib/airlock/qubes-policy.d",
            )
            .join("90-airlock.policy"),
        )?;
        ensure!(
            policy
                .lines()
                .any(|line| line == "airlock.PolicyProbe * dom0 sys-config-vault allow")
                && policy
                    .lines()
                    .any(|line| line == "airlock.PolicyProbe * @anyvm @anyvm deny"),
            "generated qrexec policy is missing PolicyProbe rules"
        );
        fs::write(session.file("config-vms-qrexec-policy.log"), &policy)?;
        let probe = session.command(
            "qrexec-client",
            &[
                "-d",
                &vault,
                "-T",
                "-w",
                "120",
                "DEFAULT:QUBESRPC airlock.PolicyProbe+ dom0",
            ],
            "config-vms-qrexec-policy-probe.log",
        )?;
        ensure!(
            probe
                .lines()
                .any(|line| line
                    == format!("airlock-policy-probe role=sys-config-vault user={user}")),
            "generated qrexec policy probe output did not match expected value"
        );
        let (host_apply, status) = host(&mut session, &message, &action, &reason)?;
        record["dom0_system_apply"] = host_apply;
        record["config_status"] = status;
        ensure!(
            record["config_status"]["pending_vm_restarts"]["required"] == true
                && record["config_status"]["pending_vm_restarts"]["domains"]
                    .as_array()
                    .is_some_and(|domains| domains.contains(&json!(staging))),
            "running staging VM restart requirement was lost"
        );
        for (guest, feature, log) in [
            (
                &staging,
                "airlock-config-applied-message",
                "config-vms-applied-feature.log",
            ),
            (
                &created,
                "airlock-created-from-config-message",
                "config-vms-created-qube-feature.log",
            ),
        ] {
            ensure!(
                session.command("qvm-features", &[guest, feature], log)? == message,
                "applied config feature did not match expected value for {guest}"
            );
        }
        backend(&mut session, &created, "created-qube")?;
        session.start(&created)?;
        let uname = session.remote(
            &created,
            "uname",
            &["-a"],
            None,
            "config-vms-created-qube-uname.log",
        )?;
        ensure!(
            uname.contains("Linux"),
            "created qube uname did not contain Linux"
        );
        ensure!(
            session.remote(
                &created,
                "id",
                &["-un"],
                None,
                "config-vms-created-qube-user.log"
            )? == user,
            "created qube qrexec user differs"
        );
        session.runtime_store(&created, "config-vms-created-qube-store-mount.log")?;
        persistence(&mut session, &created, "created-qube", &message)?;
        let archived: Value =
            serde_json::from_slice(&fs::read(session.file("config-archive.json"))?)?;
        let applied: Value = serde_json::from_slice(&fs::read(session.file("config-apply.json"))?)?;
        ensure!(
            archived["status"] == "passed"
                && archived["candidate_commit"] == archived["validation_commit"]
                && archived["candidate_commit"] == archived["vault_archive"]["candidate_commit"]
                && archived["validation"]["status"] == "passed",
            "config archive JSON did not pass"
        );
        ensure!(
            applied["status"] == "passed"
                && applied["setup"]["status"] == "passed"
                && applied["deployment"]["status"] == "passed"
                && applied["deployment_archive"]["status"] == "passed"
                && applied["system_apply_result"] == record["dom0_system_apply"]
                && applied["setup"]["restart_required"] == true
                && applied["setup"]["restart_actions"]
                    .as_array()
                    .is_some_and(|actions| actions
                        .iter()
                        .any(|action| action["domain"] == staging
                            && action["type"] == "domain-feature")),
            "config apply JSON did not pass"
        );
        results(
            &archived,
            &applied,
            &record["config_status"],
            &[&staging, &validator, &vault, &created],
            &action,
            &reason,
        )?;
        vault_evidence(
            &mut session,
            &vault,
            &user,
            if action == "boot" {
                "skipped"
            } else {
                "passed"
            },
        )?;
        record["archive"] = archived;
        record["apply"] = applied;
        record["applied_topology"] = topology;
        Ok(())
    })();
    let _ = session.command("qvm-ls", &[], "config-vms-qvm-ls.log");
    let _ = session.command(
        "journalctl",
        &[
            "-b",
            "--no-pager",
            "-n",
            "260",
            "-u",
            "qubesd",
            "-u",
            "libvirtd",
            "-u",
            "qubes-db-dom0",
            "-u",
            "qubes-qrexec-policy-daemon",
        ],
        "config-vms-journal.log",
    );
    let mut observations = String::new();
    for guest in [&vault, &staging, &validator, &created] {
        for program in ["qvm-prefs", "qvm-features"] {
            if let Ok(text) = session.command(
                program,
                &[guest],
                &format!("config-vms-{guest}-{program}.log"),
            ) {
                observations.push_str(&format!("== {guest} {program} ==\n{text}\n"));
            }
        }
        if let Ok(console) = fs::read_to_string(format!("/var/log/xen/console/guest-{guest}.log")) {
            let lines = console
                .lines()
                .rev()
                .take(500)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(
                session.file(&format!("config-vms-{guest}-console.log")),
                lines,
            )?;
        }
    }
    fs::write(session.file("config-vms-qvm-prefs.log"), observations)?;
    if let Ok(log) = fs::read_to_string("/var/log/libvirt/libxl/libxl-driver.log") {
        let lines = log
            .lines()
            .rev()
            .take(260)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(session.file("config-vms-libxl.log"), lines)?;
    }
    for (key, file, kind) in EVIDENCE {
        let file = file
            .replace("$STAGING", &staging)
            .replace("$VAULT", &vault)
            .replace("$VALIDATOR", &validator)
            .replace("$CREATED_QUBE", &created);
        let path = session.file(&file);
        record[*key] = match kind {
            Evidence::Path => json!(path),
            Evidence::Text => fs::read_to_string(path)
                .map(|text| json!(text.trim_end_matches('\n')))
                .unwrap_or(Value::Null),
            Evidence::Json => fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or(Value::Null),
        };
    }
    session.finish(&result, record)?;
    result
}
