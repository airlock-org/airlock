//! Bounded interactive JSON peers let tests observe a handshake before supplying the next request bytes.

use anyhow::{Context, Result, ensure};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use serde_json::Value;
use std::{
    io::{Read, Write},
    os::unix::process::CommandExt,
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};

/// Own one interactive process and cancel its process group if a scenario fails mid-handshake.
pub struct Dialogue {
    // Leader stays owned until completion or cancellation.
    child: Child,
    // Closed explicitly to signal request EOF while keeping the reply stream readable.
    input: Option<ChildStdin>,
    // Unbuffered reply bytes are polled before every read, including between JSON frames.
    output: ChildStdout,
    // Keep stderr on a regular file so diagnostic output cannot block the protocol.
    errors: tempfile::NamedTempFile,
}

impl Dialogue {
    /// Spawn an explicitly prepared process with independent request and reply pipes.
    pub fn spawn(command: &mut Command) -> Result<Self> {
        let errors = tempfile::NamedTempFile::new()?;
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(errors.reopen()?))
            .process_group(0)
            .spawn()?;
        Ok(Self {
            input: child.stdin.take(),
            output: child.stdout.take().context("interactive output")?,
            child,
            errors,
        })
    }

    /// Write and flush the next bounded protocol segment without closing the request stream.
    pub fn send(&mut self, bytes: &[u8]) -> Result<()> {
        let input = self
            .input
            .as_mut()
            .context("request stream already closed")?;
        input.write_all(bytes)?;
        input.flush()?;
        Ok(())
    }

    /// Read exactly one JSON line under a deadline, independent of whether the sender has closed stdin.
    pub fn reply(&mut self, timeout: Duration) -> Result<Value> {
        Ok(serde_json::from_slice(&self.line(timeout)?)?)
    }

    /// Read one bounded newline-terminated protocol frame, including non-JSON tool readiness replies.
    pub fn line(&mut self, timeout: Duration) -> Result<Vec<u8>> {
        self.until(b"\n", timeout)
    }

    /// Read a bounded terminal or protocol prompt under a deadline, excluding its terminating marker.
    pub fn until(&mut self, marker: &[u8], timeout: Duration) -> Result<Vec<u8>> {
        ensure!(!marker.is_empty(), "an interactive marker cannot be empty");
        let deadline = Instant::now() + timeout;
        let mut bytes = Vec::new();
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .with_context(|| format!("interactive reply timed out; received {bytes:?}"))?;
            let mut fds = [PollFd::new(&self.output, PollFlags::IN)];
            match poll(&mut fds, Some(&Timespec::try_from(remaining)?)) {
                Ok(0) => anyhow::bail!("interactive reply timed out; received {bytes:?}"),
                Ok(_) => {}
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
            let mut byte = [0];
            ensure!(
                self.output.read(&mut byte)? == 1,
                "interactive peer closed its reply: {errors}",
                errors = self.errors()?
            );
            bytes.push(byte[0]);
            if bytes.ends_with(marker) {
                bytes.truncate(bytes.len() - marker.len());
                return Ok(bytes);
            }
            ensure!(
                bytes.len() <= 1024 * 1024,
                "interactive reply exceeds limit"
            );
        }
    }

    /// Observe whether the owned process is still running without relinquishing cleanup ownership.
    pub fn running(&mut self) -> Result<bool> {
        Ok(self.child.try_wait()?.is_none())
    }

    /// Close requests and wait only for the owned process's bounded shutdown.
    pub fn finish(&mut self) -> Result<std::process::ExitStatus> {
        drop(self.input.take());
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait()? {
                return Ok(status);
            }
            ensure!(
                Instant::now() < deadline,
                "interactive peer did not stop: {errors}",
                errors = self.errors()?
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Retain original helper diagnostics for failed protocol assertions.
    pub fn errors(&self) -> Result<String> {
        Ok(std::fs::read_to_string(self.errors.path())?)
    }
}

impl Drop for Dialogue {
    fn drop(&mut self) {
        drop(self.input.take());
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = rustix::process::kill_process_group(
                rustix::process::Pid::from_child(&self.child),
                rustix::process::Signal::KILL,
            );
            let _ = self.child.wait();
        }
    }
}
