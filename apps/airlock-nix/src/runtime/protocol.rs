#[cfg(feature = "service")]
use super::decode;
use super::{Result, process, require};
use crate::{
    identity::{GuestName, SourceIdentity},
    review::{CandidateIdentity, DeploymentTarget},
    wire::string_enum,
};
#[cfg(feature = "service")]
use flate2::read::MultiGzDecoder;
#[cfg(feature = "client")]
use flate2::{Compression, write::GzEncoder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    path::Path,
};

#[cfg(feature = "service")]
use std::{
    io::{self, Read, Seek},
    os::fd::AsFd,
    time::{Duration, Instant},
};

pub(super) const ARCHIVE_LIMIT: u64 = 512 * 1024 * 1024;
string_enum! { pub(super) enum RequestSchema ("submission schema") { Ordinary => "airlock.sys-ctrl-guest-request.v1", Rename => "airlock.sys-ctrl-assistant-rename-request.v1" } }
string_enum! { pub(super) enum Action ("submission action") { GuestPlan => "plan", GuestApply => "apply", MachineBuild => "machine-build", MachinePlan => "machine-plan", MachineApply => "machine-apply" } }
impl Action {
    pub fn is_machine(self) -> bool {
        matches!(
            self,
            Self::MachineBuild | Self::MachinePlan | Self::MachineApply
        )
    }
    #[cfg(feature = "service")]
    pub fn is_plan(self) -> bool {
        matches!(self, Self::GuestPlan | Self::MachinePlan)
    }
    #[cfg(feature = "service")]
    pub fn is_apply(self) -> bool {
        matches!(self, Self::GuestApply | Self::MachineApply)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Header {
    pub schema: RequestSchema,
    pub action: Action,
    pub target: Option<GuestName>,
    pub revision: crate::identity::GitRevision,
    pub lock_hash: crate::identity::LockHash,
    pub archive_bytes: u64,
}
impl Header {
    pub fn source(&self) -> SourceIdentity {
        SourceIdentity {
            revision: self.revision.clone(),
            lock_hash: self.lock_hash.clone(),
        }
    }
    pub fn candidate(&self) -> Result<CandidateIdentity> {
        require(
            self.action.is_machine() == self.target.is_none(),
            "deployment target differs from action",
        )?;
        if let Some(target) = &self.target {
            require(
                [
                    "usr-assistant",
                    "sys-ctrl",
                    "sys-gui",
                    "sys-net",
                    "sys-firewall",
                    "usr-home",
                ]
                .contains(&target.as_str()),
                "unsupported guest deployment target",
            )?;
        }
        require(
            self.schema != RequestSchema::Rename || self.action.is_machine(),
            "assistant rename requires a whole-machine action",
        )?;
        require(
            self.archive_bytes > 0 && self.archive_bytes <= ARCHIVE_LIMIT,
            "invalid source archive size",
        )?;
        Ok(CandidateIdentity {
            source: self.source(),
            target: self
                .target
                .clone()
                .map(DeploymentTarget::Guest)
                .unwrap_or(DeploymentTarget::Machine),
        })
    }
    #[cfg(feature = "service")]
    pub fn rename(&self) -> bool {
        self.schema == RequestSchema::Rename
    }
}
pub(super) fn identity(repo: &Path) -> Result<SourceIdentity> {
    let repo = repo
        .to_str()
        .ok_or_else(|| super::Error::Message("non-UTF-8 repository path".into()))?;
    require(
        process::text("git", &["-C", repo, "status", "--porcelain"])?.is_empty(),
        "commit or stash checkout changes before submission",
    )?;
    let revision = process::text("git", &["-C", repo, "rev-parse", "HEAD"])?.parse()?;
    let lock_hash = format!(
        "{:x}",
        Sha256::digest(fs::read(Path::new(repo).join("flake.lock"))?)
    )
    .parse()?;
    Ok(SourceIdentity {
        revision,
        lock_hash,
    })
}
#[cfg(feature = "client")]
pub(super) fn source_archive(repo: &Path, directory: &Path) -> Result<(SourceIdentity, File)> {
    let source = identity(repo)?;
    let repo_arg = repo
        .to_str()
        .ok_or_else(|| super::Error::Message("non-UTF-8 repository path".into()))?;
    require(
        process::text(
            "git",
            &["-C", repo_arg, "submodule", "status", "--recursive"],
        )?
        .is_empty(),
        "source bundles with submodules are unsupported",
    )?;
    let bundle = directory.join("repo.bundle");
    process::output(
        "git",
        &[
            "-C",
            repo_arg,
            "bundle",
            "create",
            bundle.to_str().expect("temporary UTF-8 path"),
            "HEAD",
            "--all",
        ],
    )?;
    require(
        identity(repo)? == source,
        "checkout changed while creating source bundle",
    )?;
    let manifest = serde_json::to_vec(
        &serde_json::json!({"schema":"airlock.sys-agent-machine-source.v1","revision":source.revision,"lock_hash":source.lock_hash,"submodules":[]}),
    )?;
    let archive = File::create(directory.join("source.tar.gz"))?;
    let gzip = GzEncoder::new(archive, Compression::fast());
    let mut tar = tar::Builder::new(gzip);
    tar.append_path_with_name(bundle, "repo.bundle")?;
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    tar.append_data(&mut header, "manifest.json", manifest.as_slice())?;
    tar.into_inner()?.finish()?.sync_all()?;
    let file = File::open(directory.join("source.tar.gz"))?;
    require(
        file.metadata()?.len() <= ARCHIVE_LIMIT,
        "source archive exceeds upload limit",
    )?;
    Ok((source, file))
}

/// One deadline covers the header and compressed body. Read directly from the
/// fd after poll: buffered reads must not wait past that deadline.
#[cfg(feature = "service")]
pub(super) struct Upload {
    input: io::Stdin,
    deadline: Instant,
}
#[cfg(feature = "service")]
impl Upload {
    pub fn new() -> Self {
        Self {
            input: io::stdin(),
            deadline: Instant::now() + Duration::from_secs(120),
        }
    }
    pub fn header<T: serde::de::DeserializeOwned>(&mut self) -> Result<T> {
        let mut bytes = Vec::new();
        while bytes.len() < 4096 {
            let mut byte = [0];
            require(self.read(&mut byte)? == 1, "truncated request header")?;
            bytes.push(byte[0]);
            if byte[0] == b'\n' {
                return decode(&bytes);
            }
        }
        Err(super::Error::Message("request header exceeds limit".into()))
    }
    pub fn archive(&mut self, size: u64) -> Result<File> {
        let mut archive = tempfile::tempfile()?;
        let copied = io::copy(&mut self.take(size), &mut archive)?;
        require(copied == size, "truncated source archive")?;
        archive.rewind()?;
        let mut tar = tar::Archive::new(
            MultiGzDecoder::new(&archive).take(2 * 1024 * 1024 * 1024 + 1024 * 1024),
        );
        let mut names = std::collections::BTreeSet::new();
        for entry in tar.entries()? {
            let mut entry = entry?;
            let name = entry.path()?.into_owned();
            require(
                entry.header().entry_type().is_file(),
                "source archive contains a non-regular member",
            )?;
            require(
                name == Path::new("repo.bundle") || name == Path::new("manifest.json"),
                "unexpected source archive member",
            )?;
            require(
                names.insert(name.clone()),
                "duplicate source archive member",
            )?;
            require(
                entry.size()
                    <= if name == Path::new("manifest.json") {
                        65536
                    } else {
                        2 * 1024 * 1024 * 1024
                    },
                "source archive member exceeds limit",
            )?;
            io::copy(&mut entry, &mut io::sink())?;
        }
        require(
            names.len() == 2,
            "source archive requires manifest.json and repo.bundle",
        )?;
        archive.rewind()?;
        Ok(archive)
    }
}
#[cfg(feature = "service")]
impl Read for Upload {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        loop {
            let remaining = self
                .deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        "source upload exceeded 120 seconds",
                    )
                })?;
            let mut fds = [rustix::event::PollFd::new(
                &self.input,
                rustix::event::PollFlags::IN,
            )];
            let timeout = rustix::event::Timespec {
                tv_sec: remaining.as_secs() as i64,
                tv_nsec: remaining.subsec_nanos() as i64,
            };
            match rustix::event::poll(&mut fds, Some(&timeout)) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "source upload exceeded 120 seconds",
                    ));
                }
                Ok(_) => return rustix::io::read(self.input.as_fd(), buffer).map_err(Into::into),
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}
