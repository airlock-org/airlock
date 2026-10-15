use airlock_nix::{
    Error, Result,
    identity::JobId,
    job::JobSnapshot,
    review::{DeploymentPlan, ReviewRequest, ReviewResponse},
    state::read_json,
};
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::json;
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

#[derive(Parser)]
#[command(
    version,
    about = "Inspect Airlock Nix records locally. No build, approval grant, or deployment is performed."
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Inspect a controller snapshot; preserve approval separately from work status.
    #[command(name = "inspect-job")]
    Job {
        file: PathBuf,
        /// Check that this is the requested job, rather than a different record.
        #[arg(long)]
        request_id: Option<JobId>,
    },
    /// Validate the existing GUI request schema, source identities and scope.
    #[command(name = "inspect-review")]
    Review { file: PathBuf },
    /// Validate a GUI response as data. This grants no approval authority.
    #[command(name = "inspect-response")]
    Response { file: PathBuf },
    /// Inspect a bare deployment plan and its complete known VM impact.
    #[command(name = "inspect-plan")]
    Plan { file: PathBuf },
}

fn inspect(command: Command) -> Result<serde_json::Value> {
    match command {
        Command::Job { file, request_id } => {
            let job: JobSnapshot = read_json(&file, 4 * 1024 * 1024)?;
            if request_id
                .as_ref()
                .is_some_and(|expected| expected != job.request_id())
            {
                return Err(Error::BindingMismatch {
                    field: "controller job ID",
                });
            }
            let feedback = job.submission_feedback();
            Ok(
                json!({"kind": "job", "record": job, "submission_feedback": feedback,
                "submission_exit_code": feedback.exit_code()}),
            )
        }
        Command::Review { file } => {
            let record: ReviewRequest = read_json(&file, 65536)?;
            Ok(json!({"kind": "review_request", "record": record}))
        }
        Command::Response { file } => {
            let record: ReviewResponse = read_json(&file, 65536)?;
            Ok(json!({"kind": "review_response", "record": record}))
        }
        Command::Plan { file } => {
            let record: DeploymentPlan = read_json(&file, 4 * 1024 * 1024)?;
            Ok(json!({"kind": "plan", "record": record,
                "affected_vms": record.affected_vms(), "reboot": record.reboot_requirement()}))
        }
    }
}

fn print_json(value: &impl Serialize) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value)?;
    stdout.write_all(b"\n")
}

fn respond(output: impl Serialize, code: ExitCode) -> ExitCode {
    if let Err(error) = print_json(&output) {
        if error.kind() == io::ErrorKind::BrokenPipe {
            return code;
        }
        eprintln!("airlock-nix: could not write response: {error}");
        return ExitCode::FAILURE;
    }
    code
}

fn main() -> ExitCode {
    match inspect(Args::parse().command) {
        Ok(value) => respond(
            json!({"schema": "airlock.nix-inspection.v1", "authority": "local-inspection-only", "inspection": value}),
            ExitCode::SUCCESS,
        ),
        Err(error) => respond(error.report(), ExitCode::FAILURE),
    }
}
