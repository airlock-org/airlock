//! Role-specific runtime adapters. Dom0 retains approval and activation authority.
mod build;
mod client;
mod controller;
mod process;
mod protocol;
mod records;

pub use client::main as client_main;
pub use controller::main as controller_main;

use crate::{Error as CoreError, job::StopReason};
use serde_json::{Value, json};
use std::{io, path::Path, process::ExitCode};

type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
enum Error {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("could not start {program}: {source}")]
    Command {
        program: String,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Message(String),
    #[error("request stopped: {}", .0.as_str())]
    Stopped(StopReason),
    #[error("{operation} failed: {message}")]
    Remote {
        operation: String,
        message: String,
        details: Box<Value>,
    },
}
fn require(condition: bool, message: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Message(message.to_owned()))
    }
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    Ok(crate::state::parse_json(bytes, Path::new("<protocol>"))?)
}
fn emit(value: &impl serde::Serialize) -> Result<()> {
    use io::Write;
    let mut out = io::stdout().lock();
    serde_json::to_writer(&mut out, value)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}
fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
fn report(result: Result<u8>) -> ExitCode {
    match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            let code = match &error {
                Error::Core(CoreError::Busy { .. }) => 24,
                Error::Stopped(reason) => reason.exit_code(),
                _ => 1,
            };
            let details = match &error {
                Error::Core(e) => serde_json::to_value(e.report()).unwrap_or(Value::Null),
                Error::Remote { details, .. } => (**details).clone(),
                Error::Command { program, source } => {
                    json!({"program":program,"os_error":source.raw_os_error()})
                }
                Error::Io(source) => {
                    json!({"os_error":source.raw_os_error(),"kind":source.kind().to_string()})
                }
                _ => Value::Null,
            };
            let _ = emit(
                &json!({"schema":"airlock.nix-runtime-error.v1", "status": if code==24 {"busy"} else {"failed"}, "error":error.to_string(), "details":details}),
            );
            eprintln!("airlock-nix: {error}");
            ExitCode::from(code)
        }
    }
}
