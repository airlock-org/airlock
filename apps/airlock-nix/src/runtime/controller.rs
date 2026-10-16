use super::{
    Error, Result, build, decode, emit, now, process,
    protocol::{self, Header},
    records, report, require,
};
use crate::review::ApprovedChange;
use crate::{
    identity::*,
    job::{ApprovedStatus, JobSnapshot, JobStatus, StopReason},
    review::{
        Decision, PendingReview, ReviewObservation, ReviewOutcome, ReviewRequest, ReviewResponse,
        Scope,
    },
    state::{OperationKind, OperationLock, StateStore},
};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs::File,
    io::Seek,
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(
    version,
    about = "Controller-side source review, detached jobs and Nix deployment orchestration"
)]
struct Args {
    #[command(subcommand)]
    command: ControllerCommand,
}
#[derive(Subcommand)]
enum ControllerCommand {
    Serve,
    Work { request_id: JobId },
    Observe,
    ReviewStatusServe,
}
pub fn main() -> ExitCode {
    report(run(Args::parse().command))
}
fn run(command: ControllerCommand) -> Result<u8> {
    match command {
        ControllerCommand::Serve => serve(),
        ControllerCommand::Work { request_id } => work(request_id),
        ControllerCommand::Observe => observe(),
        ControllerCommand::ReviewStatusServe => {
            require(
                std::env::var("QREXEC_REMOTE_DOMAIN").as_deref() == Ok("usr-assistant"),
                "only usr-assistant may query review feedback",
            )?;
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Request {
                request_id: JobId,
            }
            let request: Request = protocol::Upload::new().header()?;
            emit(&status(&request.request_id, true)?)?;
            Ok(0)
        }
    }
}
fn repo() -> PathBuf {
    PathBuf::from("/home/user/dev/airlock-machine")
}
fn operation_key() -> Result<crate::state::ResourceKey> {
    records::key("controller")
}
fn acquire(store: &StateStore, id: Option<&JobId>) -> Result<OperationLock> {
    let owner = records::owner(
        if id.is_some() {
            "Controller review"
        } else {
            "Source submission"
        },
        if id.is_some() {
            "Review and build a submitted configuration"
        } else {
            "Validate and import proposed machine configuration"
        },
        if id.is_some() {
            OperationKind::Review
        } else {
            OperationKind::SourceImport
        },
    )?;
    Ok(store.try_operation(
        &operation_key()?,
        if let Some(id) = id {
            owner.with_job(id.clone())
        } else {
            owner
        },
    )?)
}
fn user_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    let uid = rustix::process::getuid().as_raw();
    let runtime = format!("/run/user/{uid}");
    cmd.env("XDG_RUNTIME_DIR", &runtime).env(
        "DBUS_SESSION_BUS_ADDRESS",
        format!("unix:path={runtime}/bus"),
    );
    cmd
}
fn worker_alive(id: &JobId) -> Result<bool> {
    let result = process::run(
        user_command("systemctl").args([
            "--user",
            "is-active",
            "--quiet",
            &format!("airlock-review-{id}.service"),
        ]),
        None,
        Some(Duration::from_secs(5)),
    )?;
    match result.status.code() {
        Some(0) => Ok(true),
        Some(3 | 4) => Ok(false),
        _ => Err(Error::Message(
            "could not determine controller worker liveness".into(),
        )),
    }
}
fn ensure_idle() -> Result<()> {
    for name in records::names("job-")? {
        let id: JobId = name.trim_start_matches("job-").parse()?;
        let job = records::job(&id)?;
        if job.status().is_active() {
            let age = now()
                - job
                    .detail("updated_at")
                    .or_else(|| job.detail("created_at"))
                    .and_then(Value::as_f64)
                    .unwrap_or(now());
            require(
                age > 10.0 && !worker_alive(&id)?,
                &format!(
                    "controller request {id} is {}; finish it before submitting another",
                    job.status().as_str()
                ),
            )?;
        }
    }
    for (id, value) in records::legacy()? {
        let phase: JobStatus = value["status"]
            .as_str()
            .expect("validated status")
            .to_owned()
            .try_into()?;
        if phase.is_active() {
            require(
                !worker_alive(&id)?,
                &format!("legacy controller request {id} is still active"),
            )?;
        }
    }
    Ok(())
}
fn serve() -> Result<u8> {
    let caller = std::env::var("QREXEC_REMOTE_DOMAIN").unwrap_or_default();
    require(
        caller == "usr-assistant" || caller == "sys-agent",
        "only usr-assistant may submit candidates",
    )?;
    let store = records::initialize()?;
    let lock = acquire(&store, None)?;
    ensure_idle()?;
    let mut upload = protocol::Upload::new();
    let header: Header = upload.header()?;
    header.candidate()?;
    require(
        caller == "usr-assistant" || header.rename(),
        "legacy caller requires explicit assistant rename",
    )?;
    let mut archive = upload.archive(header.archive_bytes)?;
    lock.check_path()?;
    let candidate = format!(
        "source-{}{}",
        header.revision,
        if header.rename() { "-rename" } else { "" }
    );
    let archive_path = records::root()?.join(format!("{candidate}.tar.gz"));
    let mut saved = tempfile::NamedTempFile::new_in(records::root()?)?;
    std::io::copy(&mut archive, &mut saved)?;
    saved.as_file().sync_all()?;
    saved.persist(&archive_path).map_err(|e| e.error)?;
    File::open(records::root()?)?.sync_all()?;
    records::save(&store, &candidate, &header)?;
    archive.rewind()?;
    let mut child = process::OwnedChild(
        Command::new("airlock-usr-assistant-machine-source")
            .args([
                "import",
                "--revision",
                header.revision.as_str(),
                "--lock-hash",
                header.lock_hash.as_str(),
            ])
            .stdin(Stdio::from(archive))
            .stdout(Stdio::null())
            .spawn()?,
    );
    require(
        child.0.wait()?.success(),
        "source importer failed; no build was started",
    )?;
    require(
        protocol::identity(&repo())? == header.source(),
        "controller checkout differs from submitted source",
    )?;
    lock.check_path()?;
    let mut random = [0; 16];
    use std::io::Read;
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    let id: JobId = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        .parse()?;
    let mut job = JobSnapshot::queued(id.clone());
    job.set_detail("request", serde_json::to_value(&header)?);
    job.set_detail("created_at", json!(now()));
    records::save_job(&store, &mut job)?;
    let exe = std::env::current_exe()?;
    let result = process::run(
        user_command("systemd-run")
            .args([
                "--user",
                "--collect",
                &format!("--unit=airlock-review-{id}"),
                "--property=Type=exec",
            ])
            .arg(format!(
                "--setenv=PATH={}",
                std::env::var("PATH").unwrap_or_default()
            ))
            .arg(exe)
            .args(["work", id.as_str()]),
        None,
        Some(Duration::from_secs(10)),
    );
    if let Err(error) = result.and_then(|r| r.checked("start controller worker")) {
        job.stop(StopReason::Failed)?;
        job.set_detail("error", json!(error.to_string()));
        records::save_job(&store, &mut job)?;
    }
    drop(lock);
    wait(&id)
}
fn wait(id: &JobId) -> Result<u8> {
    let mut previous = None;
    loop {
        let job = records::job(id)?;
        let feedback = job.submission_feedback();
        let mut value = serde_json::to_value(&job)?;
        if job.decision().is_some() {
            value["job_status"] = json!(job.status());
            value["status"] = json!("approved");
            value["continuing"] = json!(job.status().is_active());
            value["status_command"] =
                json!(format!("airlock machine review-status --request-id {id}"));
        } else if job.status().is_active()
            && now()
                - job
                    .detail("updated_at")
                    .and_then(Value::as_f64)
                    .unwrap_or(now())
                > 10.0
            && !worker_alive(id)?
        {
            // Do not rewrite the interrupted worker's durable record.
            let latest = records::job(id)?;
            if serde_json::to_value(latest)? != serde_json::to_value(&job)? {
                continue;
            }
            value["status"] = json!(if job.status() == JobStatus::AwaitingApproval {
                "cancelled"
            } else {
                "failed"
            });
            value["error"] =
                json!("Controller job was interrupted; reconnecting grants no approval");
            emit(&value)?;
            return Ok(if job.status() == JobStatus::AwaitingApproval {
                21
            } else {
                1
            });
        }
        if previous.as_ref() != value.get("status") {
            emit(&value)?;
            previous = value.get("status").cloned();
        }
        if let Some(code) = feedback.exit_code() {
            return Ok(code);
        }
        thread::sleep(Duration::from_millis(400));
    }
}
fn control(request: &Value) -> Result<Value> {
    let value = process::rpc(
        "airlock.Ctrl.ReviewControl",
        request,
        Some(Duration::from_secs(10)),
    )?;
    require(
        value.get("status").and_then(Value::as_str).is_some(),
        "ReviewControl response has no status",
    )?;
    Ok(value)
}
#[derive(Clone, Debug, Deserialize)]
struct Context {
    request_id: ReviewId,
    baseline: GitRevision,
    baseline_state: BaselineFingerprint,
    binding: Binding,
    scope: Scope,
    status: String,
    review_fingerprint: Option<ReviewFingerprint>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    revision: GitRevision,
    lock_hash: LockHash,
    family: String,
    target: Option<GuestName>,
}
impl Context {
    fn request(&self) -> Result<ReviewRequest> {
        decode(&serde_json::to_vec(
            &json!({"schema":"airlock.ctrl-approval-request.v1","request_id":self.request_id,"baseline":self.baseline,"revision":self.binding.revision,"lock_hash":self.binding.lock_hash,"family":self.binding.family,"target":self.binding.target,"scope":self.scope}),
        )?)
    }
    fn observation(&self, header: &Header) -> Result<ReviewObservation> {
        let request = self.request()?;
        require(
            request.candidate() == &header.candidate()?,
            "protected review context differs from submitted source or target",
        )?;
        Ok(ReviewObservation {
            baseline: self.baseline.clone(),
            baseline_state: self.baseline_state.clone(),
            candidate: header.candidate()?,
        })
    }
}
fn context(header: &Header) -> Result<Context> {
    let raw = control(
        &json!({"op":"context","revision":header.revision,"lock_hash":header.lock_hash,"family":if header.action.is_machine(){"machine"}else{"guest"},"target":header.target}),
    )?;
    decode(&serde_json::to_vec(&raw)?)
}
// A grant can only be produced after authenticating the GUI decision through
// dom0's protected ReviewControl service. Builders require this private type.
pub(super) struct Grant {
    pub id: ReviewId,
    pub header: Header,
    pub approval: Box<ApprovedChange>,
}
impl Grant {
    pub fn check(&self, lock: &OperationLock) -> Result<()> {
        lock.check_path()?;
        require(
            protocol::identity(&repo())? == self.header.source(),
            "controller source changed after review",
        )?;
        let raw = control(
            &json!({"op":"check","request_id":self.id,"revision":self.header.revision,"lock_hash":self.header.lock_hash}),
        )?;
        let checked: Context = decode(&serde_json::to_vec(&raw)?)?;
        require(
            checked.request_id == self.id
                && checked.status == "approved"
                && checked.scope == *self.approval.scope(),
            "protected approval is no longer valid",
        )?;
        self.approval.recheck(&checked.observation(&self.header)?)?;
        Ok(())
    }
}
fn authorize(
    store: &StateStore,
    job: &mut JobSnapshot,
    header: &Header,
    lock: &mut OperationLock,
) -> Result<Grant> {
    let ctx = context(header)?;
    let observed = ctx.observation(header)?;
    let request = ctx.request()?;
    let pending = PendingReview::new(request.clone(), &observed)?;
    job.awaiting(ctx.request_id.clone())?;
    records::save_job(store, job)?;
    lock.update_phase("awaiting_approval".into())?;
    let (reply, latest) = if ctx.status == "awaiting_approval" {
        let name = format!("review-{}", ctx.request_id);
        records::save(store, &name, &request)?;
        let app = std::env::var("AIRLOCK_CTRL_BIN")
            .unwrap_or_else(|_| "/run/current-system/sw/bin/airlock-ctrl".into());
        let mut command = user_command(&app);
        let exported = process::run(
            user_command("systemctl").args(["--user", "show-environment"]),
            None,
            Some(Duration::from_secs(5)),
        )?
        .checked("read graphical environment")?;
        let mut display = false;
        for line in String::from_utf8_lossy(&exported).lines() {
            if let Some((key, value)) = line.split_once('=')
                && [
                    "DISPLAY",
                    "WAYLAND_DISPLAY",
                    "XAUTHORITY",
                    "XCURSOR_SIZE",
                    "GDK_SCALE",
                    "GDK_DPI_SCALE",
                ]
                .contains(&key)
            {
                command.env(key, value);
                if ["DISPLAY", "WAYLAND_DISPLAY"].contains(&key) && !value.is_empty() {
                    display = true;
                }
            }
        }
        require(
            display,
            "controller graphical session is unavailable; no build was started",
        )?;
        command
            .arg("--repo")
            .arg(repo())
            .args([
                "--base",
                ctx.baseline.as_str(),
                "--candidate",
                header.revision.as_str(),
                "--approval-request",
            ])
            .arg(records::record_path(&name)?);
        let reply: ReviewResponse =
            decode(&process::run(&mut command, None, None)?.checked("source review window")?)?;
        // Validate before sending anything to the protected decision service.
        let _ = pending.decide(reply.clone(), &observed)?;
        let raw = control(
            &json!({"op":"decide","request_id":ctx.request_id,"decision":reply.decision,"scope":reply.scope,"review_fingerprint":reply.review_fingerprint}),
        )?;
        let latest: Context = decode(&serde_json::to_vec(&raw)?)?;
        (reply, latest)
    } else {
        if ctx.status != "approved" {
            return Err(stopped(&ctx.status));
        }
        let reply = ReviewResponse {
            schema: crate::review::ResponseSchema::V1,
            request_id: ctx.request_id.clone(),
            revision: header.revision.clone(),
            lock_hash: header.lock_hash.clone(),
            review_fingerprint: ctx
                .review_fingerprint
                .clone()
                .ok_or_else(|| Error::Message("protected approval has no fingerprint".into()))?,
            decision: Decision::Approve,
            scope: ctx.scope.clone(),
        };
        (reply, ctx.clone())
    };
    if latest.status != "approved" {
        return Err(stopped(&latest.status));
    }
    require(
        latest.request_id == ctx.request_id,
        "protected decision identity changed",
    )?;
    let current = latest.observation(header)?;
    let pending = PendingReview::new(request, &observed)?;
    let ReviewOutcome::Approved(approved) = pending.decide(reply, &current)? else {
        return Err(Error::Message("no approval was granted".into()));
    };
    let grant = Grant {
        id: ctx.request_id,
        header: header.clone(),
        approval: approved,
    };
    Ok(grant)
}
fn stopped(status: &str) -> Error {
    Error::Stopped(match status {
        "rejected" => StopReason::Rejected,
        "cancelled" => StopReason::Cancelled,
        "scope_exceeded" => StopReason::ScopeExceeded,
        "stale_review" => StopReason::StaleReview,
        _ => StopReason::Failed,
    })
}
fn work(id: JobId) -> Result<u8> {
    let store = records::initialize()?;
    let start = Instant::now();
    let mut lock = loop {
        match acquire(&store, Some(&id)) {
            Ok(lock) => break lock,
            Err(Error::Core(crate::Error::Busy { .. }))
                if start.elapsed() < Duration::from_secs(30) =>
            {
                thread::sleep(Duration::from_millis(100))
            }
            Err(e) => return Err(e),
        }
    };
    let mut job = records::job(&id)?;
    require(
        job.status() == JobStatus::Queued,
        "controller job has already been started",
    )?;
    let header: Header = decode(&serde_json::to_vec(
        job.detail("request")
            .ok_or_else(|| Error::Message("job request missing".into()))?,
    )?)?;
    header.candidate()?;
    let result = (|| -> Result<Value> {
        require(
            protocol::identity(&repo())? == header.source(),
            "controller source changed before review",
        )?;
        let grant = authorize(&store, &mut job, &header, &mut lock)?;
        job.approved(&grant.id)?;
        records::save_job(&store, &mut job)?;
        lock.update_phase("building".into())?;
        grant.check(&lock)?;
        build::execute(&store, &grant, &lock)
    })();
    match result {
        Ok(payload) => {
            let phase = if payload.get("unit").is_some() {
                ApprovedStatus::DeploymentStarted
            } else if header.action.is_plan() {
                ApprovedStatus::Planned
            } else {
                ApprovedStatus::Built
            };
            job.finish(phase)?;
            job.set_detail("result", payload);
            records::save_job(&store, &mut job)?;
            lock.update_phase(phase.as_str().into())?;
            Ok(0)
        }
        Err(error) => {
            let reason = match &error {
                Error::Stopped(reason) => *reason,
                Error::Core(crate::Error::StaleReview) => StopReason::StaleReview,
                _ => {
                    let detail = error.to_string();
                    if detail.contains("scope_exceeded") {
                        StopReason::ScopeExceeded
                    } else if detail.contains("stale_review") {
                        StopReason::StaleReview
                    } else {
                        StopReason::Failed
                    }
                }
            };
            job.stop(reason)?;
            job.set_detail("error", json!(error.to_string()));
            if let Error::Remote { details, .. } = &error {
                job.set_detail("control_error", (**details).clone());
            }
            records::save_job(&store, &mut job)?;
            Err(error)
        }
    }
}
fn status(id: &JobId, refresh: bool) -> Result<Value> {
    let path = records::record_path(&records::job_key(id))?;
    if !path.try_exists()? {
        return records::legacy_job(id);
    }
    let job = records::job(id)?;
    let mut value = serde_json::to_value(&job)?;
    let store = StateStore::open(&records::root()?)?;
    value["controller_lock"] = serde_json::to_value(store.operation_status(&operation_key()?)?)?;
    if refresh
        && matches!(
            job.status(),
            JobStatus::Building | JobStatus::DeploymentStarted
        )
        && let Some(auth) = job.authorization_id()
    {
        match control(&json!({"op":"status","request_id":auth})) {
            Ok(latest) => {
                value["lock"] = latest.get("lock").cloned().unwrap_or(Value::Null);
                value["deployment_observation"] = latest;
            }
            Err(error) => {
                value["observation_error"] = json!(error.to_string());
            }
        }
    }
    if job.status().is_active()
        && now()
            - job
                .detail("updated_at")
                .and_then(Value::as_f64)
                .unwrap_or(now())
            > 10.0
    {
        match worker_alive(id) {
            Ok(false) => {
                let current = records::job(id)?;
                if serde_json::to_value(current)? != serde_json::to_value(&job)? {
                    return status(id, false);
                }
                value["status"] = json!(if job.decision().is_some() {
                    "failed"
                } else if job.status() == JobStatus::AwaitingApproval {
                    "cancelled"
                } else {
                    "failed"
                });
                value["error"] =
                    json!("Controller worker was interrupted; no approval granted by observation");
            }
            Err(error) => value["observation_error"] = json!(error.to_string()),
            Ok(true) => (),
        }
    }
    Ok(value)
}
fn observe() -> Result<u8> {
    let status = process::machine_status()?;
    let store = records::initialize()?;
    records::save(&store, "observed", &status)?;
    if let Some(revision) = status.get("revision").and_then(Value::as_str) {
        let revision: GitRevision = revision.parse()?;
        records::save(&store, &format!("outcome-{revision}"), &status)?;
    }
    if status.get("phase") == Some(&json!("complete"))
        && status.get("host_matches_selection") == Some(&json!(true))
    {
        records::save(&store, "current", &status)?;
    }
    Ok(0)
}
