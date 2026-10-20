//! Preserve the existing disposable qrexec and visible-window smoke scenarios.

use super::common::{self, Session};
use anyhow::{Context, Result, ensure};
use serde_json::json;
use std::{
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};

/// Exercise Qubes VM creation/start and qrexec, deleting only a VM created by this invocation.
pub(super) fn qubes(root: Option<&Path>) -> Result<()> {
    let mut session = Session::new("qubes-smoke", root, true)?;
    let guest = common::setting("AIRLOCK_TEST_QUBE", "test-airlock-smoke");
    let mut template = common::setting("AIRLOCK_TEST_TEMPLATE", "");
    let mut created = false;
    let mut uname = None;
    let result: Result<()> = (|| {
        session.control_domain()?;
        common::vm(&guest)?;
        if template.is_empty() {
            template =
                session.command("qubes-prefs", &["default_template"], "qubes-template.log")?;
        }
        ensure!(
            !template.is_empty(),
            "no template selected; set AIRLOCK_TEST_TEMPLATE or qubes-prefs default_template"
        );
        let names = session.command(
            "qvm-ls",
            &["--raw-data", "--fields", "NAME", "--no-spinner"],
            "qubes-domains.log",
        )?;
        if !names.lines().any(|name| name == guest) {
            session.command(
                "qvm-create",
                &[
                    "--class",
                    "AppVM",
                    "--template",
                    &template,
                    "--label",
                    "red",
                    &guest,
                ],
                "qubes-create.log",
            )?;
            created = true;
        }
        session.start(&guest)?;
        let help = session
            .command("qvm-run", &["--help"], "qubes-run-help.log")
            .unwrap_or_default();
        let mut args = vec!["--pass-io"];
        if help.contains("--no-gui") {
            args.push("--no-gui");
        }
        args.extend([guest.as_str(), "uname -a"]);
        let output = session.command("qvm-run", &args, "qubes-uname.txt")?;
        ensure!(
            output.contains("Linux"),
            "qrexec uname output did not contain Linux"
        );
        uname = Some(output);
        Ok(())
    })();
    if result.is_ok() && created && common::setting("AIRLOCK_TEST_CLEANUP", "1") == "1" {
        if session
            .command(
                "qvm-check",
                &["--running", &guest],
                "qubes-cleanup-power.log",
            )
            .is_ok()
            && session
                .command(
                    "qvm-shutdown",
                    &["--wait", "--timeout=60", &guest],
                    "qubes-cleanup-shutdown.log",
                )
                .is_err()
        {
            let _ = session.command("qvm-kill", &[&guest], "qubes-cleanup-kill.log");
        }
        let _ = session.command(
            "qvm-remove",
            &["--force", &guest],
            "qubes-cleanup-remove.log",
        );
    }
    session.finish(&result, json!({"schema":"airlock.qubes-smoke.v1","qube":guest,"template":template,"reason":"qrexec uname succeeded","stdout":uname}))?;
    result
}

/// Launch through qvm-run and wait for the same title in the existing dom0 X display.
pub(super) fn gui(root: Option<&Path>) -> Result<()> {
    let mut session = Session::new("qubes-gui-smoke", root, true)?;
    let guest = common::setting("AIRLOCK_TEST_QUBE", "test-airlock-smoke");
    let title = common::setting(
        "AIRLOCK_GUI_SMOKE_TITLE",
        &format!("airlock-gui-smoke-{id}", id = std::process::id()),
    );
    let display = common::setting("AIRLOCK_GUI_DISPLAY", &common::setting("DISPLAY", ":0"));
    let authority = common::path(
        "AIRLOCK_GUI_XAUTHORITY",
        common::path(
            "XAUTHORITY",
            std::path::PathBuf::from(std::env::var_os("HOME").context("HOME is missing")?)
                .join(".Xauthority"),
        ),
    );
    let mut windows = None;
    let result: Result<()> = (|| {
        let timeout: u64 = common::setting("AIRLOCK_GUI_SMOKE_TIMEOUT", "60")
            .parse()
            .context("AIRLOCK_GUI_SMOKE_TIMEOUT must be an integer")?;
        let sleep: u64 = common::setting("AIRLOCK_GUI_SMOKE_REMOTE_SLEEP", "90")
            .parse()
            .context("AIRLOCK_GUI_SMOKE_REMOTE_SLEEP must be an integer")?;
        for service in ["qubesd.service", "qubes-qrexec-policy-daemon.service"] {
            session.command(
                "systemctl",
                &["is-active", "--quiet", service],
                "gui-services.log",
            )?;
        }
        session.start(&guest)?;
        let _ = session.command("qvm-start-gui", &[&guest], "qubes-gui-start.log");
        // Job-control syntax is transport glue; all target/title/deadline decisions are native.
        let launch = format!(
            "sh -lc {command}",
            command = common::crate_quote(&format!(
                "command -v xterm >/dev/null || exit 127; nohup xterm -title {title} -e sh -c {sleep} >/tmp/airlock-gui-smoke.log 2>&1 &",
                title = common::crate_quote(&title),
                sleep = common::crate_quote(&format!("sleep {sleep}"))
            ))
        );
        session.command("qvm-run", &[&guest, &launch], "qubes-gui-launch.log")?;
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(timeout))
            .context("GUI timeout is too large")?;
        while Instant::now() < deadline {
            if let Ok(found) = session.run(
                Command::new("xdotool")
                    .args(["search", "--name", &title])
                    .env("DISPLAY", &display)
                    .env("XAUTHORITY", &authority),
                None,
                "qubes-gui-search.log",
            ) && !found.is_empty()
            {
                windows = Some(found);
                return Ok(());
            }
            thread::sleep(
                Duration::from_secs(2).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        anyhow::bail!("no dom0 X window appeared for {title} within {timeout}s")
    })();
    session.finish(&result, json!({"schema":"airlock.qubes-gui-smoke.v1","qube":guest,"title":title,"display":display,"reason":"GUI window appeared","launch_log":session.file("qubes-gui-launch.log"),"window_ids":windows}))?;
    result
}
