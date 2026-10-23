//! Inert tools for testing the retained configuration transport's source, activation and status boundaries.

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

/// Persist fixture-owned JSON without invoking another interpreter or command.
fn save(path: impl AsRef<Path>, value: &Value) -> Result<()> {
    let path = path.as_ref();
    fs::create_dir_all(path.parent().context("fixture output parent")?)?;
    fs::write(path, serde_json::to_vec(value)?)?;
    Ok(())
}

/// Return finite supplied outputs and observable fixture effects; the actual shell commands own orchestration.
pub fn execute(root: &Path, name: &str, arguments: &[String]) -> Result<()> {
    let data: Value = serde_json::from_slice(&fs::read(root.join("config.json"))?)?;
    let mut calls = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("calls.jsonl"))?;
    writeln!(calls, "{}", json!({"command": name, "args": arguments}))?;
    ensure!(data["fail"] != name, "injected helper failure: {name}");
    let logs = std::env::var_os("AIRLOCK_LOG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("logs"));
    let state = std::env::var_os("AIRLOCK_SYSTEM_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("state"));
    let candidate = || -> Result<&str> { data["commit"].as_str().context("candidate commit") };
    match name {
        "airlock-config-archive-candidate" => {
            let control = data["control"] == true;
            let failed = control && data["control_failure"] == true;
            let archive = json!({"schema": "airlock.config-archive.v1", "status": if failed { "failed" } else { "passed" },
                "candidate_commit": candidate()?, "validation_commit": candidate()?,
                "validation":serde_json::from_slice::<Value>(&fs::read(root.join("validation.json"))?)?,
                "submitted":{"commit":candidate()?,"bundle":airlock_nix_core::configuration::file_hash(&root.join("config.bundle"))?,"manifest":airlock_nix_core::configuration::file_hash(&root.join("manifest.json"))?},
                "validated_candidate_sha256":airlock_nix_core::configuration::file_hash(&root.join("candidate.tar"))?,
                "vault_archive": {"schema": "airlock.config-vault-archive.v1", "status": "passed", "candidate_commit": candidate()?}});
            save(logs.join("config-archive.json"), &archive)?;
            if control {
                let index = arguments
                    .iter()
                    .position(|arg| arg == "--candidate-tar")
                    .context("controlled candidate archive")?;
                fs::copy(&arguments[index + 1], root.join("control-candidate.tar"))?;
            } else {
                fs::copy(
                    root.join("candidate.tar"),
                    logs.join("config-candidate-validated.tar"),
                )?;
            }
            if failed {
                std::process::exit(1);
            }
        }
        "airlock-config-apply" => {
            let index = arguments
                .iter()
                .position(|arg| arg == "--candidate-tar")
                .context("controlled candidate archive")?;
            fs::copy(&arguments[index + 1], root.join("control-candidate.tar"))?;
            let failed = data["control_failure"] == true;
            save(
                logs.join("config-apply.json"),
                &json!({"schema": "airlock.config-apply.v1", "status": if failed { "failed" } else { "passed" }}),
            )?;
            if failed {
                std::process::exit(1);
            }
            fs::write(root.join("control-activated"), "")?;
        }
        "airlock-setup-config" => {
            let index = arguments
                .iter()
                .position(|arg| arg == "--topology")
                .context("setup topology")?;
            fs::copy(&arguments[index + 1], root.join("setup-topology.json"))?;
            save(
                logs.join("setup-config.json"),
                &json!({"schema": "airlock.setup-config.v1", "status": "passed", "domains_changed": 0, "templates_changed": 0}),
            )?;
        }
        "qrexec-client" => {
            let mut payload = Vec::new();
            std::io::stdin().read_to_end(&mut payload)?;
            if arguments
                .last()
                .is_some_and(|arg| arg.contains("airlock.Config.ValidateCandidate"))
            {
                std::io::stdout().write_all(&fs::read(root.join("candidate.tar"))?)?;
            } else {
                fs::write(root.join("deployment.tar"), payload)?;
                println!(
                    "{}",
                    json!({"schema": "airlock.config-vault-deployment.v1", "status": "passed", "deployment_status": "passed", "candidate_commit": candidate()?, "vault_commit": "fixture-vault"})
                );
            }
        }
        "airlock-config-status" => {
            std::io::stdout().write_all(&fs::read(root.join("status.json"))?)?;
        }
        "airlock-dom0-profile-activate" => {
            fs::create_dir_all(&state)?;
            fs::write(state.join("profile-activated"), "activated\n")?;
        }
        "switch-to-configuration" => {
            let action = arguments.first().context("host switch action")?;
            fs::create_dir_all(&state)?;
            fs::write(state.join("host-switch-action"), action)?;
            let own = std::env::args_os().next().context("switch executable")?;
            let source = Path::new(&own)
                .parent()
                .and_then(Path::parent)
                .context("switch source")?;
            fs::write(
                state.join("host-switch-source"),
                source.as_os_str().as_encoded_bytes(),
            )?;
            if action != "boot" {
                let link = std::env::var_os("AIRLOCK_HOST_CURRENT_SYSTEM_LINK")
                    .context("current host system")?;
                let link = Path::new(&link);
                if fs::symlink_metadata(link).is_ok() {
                    fs::remove_file(link)?;
                }
                symlink(source, link)?;
            }
        }
        "nix" => {
            ensure!(
                arguments.iter().any(|arg| arg == "--offline"),
                "configuration build lost offline policy"
            );
            if arguments.iter().any(|arg| arg == "check") {
                return Ok(());
            }
            if arguments.iter().any(|arg| arg == "eval") {
                let attribute = arguments
                    .iter()
                    .find(|arg| arg.contains('#'))
                    .and_then(|arg| arg.split_once('#'))
                    .map(|(_, attr)| attr)
                    .context("evaluation selector")?;
                let name = match attribute {
                    "airlock.topology" => "topology.json",
                    "airlock.system" => "system.json",
                    _ => bail!("unexpected evaluation selector"),
                };
                std::io::stdout().write_all(&fs::read(root.join(name))?)?;
                return Ok(());
            }
            let attribute = arguments
                .last()
                .and_then(|arg| arg.split_once('#'))
                .map(|(_, attr)| attr)
                .context("build output selector")?;
            println!(
                "{output}",
                output = data["outputs"][attribute]
                    .as_str()
                    .context("unexpected build selector")?
            );
        }
        "qvm-ls" => {
            ensure!(
                data["inventory_error"] != true,
                "Qubes inventory unavailable"
            );
            if arguments
                .iter()
                .any(|argument| argument == "NAME,CLASS,STATE,TEMPLATE,LABEL")
            {
                for row in data["qvm_rows"].as_array().context("Qubes status rows")? {
                    println!("{row}", row = row.as_str().unwrap());
                }
                return Ok(());
            }
            for domain in data["domains"].as_array().context("Qubes domains")? {
                println!("{domain}", domain = domain.as_str().unwrap());
            }
        }
        "qvm-prefs" => {
            let domain = arguments.first().context("preference VM")?;
            let field = arguments.get(1).context("preference field")?;
            if domain == "sys-config-validator" {
                println!(
                    "{value}",
                    value = if field == "provides_network" {
                        "False"
                    } else {
                        ""
                    }
                );
            } else {
                ensure!(
                    data["domains"].as_array().unwrap().contains(&json!(domain)),
                    "unknown VM"
                );
                println!(
                    "{value}",
                    value = if field == "provides_network" {
                        "True"
                    } else if domain == "sys-firewall" {
                        "sys-net"
                    } else {
                        ""
                    }
                );
            }
        }
        "qvm-check" | "qvm-start" | "systemctl" => {}
        "nix-env" => bail!("unexpected mutable Nix profile command"),
        _ => bail!("unexpected configuration fixture command {name}"),
    }
    Ok(())
}
