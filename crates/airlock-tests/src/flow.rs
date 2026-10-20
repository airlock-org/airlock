//! Finite nested-VM replies exercise staged boot orchestration without starting a VM.

use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

/// Publish only fixture reports and record the exact image/overlay reuse environment supplied by the real flow script.
pub(crate) fn execute(root: &Path, name: &str) -> Result<()> {
    let phase = match name {
        "nix" => {
            println!("{image}", image = root.join("image").display());
            "resolve"
        }
        "run-nested-gui-smoke.sh" => {
            let phase = if std::env::var("AIRLOCK_RUN_CONFIG_BOOT_VERIFY").as_deref() == Ok("1") {
                "verify"
            } else {
                "stage"
            };
            let mut call = json!({"phase": phase});
            for key in [
                "AIRLOCK_VM_DIR",
                "AIRLOCK_VM_IMAGE_ATTR",
                "AIRLOCK_VM_SKIP_BASE_BUILD",
                "AIRLOCK_VM_FRESH",
            ] {
                call[key] = json!(std::env::var(key)?);
            }
            writeln!(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(root.join("calls.jsonl"))?,
                "{call}"
            )?;
            let reports: Value = serde_json::from_slice(&fs::read(root.join("reports.json"))?)?;
            let name = if phase == "verify" {
                "vm-gui-config-boot-verify.json"
            } else {
                "vm-gui-config-vms-smoke.json"
            };
            fs::write(
                root.join("logs").join(name),
                serde_json::to_vec(&reports[phase])?,
            )?;
            phase
        }
        _ => bail!("unexpected boot flow peer: {name}"),
    };
    writeln!(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("events"))?,
        "{phase}"
    )?;
    Ok(())
}
