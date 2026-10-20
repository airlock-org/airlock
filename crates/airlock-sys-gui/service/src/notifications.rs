//! Report recoverable startup failures through the desktop's native notification service.

use airlock_vm_service::support;
use anyhow::Result;
use serde::Deserialize;
use std::{process::Command, thread, time::Duration};

#[derive(Deserialize)]
struct BootStatus {
    // Dom0's reported readiness outcome; unrelated metadata is ignored.
    status: String,
    // True after critical hardware and desktop checks have succeeded.
    #[serde(default)]
    desktop_ready: bool,
}

fn rpc(service: &str) -> Result<support::Output> {
    support::run(
        Command::new(support::tool("AIRLOCK_QVM_RUN", "qvm-run")).args([
            "--pass-io",
            "--no-gui",
            "--service",
            "dom0",
            service,
        ]),
        None,
        Duration::from_secs(20),
    )
}

pub(crate) fn execute() -> Result<()> {
    loop {
        match rpc("airlock.BootStatus") {
            Ok(output) if !output.status.success() => return Ok(()),
            Ok(output) => {
                if let Ok(state) = serde_json::from_slice::<BootStatus>(&output.stdout) {
                    if state.status == "complete" {
                        return Ok(());
                    }
                    if state.desktop_ready && state.status == "degraded" {
                        let response = support::text(Command::new(support::tool("AIRLOCK_NOTIFY_SEND", "notify-send")).args(["--app-name=AirlockOS", "--icon=dialog-warning", "--expire-time=0", "--wait", "--action=retry=Retry", "--action=dismiss=Dismiss", "Some background services could not start", "Your desktop is ready. You can retry starting the background services."]), Duration::from_secs(24 * 60 * 60))?;
                        if response != "retry" {
                            return Ok(());
                        }
                        if let Err(error) =
                            rpc("airlock.BootRetry").and_then(|v| v.checked().map(|_| ()))
                        {
                            eprintln!("background-service retry: {error:#}");
                        }
                    }
                }
            }
            Err(error) => eprintln!("boot status: {error:#}"),
        }
        thread::sleep(Duration::from_secs(5));
    }
}
