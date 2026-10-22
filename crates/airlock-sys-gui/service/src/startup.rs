//! Read dom0's existing boot-status RPC without starting guests or changing control policy.

use airlock_vm_service::support;
use anyhow::Result;
use serde::Deserialize;
use std::{process::Command, time::Duration};

/// User-visible progress from the boot supervisor; it conveys no deployment authority.
#[derive(Deserialize)]
pub(crate) struct Status {
    /// Current supervisor outcome, including working, complete and degraded.
    pub(crate) status: String,
    /// Whether the graphical desktop has passed its readiness checks.
    #[serde(default)]
    pub(crate) desktop_ready: bool,
    /// Current unit or guest being prepared or started.
    #[serde(default)]
    pub(crate) stage: String,
    /// Human-readable explanation of the most recent checkpoint.
    #[serde(default)]
    pub(crate) detail: String,
}

impl Status {
    /// Describe the current checkpoint while bounding notification and terminal text.
    pub(crate) fn progress(&self) -> String {
        let text = if self.detail.is_empty() {
            &self.stage
        } else {
            &self.detail
        };
        text.chars().filter(|c| !c.is_control()).take(512).collect()
    }
}

/// Read a bounded status reply; a transport failure remains distinct from a boot outcome.
pub(crate) fn read(timeout: Duration) -> Result<Status> {
    let bytes = support::run(
        Command::new(support::tool("AIRLOCK_QVM_RUN", "qvm-run")).args([
            "--pass-io",
            "--no-gui",
            "--service",
            "dom0",
            "airlock.BootStatus",
        ]),
        None,
        timeout,
    )?
    .checked()?;
    Ok(serde_json::from_slice(&bytes)?)
}
