//! Local development transport peers run real shell framing and record finite Nix or SSH operations.

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::{fs::symlink, process::CommandExt},
    path::Path,
    process::Command,
};

/// Record one child-tool invocation before returning its finite response or executing a local fixture shell.
pub(crate) fn execute(root: &Path, name: &str, args: &[String]) -> Result<()> {
    let config: Value = serde_json::from_slice(&fs::read(root.join("peer.json"))?)?;
    writeln!(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join(format!("{name}.jsonl")))?,
        "{}",
        json!(args)
    )?;
    let bash = config["bash"].as_str().context("fixture shell")?;
    match name {
        "cargo" => {}
        "ssh" if config["shell"].as_bool() == Some(true) => {
            let mut command = Command::new(bash);
            if args == ["-tt", "--", "fixture", "airlock-dom0-shell"] {
                println!("DEVELOPMENT: dom0 shell.");
                std::io::stdout().flush()?;
                command.args(["--noprofile", "--norc"]);
            } else {
                command.arg("-c").arg(args.last().context("SSH command")?);
            }
            return Err(command.exec().into());
        }
        "ssh" => {
            if args.last().is_some_and(|arg| arg.contains("import")) {
                let mut bytes = Vec::new();
                std::io::stdin().read_to_end(&mut bytes)?;
                fs::write(root.join("received.tar.gz"), bytes)?;
            } else {
                println!("{}", json!({"status": "fixture"}));
            }
        }
        "sudo" => {
            ensure!(
                args.starts_with(&["-S".into(), "-p".into()]),
                "unexpected sudo boundary"
            );
            return Err(Command::new(&args[3]).args(&args[4..]).exec().into());
        }
        "bash" => {
            if !args.is_empty() {
                return Err(Command::new(bash).args(args).exec().into());
            }
            let mut script = String::new();
            std::io::stdin().read_to_string(&mut script)?;
            script = script.replace(
                "/var/lib/airlock/machine/receipts",
                root.join("receipts").to_str().unwrap(),
            );
            if config["snapshot"].is_object() {
                let tool = root.join("tools/native-dom0");
                script = script.replace(
                    "/run/current-system/sw/bin/airlock-dom0-service",
                    tool.to_str().unwrap(),
                );
                if let Some(release) = config["release"].as_str() {
                    script = script.replace(
                        &format!("{release}/bin/airlock-dom0-service"),
                        tool.to_str().unwrap(),
                    );
                }
            }
            return Err(Command::new(bash).args(["-c", &script]).exec().into());
        }
        "nix" => {
            if let Some(remote) = config["fetch_remote"].as_str() {
                let target = root.join("apps-fetch");
                for mut command in [
                    {
                        let mut command = Command::new("git");
                        command.args(["init", "--quiet", "--bare"]).arg(&target);
                        command
                    },
                    {
                        let mut command = Command::new("git");
                        command.arg("-C").arg(&target).args([
                            "fetch",
                            "--quiet",
                            remote,
                            "refs/heads/main",
                        ]);
                        command
                    },
                ] {
                    ensure!(
                        command.status()?.success(),
                        "local application fetch failed"
                    );
                }
                let output = Command::new("git")
                    .arg("-C")
                    .arg(target)
                    .args(["rev-parse", "FETCH_HEAD"])
                    .output()?;
                ensure!(output.status.success(), "fetched revision unavailable");
                fs::write(root.join("fetched"), output.stdout)?;
            }
            if config["prune"].as_bool() == Some(true) {
                let mut command = Command::new(
                    std::env::var_os("AIRLOCK_TEST_DEV_EXECUTABLE").context("development CLI")?,
                );
                command
                    .arg("--repository-root")
                    .arg(root.join("parent with spaces"));
                command.args(["source", "prune", "--keep", "0", "--apply"]);
                let output = command.output()?;
                ensure!(
                    output.status.success(),
                    "prune failed: {errors}",
                    errors = String::from_utf8_lossy(&output.stderr)
                );
                fs::write(root.join("pruning.json"), output.stdout)?;
            }
            let release = config["release"]
                .as_str()
                .unwrap_or("/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-built");
            if args.iter().any(|arg| arg == "build") && config["release"].is_string() {
                if let Some(index) = args.iter().position(|arg| arg == "--out-link") {
                    let path = Path::new(&args[index + 1]);
                    fs::create_dir_all(path.parent().context("output parent")?)?;
                    if path.is_symlink() {
                        fs::remove_file(path)?;
                    }
                    symlink(release, path)?;
                }
                println!("{}", json!([{"outputs": {"out": release}}]));
            } else if args.iter().any(|arg| arg == "--all") {
                println!(
                    "{inventory}",
                    inventory = config["inventory"].as_str().unwrap_or("")
                );
            } else if args.iter().any(|arg| arg == "path-info") {
                println!("{}", json!({release: {"narSize": 1024 * 1024 * 1024}}));
            }
            std::process::exit(config["nix_exit"].as_i64().unwrap_or(0) as i32);
        }
        "nix-store" => bail!("unexpected export"),
        "stat" => {
            let key = if args.last().is_some_and(|arg| arg == "/nix/store") {
                "dom0_space"
            } else {
                "bridge_space"
            };
            println!(
                "{report}",
                report = config[key].as_str().unwrap_or("10000000 4096")
            );
        }
        "systemd-run" => println!("started fixture worker"),
        "native-dom0" => {
            let reply = if args
                .first()
                .is_some_and(|arg| arg == "development-snapshot")
            {
                config["snapshot"].clone()
            } else {
                json!({})
            };
            println!("{reply}");
        }
        "cat" if config["boot"].is_object() => println!("{boot}", boot = config["boot"]),
        "cat" => {
            let status = Command::new(config["cat"].as_str().context("fixture cat")?)
                .args(args)
                .status()?;
            ensure!(status.success(), "fixture cat failed");
        }
        _ => bail!("unexpected workstation peer: {name} {args:?}"),
    }
    Ok(())
}
