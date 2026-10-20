//! Namespace-only EFI maintenance driver runs the real command against disposable filesystems.

use anyhow::{Context, Result, ensure};
use serde_json::json;
use std::{fs, path::Path, process::Command};

/// Copy regular fixture files into the explicitly mounted temporary ESP.
fn copy(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy(&entry.path(), &target)?;
        } else {
            ensure!(
                entry.file_type()?.is_file(),
                "maintenance fixture must contain regular files"
            );
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Report exit status and remaining cache after the native quarantine operation in its private mount namespace.
pub(crate) fn execute(args: &[String]) -> Result<()> {
    ensure!(args.len() == 5, "maintenance peer arguments");
    let esp = if args[4] == "separate" {
        let target = Path::new("/var/test-esp");
        copy(Path::new(&args[1]), target)?;
        target
    } else {
        Path::new(&args[1])
    };
    let result = Command::new(&args[0])
        .arg("boot-cache")
        .arg("--esp")
        .arg(esp)
        .args([
            "--descriptors",
            &args[2],
            "--quarantine-root",
            &args[3],
            "--apply",
        ])
        .output()?;
    let cache = fs::read_dir(esp.join("EFI/nixos"))?
        .map(|entry| {
            entry?
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("cache filename"))
        })
        .collect::<Result<Vec<_>>>()?;
    println!(
        "{}",
        json!({"success": result.status.success(), "stdout": String::from_utf8(result.stdout).context("maintenance report")?, "cache": cache})
    );
    Ok(())
}
