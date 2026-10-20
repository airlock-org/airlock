//! Native host-store entry points used during deployment, recovery and early boot.
//! Boot installation runs in a private mount namespace; approval never implies an immediate reboot.

use super::{boot_artifacts, fs::*, host, host_boot, host_install, host_machine};
use anyhow::{Context, Result, ensure};
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Parsed store request, validated by the application's single command parser.
#[derive(Args)]
pub(crate) struct Request {
    // Selected pool for offline inspection or recovery; live publication uses the installed pool.
    #[arg(long, global = true, default_value = "/var/lib/airlock/closure-store")]
    pool_root: PathBuf,
    // EFI partition containing the entries being inspected or selected.
    #[arg(long = "efi-directory", global = true, default_value = "/boot")]
    efi_directory: PathBuf,
    // Avoid firmware-variable access when editing an offline image.
    #[arg(long, global = true)]
    offline: bool,
    // Host store operation; no command implicitly reboots the machine.
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    Validate {
        // Exact admitted system to authenticate.
        system: String,
    },
    Publish {
        // Built host system whose live closure and writable additions are frozen.
        system: String,
    },
    Select {
        // Installed and verified boot choice to select without rebooting.
        system: String,
    },
    Retire {
        // Inactive host boot choice whose retention may be released.
        system: String,
    },
    BootSystem {
        // Kernel command line to inspect; defaults to the currently booted machine.
        #[arg(long)]
        cmdline: Option<String>,
    },
    ProtectBoot,
    SealBoot {
        // Preserve the already selected loader default during preparation.
        #[arg(long)]
        keep_default: bool,
    },
    Recover {
        // Previously retained boot choice to select from independent recovery.
        system: String,
    },
    CheckpointMachine,
    BindMachine {
        // Host system to bind to the current committed machine selection.
        system: String,
    },
    InstallBoot {
        // Host system whose upstream boot installation must be isolated.
        system: String,
    },
    InstallBootIsolated {
        // Host system to install after entering a private mount namespace.
        system: String,
    },
    Assemble {
        // Real root mounted by initramfs before stage two starts.
        #[arg(long, default_value = "/sysroot")]
        sysroot: PathBuf,
    },
    MountOverlay {
        // Root used by this dedicated chrooted mount child.
        #[arg(long, default_value = "/sysroot")]
        sysroot: PathBuf,
    },
    BootLabels {
        /// EFI filesystem containing upstream loader entries.
        #[arg(long)]
        esp: PathBuf,
        /// Optional already configured native boot-menu command.
        #[arg(long)]
        menu: Option<PathBuf>,
    },
    BootMenu {
        // Validate menu identity and report intended choices without modifying files or profiles.
        #[arg(long)]
        dry_run: bool,
        // EFI system partition containing native and retained loader entries.
        #[arg(long)]
        esp: PathBuf,
        // Physical store used to read the currently available source markers.
        #[arg(long, default_value = "/nix/store")]
        store_root: PathBuf,
        // Optional profile pin protecting the current independent recovery system.
        #[arg(long)]
        recovery_profile: Option<PathBuf>,
        // Nix profile tool supplied by the configured host package set.
        #[arg(long, default_value = "nix-env")]
        nix_env: String,
    },
    Status,
}

pub(super) fn run(command: Request) -> Result<Value> {
    let pool = command.pool_root.as_path();
    let firmware = !command.offline;
    let storage = Path::new(host::HOST);
    let machine = Path::new(host::MACHINE);
    let esp = command.efi_directory.as_path();
    match command.operation {
        Operation::BootLabels { esp, menu } => {
            label_entries(&esp)?;
            if let Some(menu) = menu {
                ensure!(
                    std::process::Command::new(menu).status()?.success(),
                    "boot menu publication failed"
                );
            }
            Ok(json!({"labeled": true}))
        }
        Operation::BootSystem { cmdline } => {
            let cmdline = match cmdline {
                Some(cmdline) => cmdline,
                None => fs::read_to_string("/proc/cmdline")?,
            };
            Ok(json!({"system": host_boot::boot_system(&cmdline)?}))
        }
        Operation::BootMenu {
            dry_run,
            esp,
            store_root,
            recovery_profile,
            nix_env,
        } => super::presentation::configure_menu(
            &esp,
            &store_root,
            recovery_profile.as_deref(),
            &nix_env,
            firmware,
            dry_run,
        ),
        Operation::Assemble { sysroot } => {
            let result = host_boot::assemble(&sysroot, &fs::read_to_string("/proc/cmdline")?);
            if let Err(error) = &result {
                host_boot::report_failure(&sysroot, error);
            }
            result
        }
        Operation::MountOverlay { sysroot } => {
            host_boot::mount_overlay(&sysroot)?;
            Ok(json!({"mounted":true}))
        }
        Operation::InstallBoot { system } => {
            entry(&system)?;
            command_process(&system)?;
            Ok(json!({"installed":system}))
        }
        Operation::InstallBootIsolated { system } => {
            host_install::install(
                &system,
                Path::new("/nix/var/nix/profiles"),
                &esp.join("loader"),
                pool,
            )?;
            Ok(json!({"installed":system}))
        }
        Operation::Publish { system } => {
            ensure!(
                pool == Path::new(host::POOL),
                "live publication requires the installed pool"
            );
            host::publish_live(&system)
        }
        Operation::Status => Ok(json!({
            "schema": "airlock.host-store.status.v1",
            "pool_allocated_bytes": host::allocated(pool)?,
            "upper_allocated_bytes": host::allocated(&storage.join("active/upper"))?,
            "generation_state_allocated_bytes": host::allocated(&storage.join("generations"))?,
            "recovery_allocated_bytes": host::allocated(Path::new("/var/lib/airlock/recovery-store"))?,
        })),
        operation => {
            ensure_pool(pool)?;
            let _lock = PoolLock::acquire(&pool.join("admission.lock"))?;
            match operation {
                Operation::Validate { system } => host::validate(pool, pool, &system, false),
                Operation::Select { system } => host::select(pool, esp, &system, firmware),
                Operation::Retire { system } => host::retire(pool, esp, &system),
                Operation::Recover { system } => {
                    ensure!(
                        Path::new("/etc/airlock/pool-recovery").is_file(),
                        "recover is available only in independent recovery"
                    );
                    host::select(pool, esp, &system, firmware)
                }
                Operation::ProtectBoot => {
                    let running: Value = read(&pool.join("host/running.json"))?;
                    host::select(pool, esp, &text(&running, "system")?, firmware)
                }
                Operation::SealBoot { keep_default } => {
                    boot_artifacts::seal(pool, esp, keep_default, firmware)?;
                    Ok(json!({"sealed":true}))
                }
                Operation::BindMachine { system } => {
                    let record = host::validate(pool, pool, &system, false)?;
                    host_machine::save(pool, storage, machine, &record)?;
                    Ok(json!({"bound":system}))
                }
                Operation::CheckpointMachine => {
                    let record: Value = read(&pool.join("host/running.json"))?;
                    let selection = machine.join("selection.json");
                    if selection.try_exists()? {
                        let selection: Value = read(&selection)?;
                        let release = PathBuf::from(text(&selection, "release")?);
                        let manifest: Value = read(&release.join("release.json"))?;
                        if manifest["host"] != record["system"] {
                            return Ok(json!({"checkpoint":"unchanged","system":record["system"]}));
                        }
                    }
                    host_machine::save(pool, storage, machine, &record)?;
                    Ok(json!({"bound":record["system"]}))
                }
                _ => unreachable!("handled before acquiring pool lock"),
            }
        }
    }
}

fn command_process(system: &str) -> Result<()> {
    command(
        "unshare",
        &[
            "--mount",
            "--propagation",
            "private",
            std::env::current_exe()?
                .to_str()
                .context("invalid service path")?,
            "storage",
            "host",
            "install-boot-isolated",
            system,
        ],
        None,
    )?;
    Ok(())
}

/// Preserve Xen titles and mark independent NixOS recovery entries during boot installation.
fn label_entries(esp: &Path) -> Result<()> {
    let directory = esp.join("loader/entries");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(".conf")
            || !(name.starts_with("xen-") || name.starts_with("nixos-"))
            || !entry.path().is_file()
        {
            continue;
        }
        let original = fs::read_to_string(entry.path())?;
        let mut changed = String::new();
        for line in original.split_inclusive('\n') {
            if let Some(title) = line.strip_prefix("title ") {
                let newline = title.ends_with('\n');
                let title = title
                    .trim_end_matches('\n')
                    .replace(" (Recovery: without Xen)", "");
                changed.push_str("title ");
                changed.push_str(&title);
                if name.starts_with("nixos-") {
                    changed.push_str(" (Recovery: without Xen)");
                }
                if newline {
                    changed.push('\n');
                }
            } else {
                changed.push_str(line);
            }
        }
        if changed != original {
            fs::write(entry.path(), changed)?;
        }
    }
    Ok(())
}
