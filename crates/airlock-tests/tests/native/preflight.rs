//! Inspect a guest's selected store without borrowing workstation programs or executing guest code.

use crate::common::{self, peers::Peers};
use airlock_vm_service::support::Output;
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::PathBuf,
};

/// A tiny logical NixOS system keeps object visibility separate from the physical fixture store.
struct BootView {
    // Inert readelf process, command receipts and all fixture-owned files.
    peers: Peers,
    // Physical stand-in for /nix/store, never the workstation or USB store.
    store: PathBuf,
    // Logical toplevel root used by activation and /run/current-system aliases.
    system: String,
    // Source executable against which renamed native launchers are identified.
    launcher: PathBuf,
}

impl BootView {
    /// Supply only three store objects, a native RPC and an explicitly selected inspection peer.
    fn new() -> Result<Self> {
        let mut peers = Peers::new()?;
        let store = peers.path("store");
        let system = "/nix/store/00000000000000000000000000000000-system".to_owned();
        let system_dir = store.join(system.trim_start_matches("/nix/store/"));
        let tools = store.join("11111111111111111111111111111111-tools");
        let etc = store.join("22222222222222222222222222222222-etc/etc");
        fs::create_dir_all(&system_dir)?;
        fs::create_dir_all(tools.join("bin"))?;
        fs::create_dir_all(etc.join("qubes-rpc"))?;
        fs::create_dir_all(tools.join("lib"))?;
        symlink(
            "/nix/store/22222222222222222222222222222222-etc/etc",
            system_dir.join("etc"),
        )?;
        for name in ["activate", "init"] {
            fs::write(
                system_dir.join(name),
                "#!/nix/store/11111111111111111111111111111111-tools/bin/sh\n",
            )?;
            fs::set_permissions(system_dir.join(name), fs::Permissions::from_mode(0o555))?;
        }
        // The peer supplies ELF metadata; none of these deliberately inert bytes may be executed.
        fs::write(tools.join("bin/sh"), b"\x7fELFinert shell")?;
        fs::set_permissions(tools.join("bin/sh"), fs::Permissions::from_mode(0o555))?;
        let launcher = tools.join("bin/fixed-command");
        fs::copy(
            common::binary("AIRLOCK_EXEC_BIN", "airlock-exec"),
            &launcher,
        )?;
        fs::write(
            tools.join("bin/fixed-command.json"),
            serde_json::to_vec(&json!({
                "program":"/nix/store/11111111111111111111111111111111-tools/bin/sh",
                "inherit_args":false
            }))?,
        )?;
        symlink(
            "/nix/store/11111111111111111111111111111111-tools/bin/fixed-command",
            etc.join("qubes-rpc/airlock.Test"),
        )?;
        fs::write(
            peers.path("paths"),
            format!(
                "{system}\n/nix/store/11111111111111111111111111111111-tools\n/nix/store/22222222222222222222222222222222-etc\n"
            ),
        )?;
        peers.rules("readelf", json!([{}]))?;
        Ok(Self {
            peers,
            store,
            system,
            launcher,
        })
    }

    /// Invoke only the inspection CLI with a deliberately isolated inventory and tool.
    fn check(&self) -> Result<Output> {
        common::run(
            self.peers
                .command(common::binary(
                    "AIRLOCK_PREPARE_BOOT_BIN",
                    "airlock-prepare-boot",
                ))?
                .args(["preflight", "--system", &self.system, "--paths"])
                .arg(self.peers.path("paths"))
                .arg("--store-root")
                .arg(&self.store)
                .arg("--launcher")
                .arg(&self.launcher)
                .arg("--readelf")
                .arg(self.peers.path("bin/readelf"))
                .arg("--report")
                .arg(self.peers.path("report.json")),
            None,
        )
    }
}

/// Copy installation, missing contracts/programs and cross-view symlinks all fail before image publication.
#[test]
fn boot_preflight_checks_installed_contracts_and_exact_store_membership() -> Result<()> {
    for failure in [
        "none",
        "copied",
        "missing-contract",
        "missing-program",
        "missing-resource",
        "cross-view",
        "interpreter",
    ] {
        let fixture = BootView::new()?;
        let tools = fixture
            .store
            .join("11111111111111111111111111111111-tools/bin");
        match failure {
            "copied" => fs::write(
                fixture
                    .store
                    .join("22222222222222222222222222222222-etc/etc/qubes-rpc/airlock.Test.mode"),
                "0555\n",
            )?,
            "missing-contract" => fs::remove_file(tools.join("fixed-command.json"))?,
            "missing-resource" => fs::write(
                tools.join("fixed-command.json"),
                r#"{"program":"/nix/store/11111111111111111111111111111111-tools/bin/sh","args":["/nix/store/11111111111111111111111111111111-tools/absent.json"]}"#,
            )?,
            "missing-program" => fs::write(
                tools.join("fixed-command.json"),
                r#"{"program":"/nix/store/11111111111111111111111111111111-tools/bin/absent"}"#,
            )?,
            "cross-view" => {
                fs::remove_file(tools.join("sh"))?;
                symlink(
                    "/nix/store/33333333333333333333333333333333-other/bin/sh",
                    tools.join("sh"),
                )?;
                let other = fixture
                    .store
                    .join("33333333333333333333333333333333-other/bin");
                fs::create_dir_all(&other)?;
                fs::write(other.join("sh"), b"\x7fELFnot admitted")?;
            }
            "interpreter" => {
                let activate = fixture
                    .store
                    .join(fixture.system.trim_start_matches("/nix/store/"))
                    .join("activate");
                fs::set_permissions(&activate, fs::Permissions::from_mode(0o755))?;
                fs::write(
                    activate,
                    "#!/nix/store/11111111111111111111111111111111-tools/bin/absent\n",
                )?;
            }
            _ => {}
        }
        let output = fixture.check()?;
        assert_eq!(
            output.status.success(),
            failure == "none",
            "{failure}: {error}",
            error = String::from_utf8_lossy(&output.stderr)
        );
        let report: Value = serde_json::from_slice(&fs::read(fixture.peers.path("report.json"))?)?;
        assert_eq!(
            report["errors"].as_array().unwrap().is_empty(),
            failure == "none"
        );
        if failure == "missing-contract" {
            assert!(String::from_utf8_lossy(&output.stderr).contains("fixed-command.json"));
        }
        assert!(
            fixture
                .peers
                .calls()?
                .iter()
                .all(|call| call["command"] == "readelf")
        );
    }
    Ok(())
}

/// ELF dependencies must resolve inside the selected view, including read-only shared libraries.
#[test]
fn boot_preflight_checks_dynamic_library_availability() -> Result<()> {
    for available in [false, true] {
        let mut fixture = BootView::new()?;
        if available {
            let library = fixture
                .store
                .join("11111111111111111111111111111111-tools/lib/libguest.so");
            fs::write(&library, b"\x7fELFinert shared library")?;
            fs::set_permissions(&library, fs::Permissions::from_mode(0o444))?;
        }
        let shell = fixture
            .store
            .join("11111111111111111111111111111111-tools/bin/sh");
        fixture.peers.rules("readelf", json!([
            {"args":["--wide","--program-headers","--dynamic",shell],"stdout":r#"
 0x0000000000000001 (NEEDED) Shared library: [libguest.so]
 0x000000000000001d (RUNPATH) Library runpath: [/nix/store/11111111111111111111111111111111-tools/lib]
"#},
            {}
        ]))?;
        let output = fixture.check()?;
        assert_eq!(
            output.status.success(),
            available,
            "{error}",
            error = String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
