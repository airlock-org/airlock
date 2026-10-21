//! Explicit USB inspection, image writing and offline personalization.

pub(crate) mod copy;

pub(crate) mod device;

pub(crate) mod growth;

pub(crate) mod personalize;

pub(crate) mod progress;

use crate::support::{file_digest, regular, root_only, run};
use airlock_nix_core::state::{OperationKind, OperationOwner, StateStore};
use anyhow::{Context, Result, ensure};
use clap::{Args, Parser, Subcommand};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::Command,
};

const MARKER: &str = "airlock-usb-dev-v2\n";

#[derive(Parser)]
#[command(
    about = "Explicit, guarded USB provisioning. No command runs implicitly at boot.",
    version
)]
struct Cli {
    // User-selected operation; inspection alone never grants write authority.
    #[command(subcommand)]
    action: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Inspect by default; explicitly opt into erasing a USB.
    Write(Write),
    /// Use a build result link and an exact hardware serial.
    WriteLatest(Latest),
    /// Write the image, expand root and personalize before first boot.
    ProvisionLatest {
        // Build result and selected disk; this operation always writes.
        #[command(flatten)]
        source: LatestSource,
        // Clean source and authorized SSH key to install offline.
        #[command(flatten)]
        settings: personalize::Settings,
        // Off-USB location for partition-table backups and receipts.
        #[arg(long, default_value = "/var/log/airlock/usb-root-expansion")]
        resize_backup_dir: PathBuf,
    },
    /// Personalize an already mounted Airlock USB root.
    Personalize {
        // Offline ext4 root of the previously written Airlock image.
        #[arg(long)]
        root: PathBuf,
        // Clean source and authorized SSH key to install offline.
        #[command(flatten)]
        settings: personalize::Settings,
    },
}

#[derive(Args)]
struct Write {
    // Uncompressed GPT image whose bytes will be written.
    #[arg(long)]
    image: PathBuf,
    // Whole-disk USB identity; direct /dev/sdX spellings are refused.
    #[arg(long)]
    device: PathBuf,
    // Explicit acknowledgement that this operation destroys USB data.
    #[arg(long)]
    write: bool,
    // Hardware serial repeated independently for a direct write.
    #[arg(long)]
    confirm_serial: Option<String>,
    // SHA-256 expected for the source before destructive access.
    #[arg(long)]
    sha256: Option<String>,
    // Write zero-filled holes as well as allocated source extents.
    #[arg(long)]
    full_write: bool,
    // Read the complete image prefix back; implies a full write.
    #[arg(long)]
    verify: bool,
}

#[derive(Args)]
struct LatestSource {
    // Result symlink produced by the most recent explicit image build.
    #[arg(long)]
    result_link: PathBuf,
    // Physical root of the isolated Nix store if not mounted at /nix.
    #[arg(long)]
    nix_root: Option<PathBuf>,
    // Exact hardware serial, never a model or a kernel disk letter.
    #[arg(long)]
    serial: String,
}

#[derive(Args)]
struct Latest {
    // Image and USB selection shared with full provisioning.
    #[command(flatten)]
    source: LatestSource,
    // Inspection remains the default for the write-latest command.
    #[arg(long)]
    write: bool,
    // Materialize all sparse image holes on the USB.
    #[arg(long)]
    full_write: bool,
    // Verify the entire image prefix after flushing.
    #[arg(long)]
    verify: bool,
}

pub(crate) fn result_image(link: &Path, nix_root: Option<&Path>) -> Result<PathBuf> {
    let result = match fs::canonicalize(link) {
        Ok(result) => result,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let root = fs::canonicalize(nix_root.context(
                "Result link points outside the mounted /nix/store; supply --nix-root.",
            )?)?;
            let target = fs::read_link(link)?;
            let relative = target
                .strip_prefix("/nix/store")
                .context("Dangling result link is not a Nix store output.")?;
            ensure!(
                relative
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
                "Invalid result link path"
            );
            let result = fs::canonicalize(root.join("nix/store").join(relative))?;
            ensure!(
                result.starts_with(&root),
                "Resolved image escaped the configured Nix root."
            );
            result
        }
        Err(error) => return Err(error.into()),
    };

    let candidates = if result.is_file() {
        vec![result.clone()]
    } else {
        fs::read_dir(&result)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?
    };
    let images: Vec<_> = candidates
        .into_iter()
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "img"))
        .map(|path| -> Result<PathBuf> {
            let canonical = fs::canonicalize(path)?;
            ensure!(
                !result.is_dir() || canonical.starts_with(&result),
                "Image escaped the selected build output"
            );
            Ok(canonical)
        })
        .collect::<Result<_>>()?;
    ensure!(
        images.len() == 1,
        "Expected exactly one raw .img under {}; found {}.",
        result.display(),
        images.len()
    );

    Ok(images[0].clone())
}

// A direct image needs an independent digest; a selected build result computes
// its own digest once, immediately before writing the pinned source file.
enum DigestSource {
    Explicit,
    BuildResult,
}

fn latest(
    source: &LatestSource,
    write: bool,
    full_write: bool,
    verify: bool,
) -> Result<(Write, device::Disk)> {
    let image = result_image(&source.result_link, source.nix_root.as_deref())?;
    let device = device::select(&source.serial)?;
    let selected = device::inspect(&device, false)?;
    ensure!(
        selected.serial.as_deref() == Some(&source.serial),
        "USB serial changed during selection"
    );
    Ok((
        Write {
            image,
            device,
            write,
            confirm_serial: Some(source.serial.clone()),
            sha256: None,
            full_write,
            verify,
        },
        selected,
    ))
}

fn write_image(
    args: &Write,
    digest_source: DigestSource,
    selected: Option<&device::Disk>,
) -> Result<()> {
    let image = fs::canonicalize(&args.image)?;
    ensure!(
        image.extension().is_some_and(|e| e == "img"),
        "Use the uncompressed raw .img file."
    );
    let mut source = regular(&image)?;
    let source_info = source.metadata()?;
    let size = source_info.len();
    let info = if selected.is_some() {
        device::inspect(&args.device, false)?
    } else {
        device::checked(&args.device)?
    };
    if let Some(selected) = selected {
        selected.same_disk(&info)?;
    }
    ensure!(size <= info.size, "USB is smaller than the image.");
    ensure!(size > 1024 * 1024, "Image is implausibly small.");
    source.seek(SeekFrom::Start(512))?;
    let mut header = [0; 8];
    source.read_exact(&mut header)?;
    ensure!(&header == b"EFI PART", "Image does not have a GPT header.");

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "device": info.name,
            "identity": args.device,
            "model": info.model,
            "serial": info.serial,
            "bytes": info.size,
            "image": image,
            "image_bytes": size,
            "write": args.write,
            "write_mode": if args.full_write || args.verify { "full" } else { "sparse" },
            "full_verify": args.verify,
        }))?
    );
    if !args.write {
        println!(
            "Inspection only. Writing requires --write --confirm-serial SERIAL --sha256 HASH."
        );
        return Ok(());
    }

    root_only()?;
    ensure!(
        args.confirm_serial == info.serial,
        "Serial confirmation does not match."
    );
    let actual = file_digest(&image)?;
    let expected = match digest_source {
        DigestSource::Explicit => args
            .sha256
            .as_ref()
            .context("Image SHA-256 is required.")?
            .to_ascii_lowercase(),
        DigestSource::BuildResult => actual.clone(),
    };
    ensure!(
        expected.len() == 64 && expected.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid image SHA-256."
    );
    ensure!(actual == expected, "Image SHA-256 does not match.");
    let current_source = fs::metadata(&image)?;
    ensure!(
        source_info.dev() == current_source.dev()
            && source_info.ino() == current_source.ino()
            && source_info.len() == current_source.len()
            && source_info.mtime() == current_source.mtime()
            && source_info.mtime_nsec() == current_source.mtime_nsec()
            && source_info.ctime() == current_source.ctime()
            && source_info.ctime_nsec() == current_source.ctime_nsec(),
        "Image identity changed."
    );

    if selected.is_some() {
        device::unmount(&args.device, &info)?;
    }
    let again = device::checked(&args.device)?;
    info.same_disk(&again)?;
    let before = fs::metadata(&info.name)?;
    ensure!(
        before.file_type().is_block_device(),
        "USB disappeared before opening"
    );
    let mut destination = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags((rustix::fs::OFlags::EXCL | rustix::fs::OFlags::NOFOLLOW).bits() as i32)
        .open(&info.name)?;
    ensure!(
        destination.metadata()?.file_type().is_block_device()
            && destination.metadata()?.rdev() == before.rdev(),
        "USB device changed while opening."
    );

    info.same_disk(&device::checked(&args.device)?)?;

    let written = copy::copy(
        &mut source,
        &mut destination,
        !(args.full_write || args.verify),
    )?;
    let after_copy = source.metadata()?;
    ensure!(
        source_info.len() == after_copy.len()
            && source_info.mtime() == after_copy.mtime()
            && source_info.mtime_nsec() == after_copy.mtime_nsec()
            && source_info.ctime() == after_copy.ctime()
            && source_info.ctime_nsec() == after_copy.ctime_nsec(),
        "Image changed while writing"
    );

    progress::waiting("Flushing USB device", Some(&info.name), || {
        Ok(destination.sync_all()?)
    })?;
    info.same_disk(&device::checked(&args.device)?)?;

    if args.verify {
        destination.seek(SeekFrom::Start(0))?;
        let mut hash = Sha256::new();
        let mut remaining = size;
        let mut bytes = vec![0; 8 * 1024 * 1024];
        let mut progress = progress::Progress::new("Verifying image", size);
        while remaining > 0 {
            let count = remaining.min(bytes.len() as u64) as usize;
            let read = destination.read(&mut bytes[..count])?;
            ensure!(read != 0, "Unexpected end of USB during verification.");
            hash.update(&bytes[..read]);
            remaining -= read as u64;
            progress.report(size - remaining);
        }
        ensure!(
            format!("{:x}", hash.finalize()) == expected,
            "USB read-back verification failed."
        );
    }

    let pinned = device::release_writer(destination)?;
    info.same_disk(&device::checked(&args.device)?)?;
    println!("Refreshing USB partition table…");
    run(Command::new("blockdev").arg("--rereadpt").arg(format!(
        "/proc/{process}/fd/{descriptor}",
        process = std::process::id(),
        descriptor = std::os::fd::AsRawFd::as_raw_fd(&pinned),
    )))
    .context("Image written and flushed, but partition refresh failed; expansion and personalization have not run.")?;
    info.same_disk(&device::checked(&args.device)?)?;

    if args.verify {
        println!("Image fully written ({written} bytes), flushed and verified.");
    } else {
        println!(
            "Image {} written ({written} bytes) and flushed; full read-back verification was skipped.",
            if args.full_write { "fully" } else { "sparsely" }
        );
    }

    println!("Previous USB contents were overwritten and are not recoverable by this tool.");
    Ok(())
}

pub(crate) fn main() -> Result<()> {
    let action = Cli::parse().action;
    let reason = match &action {
        Action::Write(args) if args.write => {
            Some(format!("write image to {}", args.device.display()))
        }
        Action::WriteLatest(args) if args.write => {
            Some(format!("write image to USB serial {}", args.source.serial))
        }
        Action::ProvisionLatest { source, .. } => Some(format!(
            "write, expand and personalize USB serial {}",
            source.serial
        )),
        Action::Personalize { root, .. } => {
            Some(format!("personalize offline USB root {}", root.display()))
        }
        _ => None,
    };
    let _lock = if let Some(reason) = reason {
        root_only()?;
        let directory = Path::new("/run/airlock/usb");
        fs::create_dir_all(directory)?;
        let state = StateStore::initialize(directory)?;
        Some(state.try_operation(
            &"provision".parse()?,
            OperationOwner::new("airlock-usb".into(), reason, OperationKind::Maintenance)?,
        )?)
    } else {
        None
    };

    match action {
        Action::Write(args) => write_image(&args, DigestSource::Explicit, None),
        Action::WriteLatest(args) => {
            let (write, selected) = latest(&args.source, args.write, args.full_write, args.verify)?;
            write_image(&write, DigestSource::BuildResult, Some(&selected))
        }
        Action::Personalize { root, settings } => {
            let selected = device::check_mount(&root, None)?;
            settings.prepare()?.install(&root, &selected)
        }
        Action::ProvisionLatest {
            source,
            settings,
            resize_backup_dir,
        } => {
            let prepared = settings.prepare()?;
            let (args, selected) = latest(&source, true, false, false)?;
            growth::backup_location(&resize_backup_dir, &selected)?;
            write_image(&args, DigestSource::BuildResult, Some(&selected))?;
            run(Command::new("udevadm").arg("settle"))?;
            growth::expand(&args.device, &selected, &resize_backup_dir)?;

            let root = tempfile::Builder::new()
                .prefix("airlock-usb-")
                .tempdir_in("/run")?
                .keep();
            let current = device::checked(&args.device)?;
            selected.same_disk(&current)?;
            let partition = current.partition(&args.device)?;
            let mount = growth::Mounted::new(&partition, &root, "nosuid,nodev")?;
            prepared.install(&root, &selected)?;
            progress::waiting(
                "Flushing personalized USB filesystem",
                Some(&args.device),
                || run(&mut Command::new("sync")),
            )?;
            progress::waiting("Unmounting personalized USB", Some(&args.device), || {
                mount.unmount()
            })
        }
    }
}
