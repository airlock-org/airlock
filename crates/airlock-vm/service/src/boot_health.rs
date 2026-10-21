//! Observe the graphical login and admitted worker mounts without starting services.

use airlock_vm_service::support;
use anyhow::{Context, Result, ensure};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

/// Local health query invoked through dom0's existing administrative transport.
#[derive(Args)]
pub(crate) struct Request {
    /// Capability whose readiness must be observed, never repaired by this command.
    #[command(subcommand)]
    action: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Require the display manager and an active local graphical greeter or user session.
    Desktop,
    /// Require this worker's service, mounted store layers and authenticated readiness identity.
    BuildStore {
        /// Installed worker role, distinct from the storage namespace.
        #[arg(long, value_enum)]
        domain: Worker,
        /// Persistent root used by the worker's Nix wrapper.
        #[arg(long, default_value = "/rw/airlock-builder/merged")]
        root: PathBuf,
        /// Volatile mount and readiness state published by the worker store service.
        #[arg(long, default_value = "/run/airlock-builder")]
        runtime: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum Worker {
    SysCtrl,
    UsrAssistant,
}

impl Worker {
    /// Match the two installed worker service identities.
    fn name(self) -> &'static str {
        match self {
            Self::SysCtrl => "sys-ctrl",
            Self::UsrAssistant => "usr-assistant",
        }
    }
}

/// Bound the entire query, including each session observation, to one short probe budget.
fn query(program: &str, arguments: &[&str], deadline: Instant) -> Result<String> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    ensure!(!remaining.is_zero(), "boot health query timed out");
    support::text(Command::new(program).args(arguments), remaining)
}

/// A running GDM process alone does not establish that its graphical login has started.
fn desktop(deadline: Instant) -> Result<()> {
    query(
        "systemctl",
        &["is-active", "--quiet", "display-manager.service"],
        deadline,
    )?;
    let sessions = query(
        "loginctl",
        &["list-sessions", "--no-legend", "--no-pager"],
        deadline,
    )?;
    ensure!(sessions.lines().count() <= 64, "too many login sessions");
    for line in sessions.lines() {
        let id = line
            .split_whitespace()
            .next()
            .context("missing login session identity")?;
        ensure!(
            id.len() <= 64 && id.bytes().all(|byte| byte.is_ascii_alphanumeric()),
            "invalid login session identity"
        );
        let observation = query(
            "loginctl",
            &[
                "show-session",
                id,
                "--no-pager",
                "--property=Class",
                "--property=Type",
                "--property=State",
                "--property=Active",
                "--property=Remote",
                "--property=Seat",
            ],
            deadline,
        );
        // A session may disappear while a greeter hands over to the logged-in user.
        let Ok(observation) = observation else {
            continue;
        };
        let properties: BTreeMap<_, _> = observation
            .lines()
            .filter_map(|line| line.split_once('='))
            .collect();
        if matches!(properties.get("Class"), Some(&"greeter" | &"user"))
            && matches!(properties.get("Type"), Some(&"x11" | &"wayland"))
            && properties.get("State") == Some(&"active")
            && properties.get("Active") == Some(&"yes")
            && properties.get("Remote") == Some(&"no")
            && properties.get("Seat") == Some(&"seat0")
        {
            return Ok(());
        }
    }
    anyhow::bail!("no active local graphical greeter or user session")
}

/// Keep qrexec available for recovery while refusing to call a broken build worker ready.
pub(crate) fn execute(request: Request) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    match request.action {
        Action::Desktop => desktop(deadline),
        Action::BuildStore {
            domain,
            root,
            runtime,
        } => {
            let role = domain.name();
            query(
                "systemctl",
                &[
                    "is-active",
                    "--quiet",
                    &format!("airlock-{role}-build-store.service"),
                ],
                deadline,
            )?;
            crate::nix::pinned_at(
                &root,
                &runtime,
                deadline.saturating_duration_since(Instant::now()),
            )?;
            let ready: Value =
                airlock_nix_core::state::read_json(&runtime.join("ready.json"), 1024 * 1024)?;
            ensure!(
                ready["identity"]["domain"] == format!("{role}-build-store")
                    && ready["identity"]["schema"] == format!("airlock.{role}-build-store.v1"),
                "worker build store belongs to another role"
            );
            Ok(())
        }
    }
}
