//! Runtime policy synchronization preserves managed rules while refreshing static links and refusing unmanaged policy files.

use crate::common;
use anyhow::Result;
use std::{fs, process::Command};

/// Refreshing static policy cannot overwrite runtime-managed rules or silently adopt an unmanaged authority file.
#[test]
fn policy_path_refresh_preserves_managed_authority_and_refuses_unmanaged_files() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path();
    let static_dir = root.join("static");
    let runtime = root.join("runtime");
    fs::create_dir_all(static_dir.join("include"))?;
    fs::create_dir(&runtime)?;
    fs::write(
        static_dir.join("10-static.policy"),
        "airlock.Static * dom0 sys-ctrl allow\n",
    )?;
    fs::write(
        static_dir.join("include/example.policy"),
        "airlock.Include * dom0 sys-ctrl allow\n",
    )?;
    fs::write(
        static_dir.join("90-airlock.policy"),
        "ignored static managed-name file\n",
    )?;
    fs::write(runtime.join("old.policy"), "stale\n")?;
    let execute = || -> Result<airlock_vm_service::support::Output> {
        common::run(
            Command::new(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))
                .args(["config", "policy-paths"])
                .arg("--static-dir")
                .arg(&static_dir)
                .arg("--runtime-dir")
                .arg(&runtime)
                .args(["--managed-file", "90-airlock.policy"]),
            None,
        )
    };
    execute()?.checked()?;
    for name in ["10-static.policy", "include"] {
        assert!(runtime.join(name).is_symlink());
        assert_eq!(
            fs::canonicalize(runtime.join(name))?,
            fs::canonicalize(static_dir.join(name))?
        );
    }
    assert!(!runtime.join("old.policy").exists());
    let managed = runtime.join("90-airlock.policy");
    let header = fs::read_to_string(&managed)?;
    assert!(header.contains("# Managed by Airlock setup-config."));
    let authority = format!(
        "{header}airlock.PolicyProbe * dom0 sys-ctrl allow\nairlock.PolicyProbe * @anyvm @anyvm deny\n"
    );
    fs::write(&managed, &authority)?;
    fs::write(static_dir.join("20-static.policy"), "new static\n")?;
    fs::write(runtime.join("stale.policy"), "stale\n")?;
    execute()?.checked()?;
    assert_eq!(fs::read_to_string(&managed)?, authority);
    assert!(runtime.join("20-static.policy").is_symlink());
    assert!(!runtime.join("stale.policy").exists());
    fs::write(&managed, "unmanaged\n")?;
    let failed = execute()?;
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr)
            .contains("refusing unmanaged Airlock qrexec policy file")
    );
    assert_eq!(fs::read_to_string(managed)?, "unmanaged\n");
    Ok(())
}

/// The shipped volume-list predicate accepts both supported Qubes row forms without confusing store-data with store.
#[test]
fn volume_list_matches_the_exact_requested_volume_and_propagates_tool_failure() -> Result<()> {
    for (row, volume, status, expected) in [
        ("pool:appvms/test/store test store No", "store", 0, true),
        ("pool:appvms/test/store test unknown No", "store", 0, true),
        (
            "pool:appvms/test/store-data test store-data No",
            "store-data",
            0,
            true,
        ),
        (
            "pool:appvms/test/store-data test store-data No",
            "store",
            0,
            false,
        ),
        ("pool:appvms/test/root test root No", "store", 0, false),
        ("", "store", 0, false),
        ("pool:appvms/test/store test store No", "store", 1, false),
    ] {
        let mut peers = common::peers::Peers::new()?;
        peers.rules("qvm-volume", serde_json::json!([{"stdout": format!("POOL:VOLUME VMNAME VOLUME_NAME REVERT_POSSIBLE\n{row}\n"), "exit": status}]))?;
        let mut command = peers.command(
            common::binary("AIRLOCK_SMOKE_BIN", "airlock-smoke")
                .to_str()
                .unwrap(),
        )?;
        command.args(["check-volume", "test", volume]);
        assert_eq!(
            common::run(&mut command, None)?.status.success(),
            expected,
            "{row}"
        );
    }
    Ok(())
}
