//! A private pipe lets an authenticated USB helper wait while an unprivileged process builds.
//! Closing the pipe without an explicit success message cancels provisioning.

use anyhow::{Result, ensure};
use std::{
    fs::File,
    io::{Read, Write},
};

/// Helper response emitted only after device checks and credential collection finish.
pub const READY: &[u8] = b"airlock-usb-prepared-v1\n";

/// Build owner's explicit authorization to consume this run's newly published image.
pub const BUILD_SUCCEEDED: &[u8] = b"airlock-image-build-succeeded-v1\n";

/// Reserve stdout for the readiness frame while sending ordinary helper output to the terminal.
/// Child tools cannot inherit this pipe, and device or Git log text cannot impersonate readiness.
pub(crate) fn reply_pipe() -> Result<File> {
    let reply = File::from(rustix::io::fcntl_dupfd_cloexec(std::io::stdout(), 3)?);
    rustix::stdio::dup2_stdout(std::io::stderr())?;
    Ok(reply)
}

/// Announce completed preparation, then require one exact success message followed by EOF.
/// Call this only after collecting credentials and checking the selected USB, before any writes.
/// Cancellation, truncated messages and extra input all fail without authorizing provisioning.
pub fn wait_for_build(input: impl Read, mut output: impl Write) -> Result<()> {
    output.write_all(READY)?;
    output.flush()?;

    let mut message = Vec::new();
    input
        .take((BUILD_SUCCEEDED.len() + 1) as u64)
        .read_to_end(&mut message)?;
    ensure!(
        message == BUILD_SUCCEEDED,
        "Image build did not authorize provisioning; USB was not written."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only a complete success frame with no trailing input permits the destructive phase.
    #[test]
    fn cancellation_and_invalid_completion_never_authorize_provisioning() -> Result<()> {
        for input in [
            Vec::new(),
            b"failed\n".to_vec(),
            BUILD_SUCCEEDED[..BUILD_SUCCEEDED.len() - 1].to_vec(),
            [BUILD_SUCCEEDED, b"extra"].concat(),
            BUILD_SUCCEEDED.to_vec(),
        ] {
            let mut output = Vec::new();
            let result = wait_for_build(input.as_slice(), &mut output);
            assert_eq!(output, READY);
            assert_eq!(result.is_ok(), input == BUILD_SUCCEEDED);
        }
        Ok(())
    }
}
