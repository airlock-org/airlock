//! Keep enrollment interactive while explaining startup delays and Qubes transport failures.

use crate::startup;
use airlock_vm_service::support;
use anyhow::{Result, bail};
use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

/// Wait only for read-only startup evidence; submit the enrollment RPC exactly once afterward.
pub(crate) fn execute(timeout: u64) -> Result<u8> {
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let mut previous = String::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!(
                "Startup did not finish within {timeout} seconds. Tailscale enrollment was not requested; retry after the background VMs are ready."
            );
        }

        let message = match startup::read(remaining.min(Duration::from_secs(15))) {
            Ok(state) if state.status == "complete" => break,
            Ok(state) if state.status == "working" => format!(
                "Waiting for background startup: {progress}",
                progress = state.progress()
            ),
            Ok(state) => bail!(
                "Startup is {status}: {progress}. Tailscale enrollment was not requested. Resolve the startup failure and retry.",
                status = state.status,
                progress = state.progress()
            ),
            Err(error) => format!(
                "Waiting for startup status; the Qubes control service is unavailable: {error:#}"
            ),
        };
        if message != previous {
            println!("{message}");
            previous = message;
        }
        thread::sleep(
            deadline
                .saturating_duration_since(Instant::now())
                .min(Duration::from_secs(2)),
        );
    }

    println!("Startup complete. Requesting Tailscale enrollment in usr-assistant...");
    // Inherit the terminal so route/DNS errors and the browser login URL appear immediately.
    // The upstream timeout bounds transport and login without replaying an uncertain request.
    let status = Command::new(support::tool("AIRLOCK_TIMEOUT", "timeout"))
        .args(["--foreground", "--kill-after=5s", "180s"])
        .arg(support::tool("AIRLOCK_QVM_RUN", "qvm-run"))
        .args([
            "--pass-io",
            "--no-gui",
            "--service",
            "usr-assistant",
            "airlock.DevTailscaleUp",
        ])
        .status()?;
    let code = status
        .code()
        .and_then(|v| u8::try_from(v).ok())
        .unwrap_or(1);
    match code {
        0 => println!("Tailscale enrollment completed."),
        124 => eprintln!(
            "The enrollment request timed out. Check startup and network connectivity before retrying; the request was not automatically repeated."
        ),
        126 => eprintln!(
            "Qubes could not complete the request. A refusal can mean a policy restriction or an interrupted control service; it does not establish that Tailscale rejected the login. Check the details above and retry after startup is stable."
        ),
        _ => eprintln!(
            "Enrollment failed (exit {code}). Check the route, DNS or HTTPS error above and the connection in sys-net/sys-firewall before retrying."
        ),
    }
    Ok(code)
}
