use super::{
    Result, decode, emit,
    process::{self, OwnedChild},
    protocol::{self, Action, Header, RequestSchema},
    report,
};
use crate::identity::{GuestName, JobId};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::{
    io::{self, Write},
    path::PathBuf,
    process::{Command, ExitCode, Stdio},
    time::Duration,
};
#[derive(Parser)]
#[command(
    version,
    about = "Submit committed configuration to sys-ctrl and wait for its review decision"
)]
struct Args {
    #[command(subcommand)]
    command: ClientCommand,
}
#[derive(Subcommand)]
enum ClientCommand {
    Plan {
        target: GuestName,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    Apply {
        target: GuestName,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    MachineBuild {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        assistant_rename: bool,
    },
    MachinePlan {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        assistant_rename: bool,
    },
    MachineApply {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        assistant_rename: bool,
    },
    ReviewStatus {
        request_id: JobId,
    },
}
pub fn main_from(args: impl IntoIterator<Item = std::ffi::OsString>) -> ExitCode {
    report(run(Args::parse_from(args).command))
}
fn run(command: ClientCommand) -> Result<u8> {
    let (action, target, repo, rename) = match command {
        ClientCommand::Plan { target, repo } => (Action::GuestPlan, Some(target), repo, false),
        ClientCommand::Apply { target, repo } => (Action::GuestApply, Some(target), repo, false),
        ClientCommand::MachineBuild {
            repo,
            assistant_rename,
        } => (Action::MachineBuild, None, repo, assistant_rename),
        ClientCommand::MachinePlan {
            repo,
            assistant_rename,
        } => (Action::MachinePlan, None, repo, assistant_rename),
        ClientCommand::MachineApply {
            repo,
            assistant_rename,
        } => (Action::MachineApply, None, repo, assistant_rename),
        ClientCommand::ReviewStatus { request_id } => {
            let mut request = serde_json::to_vec(&json!({"request_id":request_id}))?;
            request.push(b'\n');
            let result = process::run(
                Command::new("qrexec-client-vm").args(["sys-ctrl", "airlock.Ctrl.ReviewStatus"]),
                Some(request),
                Some(Duration::from_secs(30)),
            )?;
            let code = result.status.code().unwrap_or(1) as u8;
            let value: serde_json::Value = decode(&result.stdout)?;
            emit(&value)?;
            return Ok(code);
        }
    };
    let repo = repo.unwrap_or(super::home()?.join("airlock-machine"));
    let temp = tempfile::Builder::new()
        .prefix("airlock-nix-submit-")
        .tempdir()?;
    let (source, mut archive) = protocol::source_archive(&repo, temp.path())?;
    let header = Header {
        schema: if rename {
            RequestSchema::Rename
        } else {
            RequestSchema::Ordinary
        },
        action,
        target,
        revision: source.revision,
        lock_hash: source.lock_hash,
        archive_bytes: archive.metadata()?.len(),
    };
    header.candidate()?;
    let mut child = OwnedChild(
        Command::new("qrexec-client-vm")
            .args(["sys-ctrl", "airlock.Ctrl.GuestDeploy"])
            .stdin(Stdio::piped())
            .spawn()?,
    );
    let mut stdin = child.0.stdin.take().expect("piped stdin");
    let sent = (|| -> Result<()> {
        serde_json::to_writer(&mut stdin, &header)?;
        stdin.write_all(b"\n")?;
        io::copy(&mut archive, &mut stdin)?;
        Ok(())
    })();
    drop(stdin);
    let status = child.0.wait()?;
    // Preserve an early rejection's feedback when it closes a large upload.
    if status.success() {
        sent?;
    }
    Ok(status.code().unwrap_or(1) as u8)
}
