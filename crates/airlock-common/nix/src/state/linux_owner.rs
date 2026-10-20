//! Linux-only owner diagnostics. These observe and describe kernel exclusion;
//! they never acquire locks, grant authority or decide whether work may run.

use super::lock::{LockHolder, LockObservation, LockOwnerDetails, OperationOwner, check_named};
use super::{open_file, parse_json};
use crate::{Error, Result, error::io_error};
use rustix::fs::{Mode, OFlags, major, minor};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, Metadata},
    io::{Read, Seek, SeekFrom},
    num::NonZeroU32,
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, MetadataExt},
    },
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Advisory acquisition identity checked against procfs before naming a holder.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OwnerRecord {
    /// Wire format identifier used to reject unsupported record versions.
    schema: String,
    /// Named operation and reason published by the lock holder.
    owner: OperationOwner,
    /// Process identity checked against the current boot and process start time.
    pid: u32,
    /// Process start time from procfs, preventing PID reuse from matching a stale owner.
    process_start: u64,
    /// Host boot identity, preventing metadata from a prior boot being trusted.
    boot_id: String,
    /// Open descriptor number used to verify the recorded lock holder.
    lock_fd: i32,
    /// Random seek position tying diagnostics to this lock acquisition.
    lock_position: u64,
    /// Wall-clock acquisition time in seconds since the Unix epoch.
    acquired_unix_seconds: u64,
}

/// Exactly one owner is stored, even when process identification fails.
#[derive(Debug)]
pub(super) enum OwnerDiagnostics {
    Identified {
        /// Process and descriptor identity captured for this acquisition.
        record: OwnerRecord,
        /// Optional failure while publishing the advisory owner record.
        error: Option<String>,
    },
    Unavailable {
        /// Named operation retained even when process diagnostics fail.
        owner: OperationOwner,
        /// Why a verifiable process identity could not be captured.
        error: String,
    },
}

impl OwnerDiagnostics {
    pub(super) fn capture(file: &mut File, owner: OperationOwner) -> Self {
        match identify(file, &owner) {
            Ok(record) => Self::Identified {
                record,
                error: None,
            },
            Err(error) => Self::Unavailable {
                owner,
                error: error.to_string(),
            },
        }
    }

    pub(super) fn owner(&self) -> &OperationOwner {
        match self {
            Self::Identified { record, .. } => &record.owner,
            Self::Unavailable { owner, .. } => owner,
        }
    }

    pub(super) fn error(&self) -> Option<&str> {
        match self {
            Self::Identified { error, .. } => error.as_deref(),
            Self::Unavailable { error, .. } => Some(error),
        }
    }

    pub(super) fn update_phase(&mut self, file: &File, phase: String) -> Result<()> {
        match self {
            Self::Identified { record, .. } => record.owner.set_phase(phase)?,
            Self::Unavailable { owner, .. } => owner.set_phase(phase)?,
        }

        self.publish(file);
        Ok(())
    }

    pub(super) fn publish(&mut self, file: &File) {
        let Self::Identified { record, error } = self else {
            return;
        };

        let result = (|| -> std::io::Result<()> {
            let bytes = serde_json::to_vec(record)?;

            // Never rename/unlink a lock file. Positional writes preserve the
            // acquisition nonce in the kernel descriptor's seek position.
            // Readers may see an incomplete diagnostic and report Unknown.
            file.set_len(0)?;
            file.write_all_at(&bytes, 0)?;
            Ok(())
        })();
        *error = result.err().map(|error| error.to_string());
    }
}

fn identify(file: &mut File, owner: &OperationOwner) -> std::io::Result<OwnerRecord> {
    let mut random = [0_u8; 4];
    File::open("/dev/urandom")?.read_exact(&mut random)?;

    // Keep this within ordinary filesystem seek limits; no file allocation
    // occurs because diagnostic writes use positional I/O at offset zero.
    let position = u64::from(u32::from_ne_bytes(random)) + 1;
    file.seek(SeekFrom::Start(position))?;

    let pid = std::process::id();
    Ok(OwnerRecord {
        schema: "airlock.nix-lock-owner.v1".to_owned(),
        owner: owner.clone(),
        pid,
        process_start: process_start(pid)
            .ok_or_else(|| std::io::Error::other("process identity unavailable"))?,
        boot_id: boot_id().ok_or_else(|| std::io::Error::other("boot identity unavailable"))?,
        lock_fd: file.as_raw_fd(),
        lock_position: position,
        acquired_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)?
            .as_secs(),
    })
}

/// Read-only kernel/owner inspection: no file creation, repair or lock probe.
/// Linux procfs must expose the holder in this PID namespace. Unavailable
/// procfs or stale metadata never establishes an unlocked/verified owner.
pub(super) fn observe(path: &Path) -> Result<LockObservation> {
    let file = match open_file(path, OFlags::RDONLY, Mode::empty()) {
        Ok(file) => file,
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LockObservation::Unlocked);
        }
        Err(error) => return Err(error),
    };

    let metadata = file
        .metadata()
        .map_err(|error| io_error("inspect lock", path, error))?;
    check_named(&file, path)?;

    let holder = match kernel_holder(&metadata) {
        KernelLock::Unavailable => return Ok(LockObservation::Unknown),
        KernelLock::Unlocked => return Ok(LockObservation::Unlocked),
        KernelLock::Shared => LockHolder::Unknown,
        KernelLock::Exclusive { pid } => verify_owner(&file, path, &metadata, pid)
            .map_or(LockHolder::Unknown, |details| {
                LockHolder::Verified(Box::new(details))
            }),
    };

    Ok(LockObservation::Held { holder })
}

fn bounded_text(path: &Path, limit: u64) -> Option<String> {
    let file = open_file(path, OFlags::RDONLY, Mode::empty()).ok()?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;

    (bytes.len() as u64 <= limit)
        .then(|| String::from_utf8(bytes).ok())
        .flatten()
}

fn process_start(pid: u32) -> Option<u64> {
    let stat = bounded_text(Path::new(&format!("/proc/{pid}/stat")), 8192)?;
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

fn boot_id() -> Option<String> {
    let boot = bounded_text(Path::new("/proc/sys/kernel/random/boot_id"), 128)?;
    let boot = boot.trim();
    (!boot.is_empty()).then(|| boot.to_owned())
}

fn matches_inode(text: &str, metadata: &Metadata) -> bool {
    let fields: Vec<_> = text.split(':').collect();
    fields.len() == 3
        && u32::from_str_radix(fields[0], 16).ok() == Some(major(metadata.dev()))
        && u32::from_str_radix(fields[1], 16).ok() == Some(minor(metadata.dev()))
        && fields[2].parse::<u64>().ok() == Some(metadata.ino())
}

#[derive(Clone, Copy, Debug)]
enum KernelLock {
    Unavailable,
    Unlocked,
    Shared,
    Exclusive {
        /// Kernel-reported holder of the exclusive flock.
        pid: NonZeroU32,
    },
}

// /proc/locks and fdinfo's lock entries share this record format. Ignore
// unrelated inodes and blocked waiters; recognize unsupported holders explicitly.
fn parse_flock(line: &str, metadata: &Metadata) -> Option<KernelLock> {
    let fields: Vec<_> = line.split_whitespace().collect();
    if fields.len() != 8 || fields[1] != "FLOCK" || !matches_inode(fields[5], metadata) {
        return None;
    }

    Some(match fields[3] {
        "READ" => KernelLock::Shared,
        "WRITE" => fields[4]
            .parse::<NonZeroU32>()
            .map_or(KernelLock::Unavailable, |pid| KernelLock::Exclusive { pid }),
        _ => KernelLock::Unavailable,
    })
}

fn kernel_holder(metadata: &Metadata) -> KernelLock {
    let Some(locks) = bounded_text(Path::new("/proc/locks"), 4 * 1024 * 1024) else {
        return KernelLock::Unavailable;
    };
    locks
        .lines()
        .find_map(|line| parse_flock(line, metadata))
        .unwrap_or(KernelLock::Unlocked)
}

fn verify_owner(
    file: &File,
    path: &Path,
    metadata: &Metadata,
    pid: NonZeroU32,
) -> Option<LockOwnerDetails> {
    let pid = pid.get();
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 65536 {
        return None;
    }

    let record: OwnerRecord = parse_json(&bytes, path).ok()?;
    if record.pid != pid
        || record.schema != "airlock.nix-lock-owner.v1"
        || record.lock_fd < 0
        || record.lock_position == 0
        || process_start(pid)? != record.process_start
        || boot_id()? != record.boot_id
    {
        return None;
    }

    let descriptor = PathBuf::from(format!("/proc/{pid}/fd/{}", record.lock_fd));
    let actual = fs::metadata(descriptor).ok()?;
    if (actual.dev(), actual.ino()) != (metadata.dev(), metadata.ino()) {
        return None;
    }

    let info = bounded_text(
        Path::new(&format!("/proc/{pid}/fdinfo/{}", record.lock_fd)),
        65536,
    )?;
    let position: u64 = info
        .lines()
        .find_map(|line| line.strip_prefix("pos:"))?
        .trim()
        .parse()
        .ok()?;
    if position != record.lock_position {
        return None;
    }

    let held = info.lines().filter_map(|line| line.strip_prefix("lock:")).any(|line| {
        matches!(parse_flock(line, metadata), Some(KernelLock::Exclusive { pid: holder }) if holder.get() == pid)
    });
    if !held || process_start(pid)? != record.process_start || check_named(file, path).is_err() {
        return None;
    }

    Some(LockOwnerDetails::new(
        record.owner,
        pid,
        record.acquired_unix_seconds,
    ))
}
