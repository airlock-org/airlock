//! Prepared source crosses the dom0 boundary as checksum-bound data without executing authoring code.

use crate::common;
use anyhow::Result;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, process::Command};

/// Append an ordinary file with explicit mode and length to a source transport archive.
fn file(archive: &mut tar::Builder<Vec<u8>>, name: &str, content: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive.append_data(&mut header, name, content)?;
    Ok(())
}

/// Source receipts reject changed bytes, revision, mode inventory, links and traversal before publication.
#[test]
fn prepared_source_receiver_publishes_only_the_validated_tree() -> Result<()> {
    for damage in ["", "checksum", "revision", "file", "link", "traversal"] {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path();
        let content = b"// Rust source selected by the validated candidate\n";
        let mut digest = Sha256::new();
        digest.update(0o100644_u32.to_le_bytes());
        digest.update(content);
        let inventory = json!({"src/main.rs": format!("{hash:x}", hash = digest.finalize())});
        let receipt = json!({
            "schema": "airlock.prepared-source.v1", "revision": "a".repeat(40), "dirty": false,
            "artifact_files": inventory,
            "artifact_sha256": format!("{hash:x}", hash = Sha256::digest(serde_json::to_vec(&inventory)?))
        });
        let mut archive = tar::Builder::new(Vec::new());
        let mut body = content.to_vec();
        if damage == "file" {
            body.extend(b"changed");
        }
        file(&mut archive, "src/main.rs", &body)?;
        file(
            &mut archive,
            "airlock-prepared.json",
            &serde_json::to_vec(&receipt)?,
        )?;
        if ["link", "traversal"].contains(&damage) {
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o644);
            header.set_size(0);
            if damage == "link" {
                header.set_path("escape")?;
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_link_name("/tmp")?;
            } else {
                // The normal builder refuses traversal names, so inject the hostile wire header explicitly.
                header.as_mut_bytes()[..9].copy_from_slice(b"../escape");
            }
            header.set_cksum();
            archive.append(&header, std::io::empty())?;
        }
        let payload = archive.into_inner()?;
        let checksum = format!("{hash:x}", hash = Sha256::digest(&payload));
        let path = root.join("source.tar");
        let destination = root.join("received");
        fs::write(&path, payload)?;
        let output = common::run(
            Command::new(common::binary("AIRLOCK_CLI_BIN", "airlock"))
                .args(["artifact", "unpack-source", "--archive"])
                .arg(path)
                .arg("--destination")
                .arg(&destination)
                .args([
                    "--revision",
                    &if damage == "revision" {
                        "b".repeat(40)
                    } else {
                        "a".repeat(40)
                    },
                    "--sha256",
                    &if damage == "checksum" {
                        "0".repeat(64)
                    } else {
                        checksum
                    },
                ])
                .env("PATH", root),
            None,
        )?;
        assert_eq!(
            output.status.success(),
            damage.is_empty(),
            "{damage}: {error}",
            error = String::from_utf8_lossy(&output.stderr)
        );
        if damage.is_empty() {
            assert_eq!(fs::read(destination.join("src/main.rs"))?, content);
        } else {
            assert!(!destination.exists());
        }
        assert!(!root.join("escape").exists());
    }
    Ok(())
}
