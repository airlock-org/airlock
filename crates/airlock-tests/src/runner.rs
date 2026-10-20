//! Run explicitly prepared Cargo tests without invoking Cargo or provisioning tools.

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path, process::Command};

/// Select one Cargo target or all prepared test executables, forwarding only libtest arguments to children.
fn main() -> Result<()> {
    let mut arguments: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(
        !arguments.is_empty(),
        "usage: airlock-test-runner CARGO-RECEIPT [--all | --target NAME] [LIBTEST-ARG ...]"
    );
    let receipt = arguments.remove(0);
    let all = arguments
        .first()
        .is_some_and(|argument| argument == "--all");
    let target = if all {
        arguments.remove(0);
        None
    } else if arguments
        .first()
        .is_some_and(|argument| argument == "--target")
    {
        ensure!(arguments.len() > 1, "--target requires a Cargo target name");
        arguments.remove(0);
        Some(
            arguments
                .remove(0)
                .into_string()
                .map_err(|_| anyhow::anyhow!("non-UTF-8 Cargo target"))?,
        )
    } else {
        Some("integration".to_owned())
    };
    let text = fs::read_to_string(receipt)
        .context("prepare Cargo tests with make native-integration-build")?;
    let mut executables = BTreeMap::new();
    let mut finished = false;
    for line in text.lines() {
        let value: Value = serde_json::from_str(line)?;
        if value["reason"] == "build-finished" {
            finished = value["success"] == true;
        }
        if value["reason"] == "compiler-artifact"
            && value["profile"]["test"] == true
            && let Some(executable) = value["executable"].as_str()
        {
            let name = value["target"]["name"]
                .as_str()
                .context("Cargo test target name")?;
            if target.as_ref().is_none_or(|target| name == target) {
                executables.insert(executable.to_owned(), name.to_owned());
            }
        }
    }
    ensure!(finished, "Cargo preparation did not finish successfully");
    ensure!(
        !executables.is_empty(),
        "missing selected Cargo test executable in preparation receipt"
    );
    let mut failed = Vec::new();
    for (executable, name) in executables {
        ensure!(
            Path::new(&executable).is_file(),
            "prepared test executable is missing: {executable}"
        );
        let status = Command::new(&executable).args(&arguments).status()?;
        if !status.success() {
            failed.push(name);
        }
    }
    ensure!(
        failed.is_empty(),
        "Cargo test targets failed: {targets}",
        targets = failed.join(", ")
    );
    Ok(())
}
