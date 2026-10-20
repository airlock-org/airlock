//! Exercise the shipped syntax checker with inert source trees and real command invocations.

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

/// Run the Cargo-built checker against an isolated source tree.
fn run(root: &Path, fix: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_check-rust-spacing"));
    command.arg("--root").arg(root);
    if fix {
        command.arg("--fix");
    }
    command.output().unwrap()
}

/// Distinct Rust item contexts detect, repair and retain idempotent declaration spacing.
#[test]
fn detect_fix_and_idempotence() {
    for source in [
        r###"mod nested {
struct A;
struct B;
}
"###,
        r###"struct A;

impl A {
const N: u8 = 1;
fn f() {}
}
"###,
        r###"trait T {
type X;
fn f();
}
"###,
        r###"unsafe extern "C" {
fn a();
fn b();
}
"###,
        r###"fn f() {
struct A;
struct B;
}
"###,
        r###"struct A; struct B;
"###,
        r###"macro_rules! m { () => {} }
const X: u8 = 1;
"###,
        r###"use a::A;
/// Supported reexport.
pub use b::B;
"###,
        r###"const NAME: &str = "λ";
struct B;
"###,
    ] {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("sample.rs");
        fs::write(&path, source).unwrap();
        let checked = run(temporary.path(), false);
        assert!(!checked.status.success());
        assert!(String::from_utf8_lossy(&checked.stderr).contains("sample.rs:"));
        assert_eq!(fs::read_to_string(&path).unwrap(), source);

        assert!(run(temporary.path(), true).status.success());
        let fixed = fs::read_to_string(&path).unwrap();
        assert!(run(temporary.path(), false).status.success());
        assert!(run(temporary.path(), true).status.success());
        assert_eq!(fs::read_to_string(path).unwrap(), fixed);
    }
}

/// Documentation, nested block comments and attributes stay attached to their declaration.
#[test]
fn attached_comments_and_attributes() {
    for (prefix, expected) in [
        (
            r###"// about B
/// B docs
#[allow(dead_code)]
"###,
            r###"struct A;

// about B
/// B docs
#[allow(dead_code)]
struct B;"###,
        ),
        (
            r###"/* B /* nested */

 docs */
"###,
            r###"struct A;

/* B /* nested */

docs */
struct B;"###,
        ),
    ] {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("comments.rs");
        fs::write(&path, format!("struct A;\n{prefix}struct B;\n")).unwrap();
        assert!(run(temporary.path(), true).status.success());
        assert!(fs::read_to_string(path).unwrap().contains(expected));
    }
}

/// A trailing multiline comment remains with the first item during repair.
#[test]
fn multiline_trailing_comment() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("trailing.rs");
    fs::write(
        &path,
        r###"struct A; /* trailing
 comment */
// about B
struct B;
"###,
    )
    .unwrap();
    assert!(run(temporary.path(), true).status.success());
    assert!(
        fs::read_to_string(path)
            .unwrap()
            .contains("comment */\n\n// about B\nstruct B;")
    );
}

/// Compact imports and fields, statements and opaque macro tokens require no item separator.
#[test]
fn imports_fields_statements_and_opaque_bodies() {
    let temporary = tempfile::tempdir().unwrap();
    fs::write(
        temporary.path().join("compact.rs"),
        r###"use a::A;
use b::B;

struct S {
    /// The initial counter value used by the example.
    count: u8,
    /// Whether the example counter is enabled.
    enabled: bool,
}

fn f() {
    let count = 1;
    let enabled = true;
}

macro_rules! opaque { () => { struct A; struct B; } }
"###,
    )
    .unwrap();
    assert!(run(temporary.path(), false).status.success());
}

/// Generated output, symlinks and nested repositories are outside first-party source checking.
#[test]
fn exclusions_and_nested_repositories() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    fs::write(root.join("kept.rs"), "struct Kept;\n").unwrap();
    for name in [
        "target",
        "vendor",
        "vendored",
        "generated",
        "node_modules",
        "dist",
        "build",
        ".airlock-logs",
        "nested",
        "submodule",
    ] {
        let directory = root.join(name);
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("ignored.rs"), "invalid Rust").unwrap();
    }
    fs::create_dir(root.join("nested/.git")).unwrap();
    fs::write(root.join("submodule/.git"), "gitdir: elsewhere").unwrap();
    fs::write(root.join("generated.rs"), "// @generated\ninvalid Rust").unwrap();
    fs::write(
        root.join("automatic.rs"),
        "// Automatically generated\ninvalid Rust",
    )
    .unwrap();
    std::os::unix::fs::symlink(root.join("target/ignored.rs"), root.join("linked.rs")).unwrap();

    let output = run(root, false);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("1 files checked"));
}

/// A parse failure aborts repair before any valid file can be changed.
#[test]
fn parse_error_does_not_modify_files() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    let good = root.join("a.rs");
    fs::write(&good, "struct A; struct B;\n").unwrap();
    fs::write(root.join("b.rs"), "not Rust!").unwrap();
    let output = run(root, true);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("b.rs:1:"));
    assert_eq!(fs::read_to_string(good).unwrap(), "struct A; struct B;\n");
}

/// Tool invocation from another working directory still checks its own repository.
#[test]
fn default_root_is_independent_of_working_directory() {
    let temporary = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_check-rust-spacing"))
        .current_dir(temporary.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("files checked"));
}
