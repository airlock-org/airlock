//! Development-terminal scope guards and terminal restoration use real local PTYs with no dom0 connection.

use super::*;
use crate::common::peers::Peers;
use airlock_tests::{dialogue::Dialogue, process::quote};

/// Return the generated terminal configuration and native client contract from the actual packaged module.
fn configuration(packages: &Packages) -> Result<(Value, Value)> {
    let report = packages.inspect(
        "dev_dom0_terminal/terminal_configuration",
        json!({"root": packages.source.join("airlock-machine")}),
    )?;
    let launcher = report["launcher"].as_str().context("terminal launcher")?;
    let client = launcher
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|words| words[0] == "-e")
        .map(|words| words[1])
        .context("terminal client")?;
    let source = packages.inspect(
        "dev_dom0_terminal/terminal_client",
        json!({"client_path": client}),
    )?;
    Ok((report, source))
}

/// An opted-in development terminal preserves sudo/user authority and rejects wrong callers and service arguments before opening a PTY.
#[test]
#[ignore = "explicit packaged development terminal builds"]
fn package_terminal_keeps_development_and_caller_boundaries() -> Result<()> {
    let packages = Packages::new("terminal-scope")?;
    let (report, _) = configuration(&packages)?;
    for field in ["labHost", "guest", "hybridGuest", "sudoPassword"] {
        assert_eq!(report[field], true, "{field}");
    }
    for field in ["normalHost", "normalHandler", "normalGuest"] {
        assert_eq!(report[field], false, "{field}");
    }
    assert!(
        report["rejected"]
            .as_array()
            .unwrap()
            .iter()
            .any(|message| message
                .as_str()
                .unwrap()
                .contains("dom0 terminal requires a development image"))
    );
    let policy = report["policy"].as_str().unwrap();
    assert!(policy.contains("airlock.DevDom0Shell + sys-gui dom0 allow"));
    assert!(policy.contains("airlock.DevDom0Shell * @anyvm dom0 deny"));
    assert!(!policy.contains("user=root"));
    let mut peers = Peers::new()?;
    peers.rules(
        "failed-pty",
        json!([{"stderr": "simulated PTY failure\n", "exit": 1}]),
    )?;
    let mut handler = report["handler"].clone();
    assert_eq!(handler["args"][0], "desktop");
    assert_eq!(handler["args"][1], "terminal-handler");
    handler["program"] = json!(common::binary(
        "AIRLOCK_VM_SERVICE_BIN",
        "airlock-vm-service"
    ));
    common::contract_argument(
        &mut handler,
        "--logger",
        common::tool("true")?.to_str().unwrap(),
    )?;
    common::contract_argument(&mut handler, "--id", common::tool("id")?.to_str().unwrap())?;
    common::contract_argument(
        &mut handler,
        "--script",
        peers.path("bin/failed-pty").to_str().unwrap(),
    )?;
    for (source, argument) in [("", ""), ("usr-browser", ""), ("sys-gui", "command")] {
        let mut command = common::launcher(&peers.path("bin"), &handler)?;
        peers.environment(&mut command)?;
        let output = common::run(
            command
                .env("QREXEC_REMOTE_DOMAIN", source)
                .env("QREXEC_SERVICE_ARGUMENT", argument),
            None,
        )?;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        assert!(peers.calls()?.is_empty());
    }
    let user = String::from_utf8(common::run(Command::new("id").arg("-un"), None)?.checked()?)?;
    common::contract_argument(&mut handler, "--user", user.trim())?;
    let mut command = common::launcher(&peers.path("bin"), &handler)?;
    peers.environment(&mut command)?;
    let output = common::run(
        command
            .env("QREXEC_REMOTE_DOMAIN", "sys-gui")
            .env("QREXEC_SERVICE_ARGUMENT", ""),
        None,
    )?;
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("starting the dom0 user PTY session")
            && stdout.contains("simulated PTY failure"),
        "{stdout}"
    );
    Ok(())
}

/// Receive a real terminal prompt under a finite deadline, retaining all bytes for secret-echo assertions.
fn receive(dialogue: &mut Dialogue, output: &mut Vec<u8>, marker: &[u8]) -> Result<()> {
    output.extend(
        dialogue
            .until(marker, Duration::from_secs(10))
            .with_context(|| {
                format!("waiting for terminal marker {marker:?}; received {output:?}")
            })?,
    );
    output.extend(marker);
    Ok(())
}

/// Interactive PTYs support password prompts and Ctrl-C and restore the caller's terminal settings on success and connection failure.
#[test]
fn native_terminal_restores_tty_on_success_and_connection_failure() -> Result<()> {
    for failure in [false, true] {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path();
        let relay = common::peer_link(root, "terminal-relay")?;
        let script = common::tool("script")?;
        let bash = common::tool("bash")?;
        fs::write(
            root.join("tools.json"),
            serde_json::to_vec(&json!({"script": script, "bash": bash}))?,
        )?;
        if failure {
            fs::write(root.join("failure"), "")?;
        }
        let client = format!(
            "{service} desktop terminal-client --qrexec {relay}",
            service = quote(
                common::binary("AIRLOCK_VM_SERVICE_BIN", "airlock-vm-service")
                    .to_str()
                    .unwrap()
            ),
            relay = quote(relay.to_str().unwrap()),
        );
        let wrapper = format!(
            r#"printf 'BEFORE:%s\n' "$(stty -g)"
{bash} -c {client}
code=$?
printf 'AFTER:%s\n' "$(stty -g)"
exit "$code"
"#,
            bash = quote(bash.to_str().unwrap()),
            client = quote(&client)
        );
        let shell = format!(
            "{bash} -c {wrapper}",
            bash = quote(bash.to_str().unwrap()),
            wrapper = quote(&wrapper)
        );
        let mut command = Command::new(script);
        command
            .args([
                "--quiet",
                "--return",
                "--flush",
                "--echo",
                "never",
                "--command",
                &shell,
                "/dev/null",
            ])
            .env("AIRLOCK_TEST_TERMINAL_ROOT", root);
        let mut dialogue = Dialogue::spawn(&mut command)?;
        let mut output = Vec::new();
        receive(&mut dialogue, &mut output, b"BEFORE:")?;
        let before = String::from_utf8(dialogue.line(Duration::from_secs(5))?)?
            .trim()
            .to_owned();
        if failure {
            receive(&mut dialogue, &mut output, b"exit code 1")?;
        } else {
            let prompt = b"AIRLOCK_TEST_PROMPT> ";
            receive(&mut dialogue, &mut output, prompt)?;
            dialogue.send(b"test -t 0 && test -t 1 && printf '\\nPTY_OK\\n'; stty size\n")?;
            receive(&mut dialogue, &mut output, b"\r\nPTY_OK\r\n")?;
            receive(&mut dialogue, &mut output, b"30 100")?;
            dialogue
                .send(b"printf '\\n'; read -rs -p 'Password:' token; printf '\\nPROMPT_OK\\n'\n")?;
            receive(&mut dialogue, &mut output, b"\r\nPassword:")?;
            dialogue.send(b"dummy-pty-secret\n")?;
            receive(&mut dialogue, &mut output, b"\r\nPROMPT_OK\r\n")?;
            dialogue.send(b"bash -c 'printf \"\\nSLEEP_READY\\n\"; exec sleep 30'\n")?;
            receive(&mut dialogue, &mut output, b"\r\nSLEEP_READY\r\n")?;
            dialogue.send(b"\x03")?;
            receive(&mut dialogue, &mut output, prompt)?;
            dialogue.send(b"printf '\\nINT_OK\\n'; exit\n")?;
            receive(&mut dialogue, &mut output, b"\r\nINT_OK\r\n")?;
            receive(&mut dialogue, &mut output, b"You can close this window.")?;
        }
        receive(&mut dialogue, &mut output, b"AFTER:")?;
        let after = String::from_utf8(dialogue.line(Duration::from_secs(5))?)?
            .trim()
            .to_owned();
        assert_eq!(before, after);
        assert_eq!(dialogue.finish()?.success(), !failure);
        assert!(!String::from_utf8_lossy(&output).contains("dummy-pty-secret"));
        if failure {
            assert!(String::from_utf8_lossy(&output).contains("simulated connection failure"));
        }
    }
    Ok(())
}
