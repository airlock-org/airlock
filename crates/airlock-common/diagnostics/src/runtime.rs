//! Trusted Qubes identity, approval-provider selection and bounded evidence adapters.
//! Logs remain untrusted and may contain secrets; this endpoint performs no redaction.

mod process;

use crate::{authority, *};
use anyhow::{Context, Result, ensure};
use rustix::fs::{FlockOperation, Mode, OFlags, flock, open, openat};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    os::{fd::AsFd, unix::net::UnixDatagram},
    path::{Component, Path},
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// Protected installed endpoint configuration, never selected by RPC data.
pub const CONFIG_PATH: &str = "/etc/airlock/diagnostics.json";

/// Immutable host configuration for the self-only return path.
pub const RELAY_CONFIG_PATH: &str = "/etc/airlock/diagnostics-relay.json";

/// Separate dom0 RPC that can return a request only to its authenticated caller.
pub const RELAY_SERVICE: &str = "airlock.Diagnostics.Self";

/// Protected relay configuration, installed only on dom0.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelayConfig {
    /// Exact relay contract version.
    pub schema: String,
    /// Observer allowed to request the return path; every other caller is rejected.
    pub observer: CallerId,
}

/// Bound a request's complete lifetime, including input from an uncooperative client.
const REQUEST_SECONDS: u64 = 45;

/// Return observation timestamps without making wall-clock accuracy an authorization assumption.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

/// Read one small regular source without following any path component's symlink.
fn regular(path: &Path) -> Result<File> {
    ensure!(path.is_absolute(), "diagnostic path must be absolute");
    let mut directory = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let parts = path.components().skip(1).collect::<Vec<_>>();
    ensure!(!parts.is_empty(), "diagnostic path names a directory");
    for (index, part) in parts.iter().enumerate() {
        let Component::Normal(name) = part else {
            anyhow::bail!("invalid diagnostic path component");
        };
        let last = index + 1 == parts.len();
        let flags = OFlags::RDONLY
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK
            | OFlags::CLOEXEC
            | if last {
                OFlags::empty()
            } else {
                OFlags::DIRECTORY
            };
        directory = openat(&directory, Path::new(name), flags, Mode::empty())?;
    }
    let file = File::from(directory);
    ensure!(
        file.metadata()?.is_file(),
        "diagnostic source is not a regular file"
    );
    Ok(file)
}

/// Kernel-provided boot identity, sampled again after collection to detect changed provenance.
fn boot() -> Result<BootId> {
    let mut value = String::new();
    regular(Path::new("/proc/sys/kernel/random/boot_id"))?
        .take(128)
        .read_to_string(&mut value)?;
    let value = value.trim().to_owned();
    value.parse().context("kernel boot identity unavailable")
}

/// Exact accepted query and source, owned together so dispatch cannot substitute either.
struct ReadPlan {
    /// Opaque result of the verified authorization gate.
    permit: authority::Permit,
    /// Complete operation retained from the checked request.
    query: Query,
    /// Installed source, absent only for inventory discovery.
    source: Option<Source>,
}

impl ReadPlan {
    /// Bind the entire query, caller and installed contract before selecting automatic approval.
    fn check(config: &ServerConfig, caller: &CallerId, query: Query) -> Result<Self> {
        config.validate()?;
        query.validate()?;
        let source = match &query.action {
            Operation::Catalog {} => None,
            Operation::Read { source, options } => {
                let selected = config
                    .sources
                    .iter()
                    .find(|entry| &entry.id == source)
                    .context("diagnostic source is not declared")?;
                ensure!(
                    options.unit.is_none()
                        || matches!(
                            selected.reader,
                            Reader::Journal {} | Reader::UserJournal { .. }
                        ),
                    "unit filters require a journal source"
                );
                Some(selected.clone())
            }
        };
        // Catalog approval binds its complete inventory; read approval binds the exact reader.
        let descriptor = serde_json::to_string(&source)?;
        let mut sources = config
            .sources
            .iter()
            .map(|entry| serde_json::to_string(&Some(entry)))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        sources.push(serde_json::to_string(&Option::<Source>::None)?);
        let binding = serde_json::to_string(&(caller, config, &query))?;
        let request = authority::Request {
            caller: caller.as_str().to_owned(),
            target: query.target.as_str().to_owned(),
            source: descriptor,
            binding,
            bytes: match &query.action {
                Operation::Catalog {} => 1,
                Operation::Read { options, .. } => options.max_bytes,
            },
        };
        let policy = authority::Policy {
            target: config.target.as_str().to_owned(),
            callers: config
                .callers
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
            sources,
            ceiling: config.max_bytes,
        };
        // This is the sole approval-provider selection. No client field can enable it.
        let decision = authority::Decision {
            binding: request.binding.clone(),
            approved: config.approval == ApprovalMode::Automatic,
        };
        let permit = authority::Permit::check(&policy, request, &decision)
            .context("diagnostic access denied")?;
        Ok(Self {
            permit,
            query,
            source,
        })
    }

    /// Dispatch only the source retained when authority was checked.
    fn collect(&self, config: &ServerConfig, response: &mut Response) -> Result<()> {
        let Some(source) = &self.source else {
            response.sources = config.sources.clone();
            response.outcome = Outcome::Complete;
            return Ok(());
        };
        let Operation::Read { options, .. } = &self.query.action else {
            unreachable!("checked source belongs to a read");
        };
        let bytes = self.permit.request().bytes as usize;
        let captured = match &source.reader {
            Reader::File { path, .. } | Reader::StoreFile { path } => {
                let path = if matches!(source.reader, Reader::StoreFile { .. }) {
                    let resolved = fs::canonicalize(path)?;
                    ensure!(
                        resolved.starts_with("/nix/store"),
                        "configuration source does not resolve into the Nix store"
                    );
                    resolved
                } else {
                    Path::new(path).to_owned()
                };
                let mut file = regular(&path)?;
                let tail = matches!(source.reader, Reader::File { tail: true, .. });
                let length = file.metadata()?.len();
                let skipped = tail && length > bytes as u64;
                if skipped {
                    file.seek(SeekFrom::Start(length - bytes as u64))?;
                }
                let mut content = Vec::new();
                file.take(bytes as u64 + 1).read_to_end(&mut content)?;
                let truncated = skipped || content.len() > bytes;
                content.truncate(bytes);
                process::Captured {
                    stdout: content,
                    stderr: Vec::new(),
                    success: true,
                    truncated,
                    timed_out: false,
                }
            }
            reader => {
                let mut command = reader_command(
                    reader,
                    options,
                    response
                        .boot
                        .as_ref()
                        .context("missing boot identity")?
                        .as_str(),
                )?;
                process::capture(&mut command, &[], bytes, Duration::from_secs(20))?
            }
        };
        response.lossy = std::str::from_utf8(&captured.stdout).is_err()
            || std::str::from_utf8(&captured.stderr).is_err();
        response.stdout = String::from_utf8_lossy(&captured.stdout).into_owned();
        response.stderr = String::from_utf8_lossy(&captured.stderr).into_owned();
        response.outcome = if captured.timed_out {
            Outcome::TimedOut
        } else if captured.truncated {
            Outcome::Truncated
        } else if captured.success {
            Outcome::Complete
        } else {
            Outcome::Failed
        };
        Ok(())
    }
}

/// Build argv solely from a closed reader enum and validated journal parameters.
fn reader_command(reader: &Reader, options: &ReadOptions, boot: &str) -> Result<Command> {
    let (program, args): (&str, Vec<String>) = match reader {
        Reader::Journal {} | Reader::UserJournal { .. } | Reader::Kernel {} => {
            let mut args = vec![
                "--no-pager".into(),
                "--quiet".into(),
                "--output=short-iso-precise".into(),
                format!("--boot={boot}"),
                format!("--lines={lines}", lines = options.lines),
            ];
            match reader {
                Reader::Journal {} => {
                    if let Some(unit) = &options.unit {
                        args.push(format!("--unit={unit}", unit = unit.as_str()));
                    }
                }
                Reader::UserJournal { uid } => {
                    args.push(format!("_UID={uid}"));
                    if let Some(unit) = &options.unit {
                        args.push(format!("_SYSTEMD_USER_UNIT={unit}", unit = unit.as_str()));
                    }
                }
                Reader::Kernel {} => args.push("--dmesg".into()),
                _ => unreachable!(),
            }
            ("journalctl", args)
        }
        Reader::Probe { probe } => {
            let (program, args): (&str, &[&str]) = match probe {
                Probe::Services => (
                    "systemctl",
                    &[
                        "list-units",
                        "--all",
                        "--type=service",
                        "--output=json",
                        "--no-pager",
                    ],
                ),
                Probe::Pci => ("lspci", &["-nnk"]),
                Probe::Addresses => ("ip", &["-details", "-json", "address", "show"]),
                Probe::Routes => ("ip", &["-json", "route", "show", "table", "all"]),
                Probe::Mounts => ("findmnt", &["--json"]),
                Probe::XenInfo => ("xl", &["info"]),
                Probe::XenLog => ("xl", &["dmesg"]),
                Probe::Qubes => ("qvm-ls", &["--raw-data", "--fields", "NAME,STATE,XID"]),
            };
            (
                program,
                args.iter().map(|value| (*value).to_owned()).collect(),
            )
        }
        Reader::File { .. } | Reader::StoreFile { .. } => {
            anyhow::bail!("file reader cannot execute commands")
        }
    };
    // Packages install these tools explicitly. Neither the request nor a caller PATH selects them.
    let mut command = Command::new(format!("/run/current-system/sw/bin/{program}"));
    command
        .args(args)
        .current_dir("/")
        .env_clear()
        .env("PATH", "/run/current-system/sw/bin")
        .env("LC_ALL", "C")
        .env("SYSTEMD_PAGER", "cat");
    Ok(command)
}

/// Audit only access metadata; never duplicate evidence, reasons or query contents into logs.
fn audit(event: &Value) -> Result<()> {
    let socket = UnixDatagram::unbound()?;
    socket.set_nonblocking(true)?;
    socket.connect("/run/systemd/journal/socket")?;
    socket.send(
        format!("PRIORITY=6\nSYSLOG_IDENTIFIER=airlock-diagnostics\nMESSAGE={event}\n").as_bytes(),
    )?;
    Ok(())
}

/// Execute one query after authorization and a successful pre-read audit acknowledgment.
fn handle(
    config: &ServerConfig,
    caller: &CallerId,
    query: Query,
    log: impl Fn(&Value) -> Result<()>,
) -> Response {
    let mut response = Response {
        schema: RESPONSE_SCHEMA.into(),
        id: query.id.clone(),
        target: config.target.clone(),
        boot: None,
        started_ms: now(),
        finished_ms: now(),
        approval: ApprovalMode::Deny,
        outcome: Outcome::Denied,
        sources: Vec::new(),
        stdout: String::new(),
        stderr: String::new(),
        lossy: false,
    };
    let operation = match &query.action {
        Operation::Catalog {} => "catalog",
        Operation::Read { .. } => "read",
    };
    let source = match &query.action {
        Operation::Catalog {} => None,
        Operation::Read { source, .. } => Some(source.clone()),
    };
    let event = |phase: &str, outcome: Outcome| {
        json!({"schema":"airlock.diagnostics-audit.v1", "request":response.id,
        "caller":caller,"target":config.target,"source":source,"operation":operation,"phase":phase,"outcome":outcome})
    };

    let plan = match ReadPlan::check(config, caller, query) {
        Ok(plan) => plan,
        Err(_) => {
            let _ = log(&event("refused", Outcome::Denied));
            response.stderr = "diagnostic caller, target, source, parameters or approval policy refused the request".into();
            return response;
        }
    };
    // A full or unavailable audit channel prevents evidence access rather than silently bypassing it.
    if log(&event("authorized_automatic", Outcome::Complete)).is_err() {
        response.outcome = Outcome::Unavailable;
        response.stderr = "diagnostic access audit is unavailable".into();
        return response;
    }
    response.approval = ApprovalMode::Automatic;
    let collection = (|| -> Result<()> {
        let before = boot()?;
        if let Operation::Read { options, .. } = &plan.query.action
            && options
                .boot
                .as_ref()
                .is_some_and(|expected| expected != &before)
        {
            response.boot = Some(before);
            response.outcome = Outcome::StaleBoot;
            return Ok(());
        }
        response.boot = Some(before.clone());
        plan.collect(config, &mut response)?;
        if boot()? != before {
            response.stdout.clear();
            response.stderr.clear();
            response.sources.clear();
            response.outcome = Outcome::StaleBoot;
        }
        Ok(())
    })();
    if let Err(error) = collection {
        response.outcome = Outcome::Unavailable;
        response.stderr = format!("diagnostic reader unavailable: {error}");
    }
    response.finished_ms = now();
    // A failed completion acknowledgment cannot erase the already-recorded access.
    let _ = log(
        &json!({"schema":"airlock.diagnostics-audit.v1","phase":"finished","request":response.id,
        "caller":caller,"target":config.target,"source":source,"operation":operation,"outcome":response.outcome}),
    );
    response
}

/// Bound concurrent endpoint processes with protected nonblocking slot locks.
fn slot() -> Result<File> {
    let root = Path::new("/run/airlock-diagnostics");
    fs::create_dir_all(root)?;
    let directory = open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    for index in 0..4 {
        let file = File::from(openat(
            &directory,
            format!("slot-{index}").as_str(),
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::RUSR | Mode::WUSR,
        )?);
        ensure!(file.metadata()?.is_file(), "invalid diagnostic slot");
        if flock(&file, FlockOperation::NonBlockingLockExclusive).is_ok() {
            return Ok(file);
        }
    }
    anyhow::bail!("diagnostic endpoint is busy; retry later")
}

/// Read a complete request until EOF without allowing a caller to hold a slot indefinitely.
fn read_request(input: &mut impl Read, descriptor: &impl AsFd) -> Result<Vec<u8>> {
    rustix::fs::fcntl_setfl(
        descriptor,
        rustix::fs::fcntl_getfl(descriptor)? | OFlags::NONBLOCK,
    )?;
    let started = Instant::now();
    let mut bytes = Vec::new();
    loop {
        ensure!(
            started.elapsed() < Duration::from_secs(5),
            "diagnostic request input timed out"
        );
        let mut buffer = [0; 1024];
        match input.read(&mut buffer) {
            Ok(0) => return Ok(bytes),
            Ok(size) => {
                ensure!(
                    size <= REQUEST_LIMIT - bytes.len(),
                    "diagnostic request exceeds 8 KiB"
                );
                bytes.extend_from_slice(&buffer[..size]);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(error) => return Err(error.into()),
        }
    }
}

/// Bound writing to a client that stops consuming evidence.
fn write_response(response: &Response) -> Result<()> {
    let mut bytes = serde_json::to_vec(response)?;
    bytes.push(b'\n');
    let stdout = std::io::stdout();
    rustix::fs::fcntl_setfl(
        &stdout,
        rustix::fs::fcntl_getfl(&stdout)? | OFlags::NONBLOCK,
    )?;
    let mut output = stdout.lock();
    let started = Instant::now();
    let mut sent = 0;
    while sent < bytes.len() {
        ensure!(
            started.elapsed() < Duration::from_secs(5),
            "diagnostic response output timed out"
        );
        match output.write(&bytes[sent..]) {
            Ok(0) => anyhow::bail!("diagnostic response pipe closed"),
            Ok(size) => sent += size,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Serve a single Qubes call using only installed configuration and Qubes caller identity.
pub fn serve() -> Result<()> {
    ensure!(
        std::env::var_os("QREXEC_SERVICE_ARGUMENT").is_none_or(|value| value.is_empty()),
        "diagnostics takes no service argument"
    );
    let caller: CallerId = std::env::var("QREXEC_REMOTE_DOMAIN")
        .context("Qubes caller identity is missing")?
        .parse()?;
    let _slot = slot()?;
    let mut bytes = Vec::new();
    // The immutable /etc contract legitimately resolves through NixOS's store symlinks.
    File::open(CONFIG_PATH)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 1024 * 1024,
        "diagnostic contract exceeds 1 MiB"
    );
    let config: ServerConfig = serde_json::from_slice(&bytes)?;
    config.validate()?;
    let stdin = std::io::stdin();
    let input = read_request(&mut stdin.lock(), &stdin)?;
    let query = decode_query(&input)?;
    let response = handle(&config, &caller, query, audit);
    write_response(&response)
}

/// Forward a request back to its authenticated observer through dom0's existing Qubes client.
/// The verified permission supplies the sole destination; no arbitrary RPC or argv is accepted.
pub fn relay() -> Result<()> {
    ensure!(
        std::env::var_os("QREXEC_SERVICE_ARGUMENT").is_none_or(|value| value.is_empty()),
        "diagnostic relay takes no service argument"
    );
    let caller: CallerId = std::env::var("QREXEC_REMOTE_DOMAIN")
        .context("Qubes caller identity is missing")?
        .parse()?;
    let _slot = slot()?;
    let mut config = Vec::new();
    File::open(RELAY_CONFIG_PATH)?
        .take(8193)
        .read_to_end(&mut config)?;
    ensure!(config.len() <= 8192, "relay contract too large");
    let config: RelayConfig = serde_json::from_slice(&config)?;
    ensure!(
        config.schema == "airlock.diagnostics-relay.v1",
        "unknown relay contract schema"
    );
    let stdin = std::io::stdin();
    let bytes = read_request(&mut stdin.lock(), &stdin)?;
    let query = decode_query(&bytes)?;
    let permission = authority::SelfRelay::check(
        config.observer.as_str(),
        caller.as_str(),
        query.target.as_str().to_owned(),
    )
    .context("diagnostic relay may only return to its authenticated observer")?;
    audit(
        &json!({"schema":"airlock.diagnostics-audit.v1","phase":"relay","request":query.id,"caller":caller,"target":permission.target()}),
    )?;
    let mut command = Command::new("/run/current-system/sw/bin/qrexec-client");
    command
        .args([
            "-d",
            permission.target(),
            "-T",
            "-w",
            "20",
            "root:QUBESRPC airlock.Diagnostics+ dom0",
        ])
        .current_dir("/")
        .env_clear()
        .env("PATH", "/run/current-system/sw/bin");
    let response = exchange(&mut command, &query)?;
    write_response(&response)
}

/// Send a bounded request through Qubes and verify response correlation and target identity.
pub fn request(query: &Query) -> Result<Response> {
    let local = fs::read_to_string("/proc/sys/kernel/hostname")?;
    let mut command = Command::new("qrexec-client-vm");
    if local.trim() == query.target.as_str() {
        command.args(["dom0", RELAY_SERVICE]);
    } else {
        command.args([query.target.as_str(), SERVICE]);
    }
    exchange(&mut command, query)
}

/// Share transport bounding and response checks between the ordinary client and self relay.
fn exchange(command: &mut Command, query: &Query) -> Result<Response> {
    query.validate()?;
    let bytes = serde_json::to_vec(query)?;
    ensure!(
        bytes.len() <= REQUEST_LIMIT,
        "diagnostic request exceeds 8 KiB"
    );
    let captured = process::capture(
        command,
        &bytes,
        8 * OUTPUT_LIMIT as usize,
        Duration::from_secs(REQUEST_SECONDS),
    )?;
    ensure!(
        !captured.timed_out && !captured.truncated,
        "diagnostic transport timed out or exceeded its output limit"
    );
    ensure!(
        captured.success,
        "diagnostic transport failed: {error}",
        error = String::from_utf8_lossy(&captured.stderr)
    );
    let response: Response = serde_json::from_slice(&captured.stdout)?;
    ensure!(
        response.schema == RESPONSE_SCHEMA
            && response.id == query.id
            && response.target == query.target,
        "diagnostic response differs from the requested identity"
    );
    Ok(response)
}

#[cfg(test)]
mod tests;
