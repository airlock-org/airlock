//! Inert coordinator and Nix-import peers model transfer outcomes inside a disposable user namespace.

use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::symlink,
    path::Path,
};

/// Reply only to the fixed commands used by the native RPC receiver and record their exact requests.
pub(super) fn execute(root: &Path, name: &str, args: &[String]) -> Result<()> {
    if name == "airlock-test-peer" && args.first().is_some_and(|arg| arg == "rpc-race") {
        return race(&args[1], serde_json::from_str(&args[2])?);
    }
    let mut config: Value = serde_json::from_slice(&fs::read(root.join("config.json"))?)?;
    let (args, request) = if matches!(name, "engine" | "airlock-dom0-service") {
        ensure!(
            args.starts_with(&["--state".into(), "/var/lib/airlock/machine".into()]),
            "unexpected coordinator state"
        );
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes)?;
        (
            &args[2..],
            if bytes.is_empty() {
                Value::Null
            } else {
                serde_json::from_slice(&bytes)?
            },
        )
    } else {
        (args, Value::Null)
    };
    let action = if matches!(name, "engine" | "airlock-dom0-service") {
        args.first().context("coordinator action")?.as_str()
    } else {
        name
    };
    let mut calls = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("calls.jsonl"))?;
    writeln!(
        calls,
        "{}",
        json!({"program":name,"args":args,"request":request})
    )?;
    if config["fail"] == action {
        println!(
            "{}",
            config
                .get("failure")
                .cloned()
                .unwrap_or_else(|| json!({"error":"specific admission failure"}))
        );
        eprintln!("specific admission failure");
        std::process::exit(1);
    }
    match name {
        "nix" => {
            for path in config["valid"]
                .as_array()
                .context("valid store inventory")?
            {
                println!("{path}", path = path.as_str().context("store identity")?);
            }
        }
        "nix-store" if args.iter().any(|arg| arg == "--import") => {
            let mut data = Vec::new();
            std::io::stdin().read_to_end(&mut data)?;
            fs::write(root.join("imported"), data)?;
            if config["revoke_after_import"] == true {
                config["fail"] = json!("review-control");
                fs::write(root.join("config.json"), serde_json::to_vec(&config)?)?;
            }
            if config["drop_review_marker"] == true {
                fs::remove_file(root.join("etc/review-required"))?;
            }
            let imported = config.get("imported_paths").cloned().unwrap_or_else(|| {
                json!([format!(
                    "/nix/store/{hash}-rpc-release",
                    hash = "a".repeat(32)
                )])
            });
            for path in imported.as_array().context("import result")? {
                println!("{path}", path = path.as_str().context("imported path")?);
            }
        }
        "nix-store" => {
            let index = args
                .iter()
                .position(|arg| arg == "--add-root")
                .context("root option")?;
            let destination = Path::new(args.get(index + 1).context("root path")?);
            ensure!(
                destination.starts_with(root),
                "peer cannot create roots outside its fixture"
            );
            symlink(args.last().context("root target")?, destination)?;
        }
        "systemd-run" => println!("Running as unit: detached.service"),
        "engine" | "airlock-dom0-service" => {
            let candidate = config
                .get("candidate")
                .cloned()
                .unwrap_or_else(|| json!("/var/lib/airlock/machine/candidates/abc"));
            let result = match action {
                "guest-plan" | "machine-bind" => {
                    json!({"candidate":candidate,"plan":config["plan"]})
                }
                "prepared-check" => json!({"plan":config["plan"],"state":config["state"]}),
                "review-control" => json!({"binding":{"family":config["family"]}}),
                "plan" => config["plan"].clone(),
                _ => json!({"status":"ready"}),
            };
            println!("{result}");
        }
        other => anyhow::bail!("unexpected RPC fixture command: {other}"),
    }
    Ok(())
}

/// Hold one transfer open while checking authenticated lock refusal and retry in the same namespace.
fn race(native: &str, header: Value) -> Result<()> {
    use airlock_tests::dialogue::Dialogue;
    use airlock_vm_service::support;
    use flate2::{Compression, write::GzEncoder};
    use std::{process::Command, time::Duration};

    let command = || {
        let mut command = Command::new(native);
        command.args([
            "rpc",
            "--engine",
            "/var/lib/airlock/bin/engine",
            "machine-receive",
            "prepare",
        ]);
        command
    };
    let mut line = serde_json::to_vec(&header)?;
    line.push(b'\n');
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(b"retry NAR")?;
    let mut wire = line.clone();
    wire.extend(gzip.finish()?);
    let mut first = Dialogue::spawn(&mut command())?;
    first.send(&line)?;
    ensure!(
        first.reply(Duration::from_secs(5))?["missing"] == json!([header["release"]]),
        "first transfer did not negotiate inventory"
    );
    let other = support::run(
        &mut command(),
        Some(support::Input::bytes(wire.clone())),
        Duration::from_secs(5),
    )?;
    let blocked: Value = serde_json::from_slice(&other.stdout)?;
    ensure!(
        !other.status.success()
            && blocked["status"] == "busy"
            && blocked["lock"]["verified"] == true,
        "parallel transfer did not report authenticated ownership: {blocked}"
    );
    ensure!(
        blocked["lock"]["owner"] == "Machine transfer from sys-ctrl"
            && blocked["lock"]["reason"].is_string(),
        "lock identity was lost"
    );
    ensure!(
        !first.finish()?.success(),
        "incomplete first transfer succeeded"
    );
    let retry = support::run(
        &mut command(),
        Some(support::Input::bytes(wire)),
        Duration::from_secs(5),
    )?
    .checked()?;
    let reply: Value = serde_json::from_slice(
        retry
            .split(|byte| *byte == b'\n')
            .rfind(|line| !line.is_empty())
            .context("retry reply")?,
    )?;
    ensure!(
        reply["paths_transferred"] == 1,
        "retry did not transfer its negotiated inventory"
    );
    println!("{blocked}");
    Ok(())
}
