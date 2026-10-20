//! Validate the existing root input receivers without opening or creating input devices.

use anyhow::{Result, ensure};
use clap::Args;
use std::{fs, os::unix::fs::FileTypeExt, path::PathBuf};

/// Fixed local device and handler paths supplied by the installed input RPC wrapper.
#[derive(Args)]
pub(crate) struct Request {
    /// Kernel uinput device; alternate paths support explicitly scoped local diagnostics.
    #[arg(long, default_value = "/dev/uinput")]
    uinput: PathBuf,
    /// Every handler required by the installed keyboard/mouse/tablet receiver composition.
    #[arg(long = "handler", required = true)]
    handlers: Vec<PathBuf>,
}

/// Accept only dom0's argument-free readiness RPC and observe device/handler accessibility.
pub(crate) fn ready(request: Request) -> Result<()> {
    ensure!(
        std::env::var("QREXEC_REMOTE_DOMAIN").as_deref() == Ok("dom0"),
        "airlock.InputReceiverReady is restricted to dom0"
    );
    ensure!(
        std::env::var_os("QREXEC_SERVICE_ARGUMENT").is_none_or(|value| value.is_empty()),
        "airlock.InputReceiverReady does not accept arguments"
    );

    ensure!(
        fs::metadata(&request.uinput).is_ok_and(|metadata| metadata.file_type().is_char_device())
            && rustix::fs::access(&request.uinput, rustix::fs::Access::WRITE_OK).is_ok(),
        "/dev/uinput is not ready"
    );
    for handler in request.handlers {
        ensure!(
            rustix::fs::access(&handler, rustix::fs::Access::EXEC_OK).is_ok(),
            "input receiver is unavailable: {handler}",
            handler = handler.display()
        );
    }

    Ok(())
}
