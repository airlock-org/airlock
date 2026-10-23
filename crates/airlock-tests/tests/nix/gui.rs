//! Authoring stays independent of runtime X11 dependencies and preserves literal command arguments.

use super::*;

/// Resolve feature graphs without compiling or provisioning either side of the service boundary.
#[test]
fn authoring_does_not_require_runtime_dependencies() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let tree = |authoring: bool| -> Result<String> {
        let mut command = Command::new("cargo");
        command
            .args(["tree", "--locked", "--offline", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .args([
                "-p",
                "airlock-sys-gui-service",
                "--prefix",
                "none",
                "--format",
                "{p}",
                "--edges",
                "normal",
            ]);
        if authoring {
            command.args(["--no-default-features", "--features", "authoring"]);
        }
        Ok(String::from_utf8(
            common::run(&mut command, None)?.checked()?,
        )?)
    };
    let runtime = tree(false)?;
    let authoring = tree(true)?;
    assert!(!runtime.lines().any(|line| line.starts_with("rusix ")));
    assert!(authoring.lines().any(|line| line.starts_with("rusix ")));
    for dependency in ["x11rb ", "airlock-vm-service "] {
        assert!(!authoring.lines().any(|line| line.starts_with(dependency)));
        assert!(runtime.lines().any(|line| line.starts_with(dependency)));
    }
    Ok(())
}

/// Execute rendered commands and preserve upstream behavior when extending capture build hooks.
#[test]
#[ignore = "requires explicitly prepared offline Nix sources and gio"]
fn rendered_commands_preserve_literal_arguments() -> Result<()> {
    let nix = context()?;
    let temporary = tempfile::tempdir()?;
    let package = temporary.path().join("package");
    fs::create_dir_all(package.join("bin"))?;
    common::peer_link(&package.join("bin"), "airlock-sys-gui-service")?;
    let argument = "/tmp/a \"quote\" \\ slash $dollar `tick` ; %field\nnew\ttab.json";
    let commands = nix
        .evaluate(
            "gui-command-rendering",
            "gui/render-command",
            json!({"package":package,"library":nix.nixpkgs()?.join("lib"),"argument":argument,"apps":nix.prepared.join("crates")}),
        )?
        .value()?;
    for mode in ["shell", "desktop"] {
        let receipt = temporary.path().join(format!("{mode}.json"));
        let mut command = if mode == "shell" {
            let mut command = Command::new("sh");
            command.args(["-c", commands[mode].as_str().unwrap()]);
            command
        } else {
            let desktop = temporary.path().join("arguments.desktop");
            fs::write(
                &desktop,
                format!(
                    r#"[Desktop Entry]
Type=Application
Name=Argument test
Exec={exec}
"#,
                    exec = commands[mode].as_str().unwrap()
                ),
            )?;
            let mut command = Command::new("gio");
            command.arg("launch").arg(desktop);
            command
        };
        nix.environment(&mut command);
        common::run(command.env("AIRLOCK_ARGS_RECEIPT", &receipt), None)?.checked()?;
        common::until(|| Ok(receipt.exists()))?;
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(receipt)?)?,
            json!(["screen-capture", "check-config", "--config", argument])
        );
    }

    let capture = &commands["capture"];
    let patches = capture["patches"].as_array().unwrap();
    assert_eq!(patches[0], "upstream.patch");
    assert!(
        patches
            .last()
            .unwrap()
            .as_str()
            .unwrap()
            .ends_with("-0001-completed-capture-handoff.patch")
    );

    let script = format!(
        r#"
            substituteInPlace() {{ printf '%s\n' substitution; }}
            {hook}
        "#,
        hook = capture["postPatch"].as_str().unwrap(),
    );
    let output = common::run(Command::new("sh").args(["-eu", "-c", &script]), None)?.checked()?;
    assert_eq!(
        String::from_utf8(output)?.lines().collect::<Vec<_>>(),
        ["upstream", "substitution", "substitution"]
    );
    Ok(())
}
