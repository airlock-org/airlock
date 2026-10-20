//! Inert qrexec endpoint records submitted source and returns only explicitly supplied controller outcomes.

use anyhow::{Context, Result, ensure};
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, Read, Write},
    path::Path,
    time::Duration,
};

/// Read the native CLI's real framed request and archive without executing any submitted authoring code.
pub fn execute(capture: &Path, arguments: &[String]) -> Result<()> {
    let mut input = std::io::stdin().lock();
    let mut line = String::new();
    input.read_line(&mut line)?;
    let request: Value = serde_json::from_str(&line)?;
    let mut record = json!({"args": arguments, "request": request});
    if arguments.last().map(String::as_str) == Some("airlock.Ctrl.GuestDeploy") {
        let mut payload = Vec::new();
        input.read_to_end(&mut payload)?;
        ensure!(
            Some(payload.len() as u64) == request["archive_bytes"].as_u64(),
            "submission archive size differs"
        );
        let mut names = Vec::new();
        for entry in tar::Archive::new(GzDecoder::new(payload.as_slice())).entries()? {
            let mut entry = entry?;
            let name = entry.path()?.to_str().context("archive path")?.to_owned();
            if name == "manifest.json" {
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes)?;
                record["manifest"] = serde_json::from_slice(&bytes)?;
            }
            names.push(name);
        }
        names.sort();
        record["members"] = json!(names);
    }
    fs::write(capture, serde_json::to_vec(&record)?)?;
    print!(
        "{reply}",
        reply = std::env::var("AIRLOCK_TEST_SUBMISSION_REPLY")
            .unwrap_or_else(|_| "{\"status\":\"passed\"}\n".into())
    );
    eprint!(
        "{error}",
        error = std::env::var("AIRLOCK_TEST_SUBMISSION_ERROR").unwrap_or_default()
    );
    std::io::stdout().flush()?;
    std::io::stderr().flush()?;
    if let Ok(delay) = std::env::var("AIRLOCK_TEST_SUBMISSION_DELAY") {
        std::thread::sleep(Duration::from_millis(delay.parse()?));
    }
    std::process::exit(
        std::env::var("AIRLOCK_TEST_SUBMISSION_EXIT")
            .unwrap_or_else(|_| "0".into())
            .parse()?,
    );
}
