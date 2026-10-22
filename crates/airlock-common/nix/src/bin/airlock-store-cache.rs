//! Prepare immutable package fragments and indexes independently of dom0's service implementation.

use airlock_nix_core::artifact::cache;
use anyhow::{Result, bail};
use std::{env, path::Path};

/// Accept fixed build-time operations; no pool paths or Qubes control interface are exposed.
fn run() -> Result<()> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    match args.as_slice() {
        [operation, logical, source, output] if operation == "fragment" => {
            cache::pack(
                logical
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("non-UTF8 store path"))?,
                Path::new(source),
                Path::new(output),
            )?;
        }
        [operation, manifest, output, mkfs] if operation == "index" => {
            cache::build_index(Path::new(manifest), Path::new(output), mkfs)?;
        }
        _ => bail!(
            "usage: airlock-store-cache fragment STORE_PATH SOURCE OUTPUT | index MANIFEST OUTPUT MKFS"
        ),
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("airlock-store-cache: {error:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
