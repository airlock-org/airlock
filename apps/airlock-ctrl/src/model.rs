use crate::Decision;
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BLOB: usize = 1024 * 1024;
const MAX_PLAN: u64 = 16 * 1024 * 1024;

#[derive(Clone, Serialize)]
pub struct Revision {
    pub commit: String,
    pub subject: String,
}

#[derive(Clone, Serialize)]
pub struct FileChange {
    pub path: String,
    pub status: String,
    pub old_mode: Option<String>,
    pub new_mode: Option<String>,
    pub old_oid: Option<String>,
    pub new_oid: Option<String>,
    pub before: String,
    pub after: String,
    pub opaque: Option<String>,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Clone, Serialize)]
pub struct Review {
    pub schema: &'static str,
    pub mode: &'static str,
    pub repository: Option<String>,
    pub baseline: Revision,
    pub candidate: Revision,
    pub lock_hash: Option<String>,
    pub files: Vec<FileChange>,
    pub deployment: Option<Value>,
    pub fingerprint: String,
}

#[derive(Clone)]
struct Entry {
    mode: String,
    oid: String,
}

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("--no-pager")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .output()
        .context("running Git")?;
    ensure!(
        output.status.success(),
        "Git failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout)
}

fn revision(repo: &Path, reference: &str) -> Result<Revision> {
    let commit = String::from_utf8(git(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{reference}^{{commit}}"),
        ],
    )?)?
    .trim()
    .to_owned();
    let subject = String::from_utf8(git(repo, &["show", "-s", "--format=%s", &commit])?)?
        .trim()
        .to_owned();
    Ok(Revision { commit, subject })
}

fn tree(repo: &Path, commit: &str) -> Result<BTreeMap<String, Entry>> {
    let raw = git(repo, &["ls-tree", "-r", "-z", "--full-tree", commit])?;
    let mut entries = BTreeMap::new();
    for record in raw.split(|byte| *byte == 0).filter(|item| !item.is_empty()) {
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .context("invalid Git tree record")?;
        let fields: Vec<_> = std::str::from_utf8(&record[..tab])?
            .split_whitespace()
            .collect();
        ensure!(fields.len() == 3, "invalid Git tree metadata");
        // Never turn tree paths into filesystem paths or follow candidate symlinks.
        let path = std::str::from_utf8(&record[tab + 1..])
            .context("non-UTF-8 Git paths cannot be reviewed")?;
        entries.insert(
            path.to_owned(),
            Entry {
                mode: fields[0].into(),
                oid: fields[2].into(),
            },
        );
    }
    Ok(entries)
}

fn content(repo: &Path, entry: Option<&Entry>) -> Result<(String, Option<String>)> {
    let Some(entry) = entry else {
        return Ok((String::new(), None));
    };
    if entry.mode == "160000" {
        return Ok((format!("Submodule commit: {}\n", entry.oid), None));
    }
    let size: usize = String::from_utf8(git(repo, &["cat-file", "-s", &entry.oid])?)?
        .trim()
        .parse()?;
    if size > MAX_BLOB {
        return Ok((
            String::new(),
            Some(format!(
                "File exceeds the 1 MiB display limit ({size} bytes)."
            )),
        ));
    }
    let bytes = git(repo, &["cat-file", "blob", &entry.oid])?;
    if bytes.contains(&0) {
        return Ok((
            String::new(),
            Some("Binary content cannot be displayed as text.".into()),
        ));
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok((text, None)),
        Err(_) => Ok((
            String::new(),
            Some("Non-UTF-8 content cannot be displayed as text.".into()),
        )),
    }
}

fn change(
    path: String,
    old: Option<&Entry>,
    new: Option<&Entry>,
    before: String,
    after: String,
    opaque: Option<String>,
) -> FileChange {
    let diff = TextDiff::configure()
        .timeout(std::time::Duration::from_millis(250))
        .diff_lines(&before, &after);
    let additions = diff
        .iter_all_changes()
        .filter(|line| line.tag() == ChangeTag::Insert)
        .count();
    let deletions = diff
        .iter_all_changes()
        .filter(|line| line.tag() == ChangeTag::Delete)
        .count();
    FileChange {
        path,
        status: match (old, new) {
            (None, _) => "Added",
            (_, None) => "Deleted",
            _ => "Modified",
        }
        .into(),
        old_mode: old.map(|entry| entry.mode.clone()),
        new_mode: new.map(|entry| entry.mode.clone()),
        old_oid: old.map(|entry| entry.oid.clone()),
        new_oid: new.map(|entry| entry.oid.clone()),
        before,
        after,
        opaque,
        additions,
        deletions,
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl Review {
    pub fn from_git(repo: &Path, base: &str, candidate: &str, plan: Option<&Path>) -> Result<Self> {
        let repo = repo.canonicalize().context("opening repository")?;
        let baseline = revision(&repo, base)?;
        let candidate = revision(&repo, candidate)?;
        let before_tree = tree(&repo, &baseline.commit)?;
        let after_tree = tree(&repo, &candidate.commit)?;
        let paths: BTreeSet<_> = before_tree.keys().chain(after_tree.keys()).collect();
        let mut files = Vec::new();
        for path in paths {
            let old = before_tree.get(path);
            let new = after_tree.get(path);
            if old.map(|entry| (&entry.oid, &entry.mode))
                == new.map(|entry| (&entry.oid, &entry.mode))
            {
                continue;
            }
            ensure!(
                files.len() < 4096,
                "candidate exceeds 4096 changed files; review cannot be truncated"
            );
            let (before, old_opaque) = content(&repo, old)?;
            let (after, new_opaque) = content(&repo, new)?;
            files.push(change(
                path.clone(),
                old,
                new,
                before,
                after,
                old_opaque.or(new_opaque),
            ));
        }
        let lock_hash = if let Some(entry) = after_tree.get("flake.lock") {
            ensure!(
                entry.mode == "100644" || entry.mode == "100755",
                "candidate flake.lock must be a regular file"
            );
            Some(digest(&git(&repo, &["cat-file", "blob", &entry.oid])?))
        } else {
            None
        };
        let deployment = if let Some(path) = plan {
            ensure!(fs::metadata(path)?.len() <= MAX_PLAN, "plan exceeds 16 MiB");
            let value: Value =
                serde_json::from_slice(&fs::read(path)?).context("reading plan JSON")?;
            ensure!(value.is_object(), "deployment plan must be a JSON object");
            let plan = value.get("plan").unwrap_or(&value);
            ensure!(plan.is_object(), "plan field must be a JSON object");
            for key in ["changed", "stop", "start"] {
                ensure!(
                    plan.get(key).and_then(Value::as_array).is_some(),
                    "plan must contain a {key} array"
                );
            }
            for key in [
                "changed", "stop", "start", "retire", "restore", "stage", "affected",
            ] {
                if let Some(names) = plan.get(key) {
                    ensure!(
                        names
                            .as_array()
                            .is_some_and(|names| names.iter().all(Value::is_string)),
                        "plan {key} must be an array of strings"
                    );
                }
            }
            for object in [&value, plan] {
                if let Some(revision) = object.get("revision") {
                    ensure!(
                        revision.as_str() == Some(candidate.commit.as_str()),
                        "plan revision differs from candidate commit"
                    );
                }
            }
            if let Some(hash) = value.get("lock_hash") {
                ensure!(
                    hash.as_str() == lock_hash.as_deref(),
                    "plan lock hash differs from candidate flake.lock"
                );
            }
            Some(value)
        } else {
            None
        };
        let mut review = Self {
            schema: "airlock.ctrl-local-review.v1",
            mode: "local",
            repository: Some(repo.to_string_lossy().into_owned()),
            baseline,
            candidate,
            lock_hash,
            files,
            deployment,
            fingerprint: String::new(),
        };
        review.fingerprint = review.calculate_fingerprint()?;
        Ok(review)
    }

    fn calculate_fingerprint(&self) -> Result<String> {
        // Bind all displayed source, blob identities, file modes and plan contents.
        let mut value = serde_json::to_value(self)?;
        value.as_object_mut().unwrap().remove("fingerprint");
        Ok(digest(&serde_json::to_vec(&value)?))
    }

    pub fn approval_blocker(&self) -> Option<&str> {
        if self.files.is_empty() {
            return Some("The baseline and candidate have no source changes.");
        }
        if self.files.iter().any(|file| file.opaque.is_some()) {
            return Some("Some changed files cannot be displayed. Approval is disabled.");
        }
        if self.deployment.is_none() {
            return Some(
                "Source review only. Load a saved deployment plan to enable local approval.",
            );
        }
        None
    }

    pub fn receipt(&self, decision: Decision) -> Result<Value> {
        if matches!(decision, Decision::Approve)
            && let Some(reason) = self.approval_blocker()
        {
            bail!("{reason}");
        }
        Ok(json!({
            "schema": "airlock.ctrl-local-decision.v1",
            "authority": "local-review-only",
            "decision": match decision { Decision::Approve => "approve", Decision::Reject => "reject" },
            "review_fingerprint": self.fingerprint,
            "baseline_revision": self.baseline.commit,
            "candidate_revision": self.candidate.commit,
            "lock_hash": self.lock_hash,
            "deployment": self.deployment,
            "recorded_at_unix": SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        }))
    }

    pub fn record_decision(
        &self,
        decision: Decision,
        directory: &Path,
    ) -> Result<std::path::PathBuf> {
        ensure!(self.mode != "demo", "demo decisions stay in memory");
        let receipt = self.receipt(decision)?;
        fs::create_dir_all(directory).context("creating local decision directory")?;
        let path = directory.join(format!("{}.json", self.fingerprint));
        // Publish a complete private receipt atomically; never replace a previous decision.
        let mut output = tempfile::NamedTempFile::new_in(directory)?;
        let mut data = serde_json::to_vec_pretty(&receipt)?;
        data.push(b'\n');
        output.write_all(&data)?;
        output.as_file().sync_all()?;
        output
            .persist_noclobber(&path)
            .context("creating decision receipt (an existing decision is never overwritten)")?;
        fs::File::open(directory)?.sync_all()?;
        Ok(path)
    }

    pub fn plan(&self) -> Option<&Value> {
        self.deployment
            .as_ref()
            .map(|value| value.get("plan").unwrap_or(value))
    }

    pub fn demo() -> Self {
        let examples = [
            (
                "configuration/domains/usr-home.nix",
                "{ pkgs, ... }: {\n  label = \"green\";\n  netvm = \"sys-firewall\";\n  packages = with pkgs; [\n    firefox\n  ];\n}\n",
                "{ pkgs, ... }: {\n  label = \"green\";\n  netvm = \"sys-firewall\";\n  packages = with pkgs; [\n    firefox\n    keepassxc\n  ];\n}\n",
            ),
            (
                "configuration/domains/sys-ctrl.nix",
                "{\n  guestHostName = \"sys-ctrl\";\n  label = \"black\";\n  prefs = {\n    guivm = null;\n    memory = 8192;\n  };\n}\n",
                "{\n  guestHostName = \"sys-ctrl\";\n  label = \"black\";\n  prefs = {\n    guivm = \"sys-gui\";\n    memory = 8192;\n  };\n}\n",
            ),
            (
                "configuration/policy.nix",
                "{\n  rules = [\n    \"airlock.Ctrl.MachineControl + sys-ctrl dom0 allow\"\n  ];\n}\n",
                "{\n  rules = [\n    \"airlock.Ctrl.MachineControl + sys-ctrl dom0 allow\"\n    \"qubes.OpenURL * usr-home @dispvm ask\"\n  ];\n}\n",
            ),
        ];
        let files = examples
            .into_iter()
            .map(|(path, before, after)| {
                let old = Entry {
                    mode: "100644".into(),
                    oid: digest(before.as_bytes()),
                };
                let new = Entry {
                    mode: "100644".into(),
                    oid: digest(after.as_bytes()),
                };
                change(
                    path.into(),
                    Some(&old),
                    Some(&new),
                    before.into(),
                    after.into(),
                    None,
                )
            })
            .collect();
        let mut review = Self {
            schema: "airlock.ctrl-local-review.v1",
            mode: "demo",
            repository: None,
            baseline: Revision {
                commit: "example-baseline".into(),
                subject: "Current configuration · example".into(),
            },
            candidate: Revision {
                commit: "example-candidate".into(),
                subject: "Add a password manager and controller GUI · example".into(),
            },
            lock_hash: None,
            files,
            deployment: Some(
                json!({"plan": {"changed": ["sys-ctrl", "usr-home"], "stop": ["usr-home", "sys-ctrl"], "start": ["sys-ctrl", "usr-home"], "retire": [], "restore": [], "host_reboot_required": false}, "phase": "prepared", "example": true}),
            ),
            fingerprint: String::new(),
        };
        review.fingerprint = review.calculate_fingerprint().unwrap();
        review
    }
}
