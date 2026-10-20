#![cfg(feature = "evaluation")]

//! Compile the actual example-owned types and check stable rustc codes and
//! causal primary spans, not full wording. Example mains are never evaluated.
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Deserialize)]
struct Case {
    file: String,
    code: Option<String>,
    count: usize,
    source: String,
    reason_tokens: Vec<String>,
}

#[test]
fn documented_compile_fail_cases_fail_for_the_stated_reasons() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../../tests/ui/expected.json")).unwrap();
    let mut registered: Vec<_> = cases.iter().map(|case| case.file.clone()).collect();
    registered.sort();
    let mut fixtures: Vec<_> = fs::read_dir(root.join("tests/ui"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    fixtures.sort();

    assert_eq!(
        registered, fixtures,
        "every UI fixture must have a diagnostic expectation"
    );

    let artifacts = root.join("target/typed-examples/ui");
    fs::create_dir_all(&artifacts).unwrap();

    let metadata = cargo_metadata(&root);
    let scratch = tempfile::tempdir().unwrap();
    let manifest = fixture_manifest(&root, scratch.path(), &cases, &metadata);
    let target_dir = Path::new(metadata["target_directory"].as_str().unwrap());
    let executable = std::env::current_exe().unwrap();
    let relative: Vec<_> = executable
        .strip_prefix(target_dir)
        .unwrap()
        .components()
        .collect();
    assert!(
        matches!(relative.len(), 3 | 4),
        "unexpected Cargo executable path: {}",
        executable.display()
    );
    let profile = relative[relative.len() - 3].as_os_str();

    for case in cases {
        let mut command = Command::new(env!("CARGO"));
        command
            .current_dir(&root)
            .args([
                "check",
                "--locked",
                "--offline",
                "--message-format=json",
                "--color=never",
            ])
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target-dir")
            .arg(target_dir.join("ui-fixtures"))
            .arg("--profile")
            .arg(if profile == "debug" {
                std::ffi::OsStr::new("dev")
            } else {
                profile
            })
            .arg("--bin")
            .arg(Path::new(&case.file).file_stem().unwrap());
        if relative.len() == 4 {
            command.arg("--target").arg(relative[0].as_os_str());
        }
        let output = command.output().unwrap();
        assert!(!output.status.success(), "{} compiled", case.file);

        let messages: Vec<serde_json::Value> = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|event| event["reason"] == "compiler-message")
            .map(|event| event["message"].clone())
            .collect();
        let raw = messages
            .iter()
            .map(|message| serde_json::to_string(message).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(artifacts.join(format!("{}.jsonl", case.file)), &raw).unwrap();
        let raw = format!("{raw}\n{}", String::from_utf8_lossy(&output.stderr));
        let errors: Vec<serde_json::Value> = messages
            .into_iter()
            .filter(|d| {
                d["level"] == "error"
                    && (d["code"]["code"].is_string()
                        || d["spans"]
                            .as_array()
                            .is_some_and(|spans| spans.iter().any(|s| s["is_primary"] == true)))
            })
            .collect();
        assert_eq!(errors.len(), case.count, "{}: {raw}", case.file);

        for error in errors {
            assert_eq!(
                error["code"]["code"].as_str(),
                case.code.as_deref(),
                "{}: {raw}",
                case.file
            );

            let primary: Vec<_> = error["spans"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|s| s["is_primary"] == true)
                .collect();
            let text = primary
                .iter()
                .flat_map(|s| s["text"].as_array().unwrap())
                .filter_map(|t| t["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains(&case.source), "{}: {raw}", case.file);

            let labels = primary
                .iter()
                .filter_map(|s| s["label"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let labels = format!("{labels}\n{}", error["message"].as_str().unwrap_or(""));

            for token in &case.reason_tokens {
                assert!(labels.contains(token), "{}: {raw}", case.file);
            }
        }
    }
}

fn cargo_metadata(root: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO"))
        .current_dir(root)
        .args(["metadata", "--locked", "--offline", "--format-version=1"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

// The scratch package uses the actual fixture files and the workspace's exact
// locked dependency closure. Cargo owns artifact selection and compiler flags.
fn fixture_manifest(
    root: &Path,
    scratch: &Path,
    cases: &[Case],
    metadata: &serde_json::Value,
) -> PathBuf {
    let quoted = |path: &Path| serde_json::to_string(path.to_str().unwrap()).unwrap();
    let mut manifest = format!(
        "[package]\nname = \"rusix-ui-fixtures\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\nresolver = \"3\"\n\n[dependencies]\nrusix = {{ path = {} }}\n",
        quoted(&root.join("crates/rusix")),
    );
    for case in cases {
        manifest.push_str(&format!(
            "\n[[bin]]\nname = {}\npath = {}\n",
            quoted(Path::new(&case.file).file_stem().map(Path::new).unwrap()),
            quoted(&root.join("tests/ui").join(&case.file)),
        ));
    }
    let path = scratch.join("Cargo.toml");
    fs::write(&path, manifest).unwrap();

    let packages = metadata["packages"].as_array().unwrap();
    let nodes = metadata["resolve"]["nodes"].as_array().unwrap();
    let backend = packages
        .iter()
        .find(|package| package["name"] == "rusix")
        .unwrap();
    let mut pending = vec![backend["id"].as_str().unwrap()];
    let mut selected = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if selected.insert(id) {
            let node = nodes.iter().find(|node| node["id"] == id).unwrap();
            for dependency in node["deps"].as_array().unwrap() {
                if dependency["dep_kinds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|kind| kind["kind"] != "dev")
                {
                    pending.push(dependency["pkg"].as_str().unwrap());
                }
            }
        }
    }

    let source = fs::read_to_string(root.join("Cargo.lock")).unwrap();
    let (header, _) = source.split_once("[[package]]").unwrap();
    let mut blocks: Vec<_> = source
        .split("[[package]]")
        .skip(1)
        .filter(|block| {
            packages.iter().any(|package| {
                selected.contains(package["id"].as_str().unwrap())
                    && lock_field(block, "name").as_deref() == package["name"].as_str()
                    && lock_field(block, "version").as_deref() == package["version"].as_str()
                    && lock_field(block, "source").as_deref() == package["source"].as_str()
            })
        })
        .map(str::to_owned)
        .collect();
    blocks.push(
        "\nname = \"rusix-ui-fixtures\"\nversion = \"0.0.0\"\ndependencies = [\n \"rusix\",\n]\n\n"
            .into(),
    );
    blocks.sort_by_key(|block| {
        (
            lock_field(block, "name"),
            lock_field(block, "version"),
            lock_field(block, "source"),
        )
    });
    fs::write(
        scratch.join("Cargo.lock"),
        format!("{header}[[package]]{}", blocks.join("[[package]]")),
    )
    .unwrap();
    path
}

// Cargo's package identity fields are quoted ASCII strings; retain every other
// lockfile field verbatim, including checksums and dependency version qualifiers.
fn lock_field(block: &str, name: &str) -> Option<String> {
    let prefix = format!("{name} = ");
    block
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(|value| serde_json::from_str(value).unwrap())
}
