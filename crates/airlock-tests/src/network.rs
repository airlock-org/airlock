//! Finite network-tool responses keep enrollment and provider readiness tests offline.

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::process::CommandExt,
    path::Path,
    process::Command,
};

/// Report a single selected failure while allowing the real startup shell to check services individually.
pub(crate) fn execute(root: &Path, name: &str, args: &[String]) -> Result<()> {
    let config: Value = serde_json::from_slice(&fs::read(root.join("network.json"))?)?;
    writeln!(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("calls.jsonl"))?,
        "{}",
        json!({"command": name, "args": args})
    )?;
    let failure = config["failure"].as_str().unwrap_or("");
    match name {
        "ip" => {
            if failure != "route" {
                println!("default via 10.137.0.1");
            }
        }
        "getent" => ensure!(failure != "dns", "DNS unavailable"),
        "curl" => ensure!(failure != "https", "HTTPS unavailable"),
        "tailscale" => fs::write(root.join("enrolled"), serde_json::to_vec(args)?)?,
        "timeout" => {
            let index = args
                .iter()
                .position(|arg| {
                    ["tailscale", "getent", "qvm-start", "qrexec-client"].contains(&arg.as_str())
                })
                .context("unexpected timeout operation")?;
            let name = &args[index];
            if name == "tailscale" && failure == "login" {
                std::process::exit(124);
            }
            return Err(Command::new(root.join("bin").join(name))
                .args(&args[index + 1..])
                .exec()
                .into());
        }
        "qrexec-client" => {
            let command = args.get(2).context("remote readiness command")?;
            if command.starts_with("user:") {
                println!("/nix/store/expected");
            } else {
                let script = command
                    .strip_prefix("root:")
                    .context("provider probe user")?;
                return Err(
                    Command::new(config["bash"].as_str().context("fixture shell")?)
                        .args(["-c", script])
                        .exec()
                        .into(),
                );
            }
        }
        "systemctl" => {
            ensure!(
                args.len() > 3
                    || config["healthy"] != false
                    || args
                        .get(2)
                        .is_none_or(|arg| arg != "qubes-firewall.service"),
                "firewall inactive"
            );
        }
        "qvm-start" | "qvm-check" | "qvm-ls" | "supervisor" => {}
        _ => bail!("unexpected network peer: {name}"),
    }
    Ok(())
}
