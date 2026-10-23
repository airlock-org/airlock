//! Capture fixed readers with a combined byte budget and a process-group deadline.

use anyhow::{Result, ensure};
use rustix::{
    fs::{OFlags, fcntl_getfl, fcntl_setfl},
    process::{Pid, Signal, WaitId, WaitIdOptions, kill_process_group, waitid},
};
use std::{
    io::{Read, Write},
    os::{fd::AsFd, unix::process::CommandExt},
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

/// One bounded collection; stdout and stderr consume the same budget.
pub(super) struct Captured {
    /// Evidence bytes returned by the selected reader.
    pub stdout: Vec<u8>,
    /// Reader diagnostics, subject to the same output ceiling.
    pub stderr: Vec<u8>,
    /// Reader finished with a successful exit status.
    pub success: bool,
    /// Collection encountered output beyond its budget.
    pub truncated: bool,
    /// Reader or an inherited pipe failed to finish before the deadline.
    pub timed_out: bool,
}

/// Own the process group so descendants cannot retain diagnostic pipes after a request ends.
struct ChildGroup {
    /// Direct child, always reaped before releasing its group ownership.
    child: Option<Child>,
    /// Separate process group created at spawn; the collector never joins it.
    group: Pid,
}

impl Drop for ChildGroup {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = kill_process_group(self.group, Signal::KILL);
            let _ = child.wait();
        }
    }
}

impl ChildGroup {
    /// Observe exit without reaping, keeping the process-group identity reserved until cleanup.
    fn exited(&self) -> Result<bool> {
        loop {
            match waitid(
                WaitId::Pid(self.group),
                WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
            ) {
                Ok(status) => return Ok(status.is_some()),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Signal the owned group before reaping its leader; never signal a recycled process identity.
    fn finish(&mut self) -> Result<ExitStatus> {
        match kill_process_group(self.group, Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(error.into()),
        }
        let status = self.child.as_mut().expect("owned child").wait()?;
        self.child = None;
        Ok(status)
    }
}

/// Preserve descriptor flags while ensuring a pipe cannot block the deadline loop.
fn nonblocking(pipe: &impl AsFd) -> Result<()> {
    fcntl_setfl(pipe, fcntl_getfl(pipe)? | OFlags::NONBLOCK)?;
    Ok(())
}

/// Read at most one pipe chunk and distinguish EOF from temporary lack of input.
fn drain<T: Read>(
    pipe: &mut Option<T>,
    bytes: &mut Vec<u8>,
    remaining: &mut crate::authority::Budget,
) -> Result<bool> {
    let Some(reader) = pipe else {
        return Ok(false);
    };
    let mut buffer = [0; 8192];
    match reader.read(&mut buffer) {
        Ok(0) => *pipe = None,
        Ok(size) => {
            let keep = remaining.consume(size as u64) as usize;
            bytes.extend_from_slice(&buffer[..keep]);
            return Ok(keep < size);
        }
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) => {}
        Err(error) => return Err(error.into()),
    }
    Ok(false)
}

/// Execute a fixed command while bounding input delivery, both output pipes and descendants.
pub(super) fn capture(
    command: &mut Command,
    input: &[u8],
    bytes: usize,
    timeout: Duration,
) -> Result<Captured> {
    ensure!(bytes > 0, "empty collection budget");
    let child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;
    let group = Pid::from_raw(child.id() as i32).expect("positive child PID");
    let mut child = ChildGroup {
        child: Some(child),
        group,
    };
    let owned = child.child.as_mut().expect("owned child");
    let mut stdout = owned.stdout.take();
    let mut stderr = owned.stderr.take();
    let mut stdin = owned.stdin.take();
    nonblocking(stdout.as_ref().unwrap())?;
    nonblocking(stderr.as_ref().unwrap())?;
    nonblocking(stdin.as_ref().unwrap())?;

    let started = Instant::now();
    let mut result = Captured {
        stdout: Vec::new(),
        stderr: Vec::new(),
        success: false,
        truncated: false,
        timed_out: false,
    };
    let mut remaining = crate::authority::Budget::new(bytes as u64);
    let mut sent = 0;
    let mut exited = false;
    loop {
        if started.elapsed() >= timeout {
            result.timed_out = true;
            break;
        }
        if sent == input.len() {
            stdin = None;
        }
        if let Some(writer) = &mut stdin {
            match writer.write(&input[sent..]) {
                Ok(0) => anyhow::bail!("reader closed its request pipe"),
                Ok(size) => sent += size,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }

        result.truncated = drain(&mut stdout, &mut result.stdout, &mut remaining)?;
        if !result.truncated {
            result.truncated = drain(&mut stderr, &mut result.stderr, &mut remaining)?;
        }
        if result.truncated {
            break;
        }
        if !exited {
            exited = child.exited()?;
        }
        if stdout.is_none() && stderr.is_none() && exited {
            result.success = child.finish()?.success();
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(result)
}
