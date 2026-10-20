//! Finite image acceptance peers record trials and simulate concurrent input or publication changes.

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::symlink,
    path::Path,
    time::Duration,
};

/// Execute only the offline tool calls selected by an owned temporary fixture.
pub(crate) fn execute(root: &Path, name: &str, args: &[String]) -> Result<()> {
    match name {
        "nix" => println!("nix 2.test"),
        "nix-store" => {
            if args.iter().any(|arg| arg == "--load-db") {
                std::io::stdin().read_to_end(&mut Vec::new())?;
            }
            if let Some(index) = args.iter().position(|arg| arg == "--add-root") {
                ensure!(
                    args.iter().any(|arg| arg == "--indirect"),
                    "indirect root required"
                );
                ensure!(
                    args.iter().any(|arg| arg == "--realise"),
                    "realisation required"
                );
                let target = Path::new(args.get(index + 1).context("root path")?);
                if target.is_symlink() {
                    fs::remove_file(target)?;
                }
                symlink(args.last().context("root destination")?, target)?;
            } else if args.iter().any(|arg| arg == "--realise") && root.join("mutate").exists() {
                let target = fs::read_to_string(root.join("mutate"))?;
                OpenOptions::new()
                    .append(true)
                    .open(target)?
                    .write_all(b" ")?;
            }
        }
        "bwrap" => {
            for required in ["--unshare-net", "--offline", "--clearenv"] {
                ensure!(args.iter().any(|arg| arg == required), "missing {required}");
            }
            let index = args
                .iter()
                .position(|arg| arg == "NIX_CONFIG")
                .context("Nix configuration")?;
            ensure!(
                args[index + 1].contains("substituters = "),
                "network substituters disabled"
            );
            let source = args
                .iter()
                .find(|arg| arg.starts_with("path:") && arg.contains("#nixosConfigurations"))
                .context("trial configuration")?;
            let path = Path::new(source[5..].split('#').next().unwrap());
            let name = path
                .parent()
                .and_then(Path::file_name)
                .context("trial name")?;
            writeln!(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(root.join("trials"))?,
                "{name}",
                name = name.to_string_lossy()
            )?;

            if root.join("sleep").exists() {
                std::thread::sleep(Duration::from_secs(5));
            }
            ensure!(!root.join("fail").exists(), "trial failed");
            println!(
                "{}",
                json!([{"outputs": {"out": format!("/nix/store/{hash}-system", hash = "d".repeat(32))}}])
            );
            if root.join("retarget.json").exists() {
                let value: Value = serde_json::from_slice(&fs::read(root.join("retarget.json"))?)?;
                let alias = value["alias"].as_str().context("output alias")?;
                fs::remove_file(alias)?;
                symlink(
                    value["target"].as_str().context("new output parent")?,
                    alias,
                )?;
            }
        }
        "image-builder" => {
            let seed = std::env::var("AIRLOCK_MACHINE_SEED_ARCHIVE")?;
            let inputs = std::env::var("AIRLOCK_MACHINE_OFFLINE_INPUTS")?;
            writeln!(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(root.join("builds"))?,
                "{}",
                json!({"seed": seed, "inputs": inputs})
            )?;
            let index = args
                .iter()
                .position(|arg| arg == "--out-link")
                .context("output link")?;
            symlink(
                format!("/nix/store/{hash}-image", hash = "g".repeat(32)),
                &args[index + 1],
            )?;
            if root.join("tamper").exists() {
                fs::write(inputs, "changed during build")?;
            }
        }
        _ => bail!("unexpected image gate peer: {name} {args:?}"),
    }
    Ok(())
}
