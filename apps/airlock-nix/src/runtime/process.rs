//! Child ownership and bounded capture. Locks stay with the orchestrator, never
//! an exec'd child. Dropping a pipe owner terminates and reaps its child.
use super::{Error, Result, require};
use std::{
    io::{Read, Write},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub(super) const LIMIT: usize = 32 * 1024 * 1024;
pub(super) struct OwnedChild(pub Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
pub(super) struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
impl Output {
    pub fn checked(self, operation: &str) -> Result<Vec<u8>> {
        if !self.status.success() {
            return Err(Error::Remote {
                operation: operation.to_owned(),
                message: String::from_utf8_lossy(&self.stderr)
                    .chars()
                    .take(4096)
                    .collect(),
                details: Box::new(
                    serde_json::json!({"exit_code":self.status.code(),"stdout":String::from_utf8_lossy(&self.stdout).chars().take(4096).collect::<String>(),"stderr":String::from_utf8_lossy(&self.stderr).chars().take(4096).collect::<String>()}),
                ),
            });
        }
        Ok(self.stdout)
    }
}
fn drain(
    mut input: impl Read,
    overflow: &AtomicBool,
    forward_stderr: bool,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 16384];
    loop {
        let size = input.read(&mut buffer)?;
        if size == 0 {
            return Ok(bytes);
        }
        if bytes.len() + size > LIMIT {
            overflow.store(true, Ordering::Release);
            return Ok(bytes);
        }
        if forward_stderr {
            let _ = std::io::stderr().write_all(&buffer[..size]);
        }
        bytes.extend_from_slice(&buffer[..size]);
    }
}
pub(super) fn run(
    command: &mut Command,
    input: Option<Vec<u8>>,
    timeout: Option<Duration>,
) -> Result<Output> {
    run_capture(command, input, timeout, false)
}
fn run_capture(
    command: &mut Command,
    input: Option<Vec<u8>>,
    timeout: Option<Duration>,
    forward_stderr: bool,
) -> Result<Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if input.is_some() {
        command.stdin(Stdio::piped());
    } else {
        command.stdin(Stdio::null());
    }
    let mut child = OwnedChild(command.spawn().map_err(|source| Error::Command {
        program: command.get_program().to_string_lossy().into_owned(),
        source,
    })?);
    let overflow = Arc::new(AtomicBool::new(false));
    thread::scope(|scope| -> Result<Output> {
        let stdout = child.0.stdout.take().expect("piped stdout");
        let stderr = child.0.stderr.take().expect("piped stderr");
        let out = scope.spawn(|| drain(stdout, &overflow, false));
        let err = scope.spawn(|| drain(stderr, &overflow, forward_stderr));
        let writer = input.map(|bytes| {
            let mut pipe = child.0.stdin.take().expect("piped stdin");
            scope.spawn(move || pipe.write_all(&bytes))
        });
        let start = Instant::now();
        let mut timed_out = false;
        let status = loop {
            if let Some(status) = child.0.try_wait()? {
                break status;
            }
            if overflow.load(Ordering::Acquire)
                || timeout.is_some_and(|limit| start.elapsed() > limit)
            {
                timed_out = !overflow.load(Ordering::Acquire);
                child.0.kill()?;
                break child.0.wait()?;
            }
            thread::sleep(Duration::from_millis(25));
        };
        let stdout = out
            .join()
            .map_err(|_| Error::Message("stdout reader failed".into()))??;
        let stderr = err
            .join()
            .map_err(|_| Error::Message("stderr reader failed".into()))??;
        if let Some(writer) = writer {
            let sent = writer
                .join()
                .map_err(|_| Error::Message("stdin writer failed".into()))?;
            if status.success() {
                sent?;
            }
        }
        require(
            !timed_out,
            "child command timed out; no success was recorded",
        )?;
        require(
            !overflow.load(Ordering::Acquire),
            "child output exceeded the capture limit",
        )?;
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    })
}
pub(super) fn output(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    run_capture(Command::new(program).args(args), None, None, true)?.checked(program)
}
pub(super) fn text(program: &str, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8(output(program, args)?)
        .map_err(|_| Error::Message("command returned non-UTF-8 text".into()))?
        .trim()
        .to_owned())
}
pub(super) fn rpc(
    service: &str,
    request: &impl serde::Serialize,
    timeout: Option<Duration>,
) -> Result<serde_json::Value> {
    let mut bytes = serde_json::to_vec(request)?;
    bytes.push(b'\n');
    let result = run(
        Command::new("qrexec-client-vm").args(["dom0", service]),
        Some(bytes),
        timeout,
    )?;
    let parsed = super::decode::<serde_json::Value>(&result.stdout);
    if !result.status.success()
        || parsed.is_err()
        || parsed
            .as_ref()
            .ok()
            .is_some_and(|x| x.get("error").is_some())
    {
        let diagnostic = parsed.as_ref().err().map(ToString::to_string);
        return Err(Error::Remote {
            operation: service.to_owned(),
            message: parsed
                .as_ref()
                .ok()
                .and_then(|v| v.get("error"))
                .map(ToString::to_string)
                .unwrap_or_else(|| {
                    String::from_utf8_lossy(&result.stderr)
                        .chars()
                        .take(4096)
                        .collect::<String>()
                        + &diagnostic
                            .map(|error| format!("; invalid response: {error}"))
                            .unwrap_or_default()
                }),
            details: Box::new(
                serde_json::json!({"exit_code":result.status.code(),"response":parsed.ok(),"stdout":String::from_utf8_lossy(&result.stdout).chars().take(4096).collect::<String>(),"stderr":String::from_utf8_lossy(&result.stderr).chars().take(4096).collect::<String>()}),
            ),
        });
    }
    let value = parsed?;
    require(value.is_object(), "RPC response must be an object")?;
    Ok(value)
}

/// A failed deployment phase is a successful status observation. Preserve its
/// diagnostic `error` field rather than treating it as a failed RPC operation.
pub(super) fn machine_status() -> Result<serde_json::Value> {
    let bytes = serde_json::to_vec(
        &serde_json::json!({"schema":"airlock.machine-control.v1","action":"status"}),
    )?;
    let result = run(
        Command::new("qrexec-client-vm").args(["dom0", "airlock.Ctrl.MachineControl"]),
        Some([bytes, vec![b'\n']].concat()),
        Some(Duration::from_secs(20)),
    )?;
    let raw = result.checked("observe machine status")?;
    let value: serde_json::Value = super::decode(&raw)?;
    require(
        value.is_object()
            && value
                .get("phase")
                .and_then(serde_json::Value::as_str)
                .is_some(),
        "machine observation has no deployment phase",
    )?;
    Ok(value)
}
