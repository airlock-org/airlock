//! Packaged Qubes policy retains its upstream parser with Rust-owned authority assertions.

use super::*;

/// Require a textual field from the packaged interface report.
fn text(value: &Value) -> &str {
    value.as_str().expect("interface text")
}

/// Invoke only the upstream Python interface fragment; all queries, temporary data and assertions stay in Rust.
fn upstream(request: Value) -> Result<Value> {
    common::json(
        Command::new(common::binary(
            "AIRLOCK_QUBES_PROBE_BIN",
            "airlock-qubes-probe",
        ))
        .arg("policy")
        .env(
            "PYTHONPATH",
            request["python_path"].as_str().expect("policy Python path"),
        ),
        Some(serde_json::to_vec(&request)?),
    )
}

/// Policy evaluation permits only the declared screen-capture routes.
#[test]
#[ignore = "explicit packaged Qubes interface builds"]
fn package_interfaces_preserve_narrow_policy() -> Result<()> {
    let packages = Packages::new("interfaces")?;
    let report = packages.inspect(
        "live_vm_interfaces/interfaces",
        json!({"root": packages.source.join("airlock-machine")}),
    )?;
    let path = text(&report["qrexecPythonPath"]);
    let physical = packages.physical(path)?;
    let mut queries = Vec::new();
    let mut expected = Vec::new();
    for service in ["airlock.CaptureImport", "airlock.CaptureOpen"] {
        for argument in ["+image", "+video"] {
            queries.push(json!([service, argument, "sys-gui", "usr-home"]));
            expected.push(true);
        }
        for (argument, source, target) in [
            ("+other", "sys-gui", "usr-home"),
            ("+image", "sys-gui", "usr-other"),
            ("+image", "usr-other", "usr-home"),
            ("+image", "sys-gui", "dom0"),
        ] {
            queries.push(json!([service, argument, source, target]));
            expected.push(false);
        }
    }
    assert_eq!(
        upstream(
            json!({"operation": "policy", "python_path": physical, "policy": report["policy"], "domains": ["dom0", "sys-gui", "usr-home", "usr-other"], "queries": queries})
        )?,
        json!(expected)
    );
    Ok(())
}

/// New VM menu sync waits for readiness, clears the inherited caller identity and refuses undeclared control callers.
#[test]
#[ignore = "explicit packaged Qubes interface builds"]
fn package_interface_shells_preserve_session_and_caller_barriers() -> Result<()> {
    let packages = Packages::new("interfaces")?;
    let report = packages.inspect(
        "live_vm_interfaces/interfaces",
        json!({"root": packages.source.join("airlock-machine")}),
    )?;
    assert_eq!(report["sync"]["args"], json!(["sync-appmenus"]));
    assert_eq!(report["sync"]["argument_count"], 1);
    assert!(text(&report["sync"]["program"]).ends_with("/bin/airlock-sys-gui-service"));
    assert!(
        text(&report["policy"])
            .contains("airlock.AppSessionReady + sys-gui @tag:guivm-sys-gui allow")
    );
    assert!(text(&report["policy"]).contains("airlock.AppSessionReady * @anyvm @anyvm deny"));
    assert!(text(&report["receiverUser"]).contains("force-user='user'"));
    let temporary = tempfile::tempdir()?;
    let mut receiver = report["receiver"].clone();
    assert_eq!(receiver["empty_service_argument"], true);
    receiver["program"] = json!(common::tool("true")?);
    receiver["args"] = json!([]);
    assert!(
        !common::run(
            common::launcher(temporary.path(), &receiver)?
                .env("QREXEC_SERVICE_ARGUMENT", "unexpected"),
            None
        )?
        .status
        .success()
    );
    for service in ["airlock.Ctrl.MachinePrepare", "airlock.Pool.AdmitCandidate"] {
        assert!(
            text(&report["controlPolicy"]).contains(&format!("{service} * @anyvm @anyvm deny"))
        );
    }
    for handler in ["prepare", "candidate"] {
        for caller in ["usr-new", "sys-gui", "usr-assistant", ""] {
            assert_eq!(
                common::run(
                    Command::new("bash")
                        .args(["-c", text(&report[handler])])
                        .env("QREXEC_REMOTE_DOMAIN", caller),
                    None
                )?
                .status
                .code(),
                Some(1)
            );
        }
    }
    Ok(())
}
