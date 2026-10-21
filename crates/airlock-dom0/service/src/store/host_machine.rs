//! Bind the machine's selected release and guest generations to a particular host boot.
//! Recovery restores that binding before runtime coordination can observe writable state.

use super::{fs::*, host, records};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

fn binding_path(storage: &Path, record: &Value) -> Result<PathBuf> {
    let generation = number(record, "generation")?;
    ensure!(generation > 0, "invalid host generation");
    Ok(storage
        .join("generations")
        .join(generation.to_string())
        .join("machine.json"))
}

pub(super) fn save(root: &Path, storage: &Path, machine: &Path, record: &Value) -> Result<()> {
    let pointer = machine.join("selection.json");
    let selection: Value = if pointer.try_exists()? {
        read(&pointer)?
    } else {
        Value::Null
    };
    let staged = if !selection.is_null() {
        let release = PathBuf::from(text(&selection, "release")?);
        ensure!(
            release.starts_with(machine)
                && !release
                    .components()
                    .any(|c| c == std::path::Component::ParentDir),
            "host binding requires a local machine candidate"
        );
        ensure!(
            read::<Value>(&release.join("release.json"))?["host"] == record["system"],
            "machine selection differs from host binding"
        );
        let path = release.join("staged.json");
        if path.try_exists()? {
            read(&path)?
        } else {
            json!({})
        }
    } else {
        let mut staged = json!({});
        for directory in children(&root.join("domains"))? {
            let name = directory
                .file_name()
                .and_then(|s| s.to_str())
                .context("invalid domain directory")?;
            if name == "dom0" || name.ends_with("-build-store") {
                continue;
            }
            if let Some(generation) = records::current(root, name)? {
                staged[name] = json!(generation);
            }
        }
        staged
    };
    write(
        &binding_path(storage, record)?,
        &json!({"schema":"airlock.host-machine.v1","system":record["system"],"selection":selection,"staged":staged}),
        0o600,
    )
}

pub(super) fn restore(sysroot: &Path, record: &Value) -> Result<()> {
    let storage = sysroot.join(host::HOST.trim_start_matches('/'));
    let machine = sysroot.join(host::MACHINE.trim_start_matches('/'));
    let path = binding_path(&storage, record)?;
    if !path.try_exists()? {
        ensure!(
            record["generation"] == 1 && !machine.join("selection.json").try_exists()?,
            "host generation has no machine boot binding"
        );
        return Ok(());
    }
    let binding: Value = read(&path)?;
    ensure!(
        binding["schema"] == "airlock.host-machine.v1" && binding["system"] == record["system"],
        "host machine boot binding mismatch"
    );
    mkdir_all(&machine)?;
    let selection = &binding["selection"];
    if !selection.is_null() {
        let release = PathBuf::from(text(selection, "release")?);
        ensure!(
            release.starts_with(host::MACHINE)
                && !release
                    .components()
                    .any(|c| c == std::path::Component::ParentDir),
            "invalid bound machine candidate"
        );
        let release = sysroot.join(release.strip_prefix("/")?);
        let manifest: Value = read(&release.join("release.json"))?;
        ensure!(
            manifest["host"] == record["system"],
            "bound release differs from booted host"
        );
        let transaction = text(selection, "transaction")?;
        component(&transaction)?;
        let status: Value = read(
            &machine
                .join("transactions")
                .join(transaction)
                .join("status.json"),
        )?;
        ensure!(
            status["release"] == selection["release"]
                && matches!(
                    status["phase"].as_str(),
                    Some("complete" | "reboot-required")
                ),
            "bound deployment is incomplete; use independent recovery"
        );
        if manifest["stagedGenerations"]
            .as_object()
            .is_some_and(|v| !v.is_empty())
        {
            ensure!(
                manifest["stagedGenerations"] == binding["staged"]
                    && read::<Value>(&release.join("staged.json"))? == binding["staged"],
                "bound guest generations differ from release"
            );
        }
        write(&machine.join("selection.json"), selection, 0o644)?;
        write(&machine.join("latest.json"), &status, 0o644)?;
    } else {
        remove(&machine.join("selection.json"))?;
        remove(&machine.join("latest.json"))?;
        sync_dir(&machine)?;
    }
    write(&storage.join("boot-staged.json"), &binding["staged"], 0o644)?;
    let temporary = machine.join("controller.new");
    remove(&temporary)?;
    symlink(
        format!("{}/sw/bin/airlock-dom0-service", text(record, "system")?),
        &temporary,
    )?;
    fs::rename(temporary, machine.join("controller"))?;
    sync_dir(&machine)
}
