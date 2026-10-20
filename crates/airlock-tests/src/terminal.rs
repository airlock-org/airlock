//! An inert qrexec peer supplies a local util-linux PTY for the explicitly selected terminal regression.

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{fs, os::unix::process::CommandExt, path::Path, process::Command};

/// Preserve real PTY and shell behavior without connecting to dom0 or invoking a Qubes transport.
pub(crate) fn execute(root: &Path, args: &[String]) -> Result<()> {
    ensure!(
        args == [
            "--no-filter-escape-chars-stdout",
            "--no-filter-escape-chars-stderr",
            "dom0",
            "airlock.DevDom0Shell"
        ],
        "unexpected terminal RPC arguments"
    );
    if root.join("failure").exists() {
        println!("simulated connection failure");
        std::process::exit(1);
    }
    let tools: Value = serde_json::from_slice(&fs::read(root.join("tools.json"))?)?;
    let bash = tools["bash"].as_str().context("fixture shell")?;
    let shell = format!(
        "stty sane rows 30 cols 100; export PS1='AIRLOCK_TEST_PROMPT> '; exec {bash} --noprofile --norc -i",
        bash = airlock_tests::process::quote(bash)
    );
    Err(Command::new(tools["script"].as_str().context("PTY tool")?)
        .args([
            "--quiet",
            "--return",
            "--flush",
            "--command",
            &shell,
            "/dev/null",
        ])
        .exec()
        .into())
}
