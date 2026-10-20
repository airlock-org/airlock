#![cfg(feature = "evaluation")]

//! Verify ordinary and renamed dependencies from a separate user's library.
use std::{fs, path::Path, process::Command};

#[test]
fn downstream_libraries_can_use_reexported_macros_and_explicit_sources() {
    let package = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = package.join("../..").canonicalize().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let source = include_str!("consumers/library.rs.txt");
    let quoted_package = serde_json::to_string(package.to_str().unwrap()).unwrap();

    for name in ["rusix", "renamed_rusix"] {
        let root = scratch.path().join(name);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"rusix-consumer\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n\n[dependencies]\n{name} = {{ package = \"rusix\", path = {quoted_package} }}\n",
            ),
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            source.replace("rusix::", &format!("{name}::")),
        )
        .unwrap();

        let lock = Command::new(env!("CARGO"))
            .current_dir(&root)
            .args(["generate-lockfile", "--offline"])
            .output()
            .unwrap();
        assert!(
            lock.status.success(),
            "{}",
            String::from_utf8_lossy(&lock.stderr)
        );

        let output = Command::new(env!("CARGO"))
            .current_dir(&root)
            .args(["test", "--locked", "--offline"])
            .arg("--target-dir")
            .arg(repository.join("target/consumer-fixtures"))
            .env("RUSIX_TEST_NIXPKGS", repository.join("vendor/nixpkgs"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "consumer {name}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}
