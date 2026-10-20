//! Mount peers substitute tiny bind mounts for image/overlay construction inside an explicit private namespace.

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{fs, io::Read, path::Path, process::Command};

/// Delegate real mount bookkeeping only within the namespace created by the Rust builder test.
pub(crate) fn execute(root: &Path, name: &str, args: &[String]) -> Result<()> {
    let config: Value = serde_json::from_slice(&fs::read(root.join("tools.json"))?)?;
    let mut command = match name {
        "mount" => {
            let target = Path::new(args.last().context("mount target")?);
            ensure!(
                !(root.join("last-mount").exists() && target.ends_with("merged/nix/store")),
                "injected final mount failure"
            );
            let mut command = Command::new(config["mount"].as_str().context("real mount")?);
            if args.starts_with(&["-t".into(), "erofs".into()])
                || (args.starts_with(&["-t".into(), "overlay".into()])
                    && target.ends_with("lower/nix/store"))
            {
                command.arg("--bind").arg(root.join("source")).arg(target);
            } else if args.starts_with(&["-t".into(), "overlay".into()]) {
                let state = target
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::parent)
                    .context("builder state")?;
                command.arg("--bind").arg(state.join("upper")).arg(target);
            } else {
                command.args(args);
            }
            command
        }
        "umount" => {
            ensure!(
                !root.join("umount-fail").exists(),
                "injected unmount failure"
            );
            let mut command = Command::new(config["umount"].as_str().context("real unmount")?);
            command.args(args);
            command
        }
        "nix-store" => {
            let target = Path::new(args.get(1).context("registration store")?);
            ensure!(
                target.join("nix/store/.links").is_dir(),
                "lower store links missing"
            );
            let mut bytes = Vec::new();
            std::io::stdin().read_to_end(&mut bytes)?;
            ensure!(bytes == b"registration", "unexpected registration bytes");
            fs::create_dir_all(target.join("nix/var/nix/db"))?;
            return Ok(());
        }
        _ => anyhow::bail!("unexpected builder mount peer: {name}"),
    };
    ensure!(command.status()?.success(), "private mount helper failed");
    Ok(())
}
