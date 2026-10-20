//! Pass evaluated machine snapshots to the existing dom0 service's offline planner.
//! The caller supplies running VM names and the baseline host; no live state is queried.

use airlock_machine_core::inspection::{EvaluatedMachine, Inspection};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Writes evaluated manifest and topology data in the planner's snapshot layout.
/// Returns the recorded host path for explicit baseline comparison; malformed input
/// or filesystem failures are errors. Writing a snapshot does not build or admit a release.
fn snapshot(input: &Path, directory: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let evaluated: Value = serde_json::from_slice(&std::fs::read(input)?)?;
    let manifest = evaluated
        .get("manifest")
        .ok_or("missing evaluated manifest")?;
    let topology = evaluated
        .get("topology")
        .ok_or("missing evaluated topology")?;
    let host = manifest
        .get("host")
        .and_then(Value::as_str)
        .ok_or("missing evaluated host path")?;

    std::fs::create_dir_all(directory)?;
    std::fs::write(
        directory.join("release.json"),
        serde_json::to_vec_pretty(manifest)?,
    )?;
    std::fs::write(
        directory.join("topology.json"),
        serde_json::to_vec_pretty(topology)?,
    )?;

    Ok(host.to_owned())
}

/// Runs the existing service's offline planner against two evaluated snapshots.
/// Uses only the supplied baseline, host path and running-VM list; records the
/// command and both output streams below `output`. A previous plan is removed
/// before inspection, and a new one is saved only for a successful response with
/// `local-inspection-only` authority. Planner rejection is returned as an error.
pub fn plan(
    planner: &Path,
    before: &Path,
    after: &Path,
    output: &Path,
    running: &[String],
) -> Result<Value, Box<dyn std::error::Error>> {
    let planner = planner.canonicalize()?;
    std::fs::create_dir_all(output)?;
    let output = output.canonicalize()?;

    // A failed repeat inspection must not leave a previous successful plan visible.
    match std::fs::remove_file(output.join("plan.json")) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let expected = serde_json::to_value(inspect_files(before, after, running)?)?;
    let baseline = output.join("baseline");
    let candidate = output.join("candidate");
    let host = snapshot(before, &baseline)?;
    snapshot(after, &candidate)?;

    // --baseline selects the existing local-inspection branch. --host avoids
    // even reading /run/current-system. Coordinator construction creates no state.
    let mut arguments = vec![
        PathBuf::from("--state"),
        output.join("unused-state"),
        PathBuf::from("plan"),
        candidate,
        PathBuf::from("--baseline"),
        baseline,
        PathBuf::from("--host"),
        PathBuf::from(host),
    ];
    for name in running {
        arguments.extend([PathBuf::from("--running"), PathBuf::from(name)]);
    }

    std::fs::write(
        output.join("command.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "program": planner, "arguments": arguments,
        }))?,
    )?;

    let result = Command::new(planner).args(arguments).output()?;
    std::fs::write(output.join("planner.stderr"), &result.stderr)?;
    std::fs::write(output.join("planner.stdout"), &result.stdout)?;
    if !result.status.success() {
        // The service emits protocol errors as JSON on stdout; CLI/loader errors
        // can instead arrive on stderr. Keep both original streams on disk.
        let message = serde_json::from_slice::<Value>(&result.stdout)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_else(|| String::from_utf8_lossy(&result.stderr).into_owned());
        return Err(format!(
            "existing Airlock planner rejected inspection ({status}): {message}",
            status = result.status,
        )
        .into());
    }

    let value: Value = serde_json::from_slice(&result.stdout)?;
    if value.get("authority").and_then(Value::as_str) != Some("local-inspection-only") {
        return Err("planner did not return local-inspection-only authority".into());
    }

    if value != expected {
        return Err("service inspection differs from the shared planning library".into());
    }

    std::fs::write(output.join("plan.json"), serde_json::to_vec_pretty(&value)?)?;

    Ok(value)
}

/// Decode and validate evaluated configuration before passing it to the common planner.
fn evaluated(path: &Path) -> Result<EvaluatedMachine, Box<dyn std::error::Error>> {
    let machine: EvaluatedMachine = serde_json::from_slice(&std::fs::read(path)?)?;
    machine.view().validate()?;

    Ok(machine)
}

/// Inspect evaluated releases directly through the library also used by dom0.
/// Running VM names and the baseline host are explicit inputs; no live state is read.
pub fn inspect_files(
    before: &Path,
    after: &Path,
    running: &[String],
) -> Result<Inspection, Box<dyn std::error::Error>> {
    let before = evaluated(before)?;
    let after = evaluated(after)?;
    let running = running
        .iter()
        .map(|name| name.parse())
        .collect::<Result<_, _>>()?;

    Ok(airlock_machine_core::inspection::inspect(
        before.view(),
        after.view(),
        &running,
        &before.manifest.host,
    )?)
}
