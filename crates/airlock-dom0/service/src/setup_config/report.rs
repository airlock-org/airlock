//! Combine setup adapter receipts without treating logs as execution authority.

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

/// Append one informational Rust reconciliation event to the current setup log.
pub(super) fn append(directory: &Path, file: &str, event: &Value) -> Result<()> {
    fs::create_dir_all(directory)?;
    let mut output = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(directory.join(file))?;
    writeln!(output, "{event}")?;
    Ok(())
}

/// Read complete JSONL events; malformed evidence must not silently become success.
fn lines(path: &Path) -> Result<Vec<Value>> {
    if !path.try_exists()? {
        return Ok(Vec::new());
    }
    fs::read_to_string(path)?
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

/// Merge only child helpers actually invoked by this run and fixed local receipt names.
/// Finalization is idempotent; it never changes VM or storage state.
pub(super) fn finalize(path: &Path) -> Result<()> {
    let directory = path.parent().context("setup result directory")?;
    let mut root: Value = crate::state::read(path)?;
    let run = root["run_id"]
        .as_str()
        .context("missing setup run identity")?
        .to_owned();
    if root["rust_reconciled"] == run {
        return Ok(());
    }
    let mut reports = vec![root.clone()];
    if root["phase"] == "finish-only" {
        let prepared: Value = crate::state::read(&directory.join("setup-prepare.json"))?;
        ensure!(
            prepared["run_id"] == run,
            "setup preparation belongs to another run"
        );
        reports.push(prepared);
    }
    let mut seen = std::collections::BTreeSet::new();
    for helper in lines(&directory.join("rust-setup-helpers.jsonl"))? {
        let name = helper.as_str().context("invalid helper receipt identity")?;
        ensure!(
            !name.is_empty() && !name.contains('/') && name != "." && name != "..",
            "invalid helper receipt path"
        );
        ensure!(seen.insert(name.to_owned()), "duplicate helper receipt");
        let receipt = directory.join(name).join("setup-config.json");
        ensure!(
            root["status"] != "passed" || receipt.try_exists()?,
            "missing helper receipt"
        );
        if receipt.try_exists()? {
            let child: Value = crate::state::read(&receipt)?;
            ensure!(
                child["run_id"] == run,
                "helper receipt belongs to another run"
            );
            ensure!(
                root["status"] != "passed" || child["status"] == "passed",
                "helper did not complete successfully"
            );
            reports.push(child);
        }
    }
    let counts = [
        "templates_changed",
        "kernels_changed",
        "domains_changed",
        "prefs_changed",
        "features_changed",
        "services_changed",
        "devices_changed",
        "qrexec_policy_changed",
        "root_images_changed",
        "store_volumes_changed",
        "state_volumes_changed",
        "store_views_changed",
        "qubesd_restarts",
        "unsupported_domains_count",
    ];
    for name in counts {
        let total = reports
            .iter()
            .try_fold(0u64, |sum, report| -> Result<u64> {
                sum.checked_add(report[name].as_u64().unwrap_or(0))
                    .context("setup count overflow")
            })?;
        root[name] = json!(total);
    }
    for name in ["live_actions", "restart_actions", "unsupported_domains"] {
        root[name] = Value::Array(
            reports
                .iter()
                .flat_map(|report| report[name].as_array().into_iter().flatten().cloned())
                .collect(),
        );
    }
    root["restart_required"] = json!(reports.iter().any(|r| r["restart_required"] == true));
    for mut action in lines(&directory.join("rust-vm-actions.jsonl"))? {
        let restart = action
            .as_object_mut()
            .context("invalid reconciliation event")?
            .remove("restart")
            .unwrap_or(Value::Bool(false));
        let count = match action["type"].as_str() {
            Some("domain") => "domains_changed",
            Some("domain-pref") => "prefs_changed",
            Some("domain-feature") => "features_changed",
            Some("domain-service") => "services_changed",
            Some("domain-device") => "devices_changed",
            _ => anyhow::bail!("unknown reconciliation event"),
        };
        if action["action"] != "defer" {
            root[count] = json!(
                root[count]
                    .as_u64()
                    .unwrap_or(0)
                    .checked_add(1)
                    .context("setup count overflow")?
            );
        }
        root["live_actions"]
            .as_array_mut()
            .unwrap()
            .push(action.clone());
        if restart == true {
            action["action"] = json!("restart");
            root["restart_actions"].as_array_mut().unwrap().push(action);
            root["restart_required"] = json!(true);
        }
    }
    root["rust_reconciled"] = json!(run);
    save(path, &root)?;
    if root["phase"] == "prepare-only" {
        save(&directory.join("setup-prepare.json"), &root)?;
    }
    Ok(())
}

/// Derive one fixed helper log directory; arbitrary path components are refused.
pub(crate) fn helper_directory(root: &Path, domain: &str, mode: &str) -> Result<PathBuf> {
    let _: airlock_nix_core::identity::GuestName = domain.parse()?;
    ensure!(
        matches!(mode, "--backend-only" | "--devices-only"),
        "invalid helper mode"
    );
    let run =
        std::env::var("AIRLOCK_SETUP_RUN_ID").unwrap_or_else(|_| std::process::id().to_string());
    ensure!(
        run.bytes().all(|b| b.is_ascii_digit()) && !run.is_empty(),
        "invalid setup run ID"
    );
    let name = format!("vm-{run}-{domain}-{mode}");
    let directory = root.join(&name);
    fs::create_dir_all(&directory)?;
    match fs::remove_file(directory.join("setup-config.json")) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    append(root, "rust-setup-helpers.jsonl", &json!(name))?;
    Ok(directory)
}

/// Replace an informational receipt atomically, independently of the trusted journal writer.
/// Logs are not authority and need not reside beneath the protected state directory.
pub(crate) fn save(path: &Path, value: &Value) -> Result<()> {
    let mut output = tempfile::NamedTempFile::new_in(path.parent().context("receipt directory")?)?;
    serde_json::to_writer_pretty(&mut output, value)?;
    writeln!(output)?;
    output.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal receipt keeps each merge boundary and run identity explicit.
    fn receipt(phase: &str) -> Value {
        json!({"run_id":"123", "phase":phase, "status":"passed", "prefs_changed":1,
            "live_actions":[{"type":"global-pref","action":"set"}], "restart_actions":[],
            "unsupported_domains":[], "restart_required":false})
    }

    /// Preparation, one child and Rust changes are counted once across repeated finalization.
    #[test]
    fn merges_preparation_and_helpers_without_losing_restart_evidence() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let path = root.join("setup-config.json");
        save(&path, &receipt("prepare-only"))?;
        finalize(&path)?;
        fs::create_dir(root.join("child"))?;
        let mut child = receipt("backend-only");
        child["restart_required"] = json!(true);
        child["restart_actions"] = json!([{"type":"domain-root","domain":"usr-home"}]);
        save(&root.join("child/setup-config.json"), &child)?;
        append(root, "rust-setup-helpers.jsonl", &json!("child"))?;
        append(
            root,
            "rust-vm-actions.jsonl",
            &json!({"type":"domain-feature","action":"set","restart":false}),
        )?;
        append(
            root,
            "rust-vm-actions.jsonl",
            &json!({"type":"domain-pref","action":"defer"}),
        )?;
        save(&path, &receipt("finish-only"))?;
        finalize(&path)?;
        let result: Value = crate::state::read(&path)?;
        assert_eq!(result["prefs_changed"], 3);
        assert_eq!(result["features_changed"], 1);
        assert_eq!(result["live_actions"].as_array().unwrap().len(), 5);
        assert_eq!(result["restart_required"], true);
        assert_eq!(result["restart_actions"], child["restart_actions"]);
        let before = fs::read(&path)?;
        finalize(&path)?;
        assert_eq!(fs::read(&path)?, before);
        Ok(())
    }

    /// Incomplete, stale, failed or duplicate child evidence cannot become a passed report.
    #[test]
    fn refuses_invalid_helper_evidence() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let root = directory.path();
        let path = root.join("setup-config.json");
        fs::create_dir(root.join("child"))?;
        append(root, "rust-setup-helpers.jsonl", &json!("child"))?;
        for invalid in 0..4 {
            save(&path, &receipt("all"))?;
            let mut child = receipt("backend-only");
            match invalid {
                0 => {}
                1 => {
                    child["run_id"] = json!("other");
                    save(&root.join("child/setup-config.json"), &child)?;
                }
                2 => {
                    child["status"] = json!("failed");
                    save(&root.join("child/setup-config.json"), &child)?;
                }
                _ => {
                    save(&root.join("child/setup-config.json"), &child)?;
                    append(root, "rust-setup-helpers.jsonl", &json!("child"))?;
                }
            }
            assert!(finalize(&path).is_err());
        }
        Ok(())
    }
}
