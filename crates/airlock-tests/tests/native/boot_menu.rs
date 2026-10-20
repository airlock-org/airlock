//! Boot menu regression scenarios operate on a temporary ESP and synthetic retained system roots.

use crate::common::{self, peers::Peers};
use anyhow::Result;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Parse systemd-boot fields while leaving complete file comparisons to mutation checks.
fn fields(path: &Path) -> Result<BTreeMap<String, String>> {
    Ok(fs::read_to_string(path)?
        .lines()
        .filter_map(|line| line.split_once(char::is_whitespace))
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect())
}

/// Capture every configuration file before an operation which must either succeed atomically or refuse.
fn snapshot(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            files.extend(snapshot(&entry.path())?);
        } else if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "conf")
        {
            files.insert(entry.path(), fs::read(entry.path())?);
        }
    }
    Ok(files)
}

/// Native boot-menu invocation and its private ESP/store fixtures.
struct Menu {
    // Owns the ESP, retained source markers and inert profile selection command.
    peers: Peers,
}

impl Menu {
    /// Seed ordinary, recovery, duplicate-generation and unrelated operating-system entries.
    fn new() -> Result<Self> {
        let mut peers = Peers::new()?;
        peers.rules("nix-env", json!([{}]))?;
        fs::create_dir_all(peers.path("esp/loader/entries"))?;
        fs::create_dir(peers.path("store"))?;
        fs::write(
            peers.path("esp/loader/loader.conf"),
            "timeout 10\npreferred xen-base12\ndefault xen-base12\n",
        )?;
        let menu = Self { peers };
        for (name, generation, role) in [
            ("base12", 12, None),
            ("lab12", 12, Some("normal")),
            ("recovery12", 12, Some("recovery")),
            ("lab11", 11, Some("normal")),
            ("lab10a", 10, Some("normal")),
            ("lab10b", 10, Some("normal")),
            ("recovery11", 11, Some("recovery")),
            ("gui12", 12, None),
        ] {
            menu.add(name, generation, role)?;
        }
        fs::write(
            menu.peers.path("esp/loader/entries/other.conf"),
            "title Other OS\nefi /EFI/other.efi\n",
        )?;
        Ok(menu)
    }

    /// Add a generation whose role and committed source identity are separate from its display title.
    fn add(&self, name: &str, generation: u64, role: Option<&str>) -> Result<()> {
        if let Some(role) = role {
            let marker = self.peers.path("store").join(name).join("etc/airlock");
            fs::create_dir_all(&marker)?;
            fs::write(marker.join("boot-role"), format!("{role}\n"))?;
            fs::write(
                marker.join("boot-source.json"),
                serde_json::to_vec(
                    &json!({"revision":format!("{generation:040x}"),"timestamp":1790778600+generation*60}),
                )?,
            )?;
        }
        for xen in [false, true] {
            let prefix = if xen { "xen" } else { "nixos" };
            let image = if xen {
                format!("efi /EFI/nixos/xen-{name}.efi")
            } else {
                format!("options init=/nix/store/{name}/init console=tty0")
            };
            fs::write(
                self.peers
                    .path("esp/loader/entries")
                    .join(format!("{prefix}-{name}.conf")),
                format!(
                    r#"title NixOS
version Generation {generation} test
{image}
sort-key nixos
"#
                ),
            )?;
        }
        Ok(())
    }

    /// Build the actual offline boot menu command; only the test's ESP and store are selected.
    fn command(&self, dry: bool, recovery_profile: bool) -> Result<Command> {
        let mut command = self
            .peers
            .command(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))?;
        command
            .args(["storage", "host", "--offline", "boot-menu", "--esp"])
            .arg(self.peers.path("esp"))
            .arg("--store-root")
            .arg(self.peers.path("store"))
            .arg("--nix-env")
            .arg(self.peers.path("bin/nix-env"));
        if dry {
            command.arg("--dry-run");
        }
        if recovery_profile {
            command
                .arg("--recovery-profile")
                .arg(self.peers.path("profiles/system-profiles/airlock-recovery"));
        }
        Ok(command)
    }

    /// Apply or inspect the native classification and decode its public result.
    fn run(&self, dry: bool, recovery_profile: bool) -> Result<Value> {
        self.peers.json(&mut self.command(dry, recovery_profile)?)
    }
}

/// Normal generations precede recovery, source labels do not affect numeric order and repeat setup is stable.
#[test]
fn boot_menu_orders_generations_and_preserves_explicit_identity() -> Result<()> {
    let menu = Menu::new()?;
    menu.run(false, false)?;
    let current = menu.peers.path("esp/loader/entries/xen-lab12.conf");
    let recovery = menu.peers.path("esp/loader/entries/xen-recovery12.conf");
    assert_eq!(
        fields(&menu.peers.path("esp/loader/loader.conf"))?["default"],
        "xen-lab12.conf"
    );
    assert!(!fields(&current)?.contains_key("version"));
    assert!(fields(&recovery)?["title"].starts_with("Airlock (recovery, "));
    let before = snapshot(&menu.peers.path("esp"))?;
    menu.run(false, false)?;
    assert_eq!(snapshot(&menu.peers.path("esp"))?, before);
    let mut entries = menu.run(true, false)?["entries"]
        .as_array()
        .unwrap()
        .clone();
    entries.sort_by_key(|entry| {
        fields(Path::new(entry["path"].as_str().unwrap())).unwrap()["sort-key"].clone()
    });
    assert_eq!(
        entries
            .iter()
            .map(|entry| (
                entry["generation"].as_u64().unwrap(),
                entry["role"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [
            (12, "normal"),
            (11, "normal"),
            (10, "normal"),
            (12, "recovery"),
            (11, "recovery")
        ]
    );

    for path in [&current, &recovery] {
        let text = fs::read_to_string(path)?
            .lines()
            .filter(|line| !line.starts_with("title ") && !line.starts_with("version "))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, format!("{text}\n"))?;
    }
    menu.run(false, false)?;
    assert_eq!(
        fields(&menu.peers.path("esp/loader/loader.conf"))?["default"],
        "xen-lab12.conf"
    );
    Ok(())
}

/// Missing recovery or invalid source provenance must leave the entire ESP unchanged.
#[test]
fn boot_menu_refuses_incomplete_or_uncommitted_identity_before_mutation() -> Result<()> {
    for damage in [
        "no-recovery",
        "invalid-source",
        "missing-source",
        "retained-generation",
    ] {
        let menu = Menu::new()?;
        match damage {
            "no-recovery" => {
                for name in ["recovery12", "recovery11"] {
                    fs::remove_file(
                        menu.peers
                            .path("store")
                            .join(name)
                            .join("etc/airlock/boot-role"),
                    )?;
                }
            }
            "invalid-source" => fs::write(
                menu.peers.path("store/lab12/etc/airlock/boot-source.json"),
                r#"{"revision":"dirty","timestamp":1790778600}"#,
            )?,
            "missing-source" => {
                fs::remove_file(menu.peers.path("store/lab12/etc/airlock/boot-source.json"))?
            }
            "retained-generation" => fs::write(
                menu.peers.path("esp/loader/entries/airlock-xen-old.conf"),
                r#"version Generation 1
# airlock-system /nix/store/old
# airlock-role normal
"#,
            )?,
            _ => unreachable!(),
        }
        let before = snapshot(&menu.peers.path("esp"))?;
        assert!(
            !common::run(&mut menu.command(false, false)?, None)?
                .status
                .success(),
            "{damage}"
        );
        assert_eq!(snapshot(&menu.peers.path("esp"))?, before, "{damage}");
    }
    Ok(())
}

/// Recovery pinning selects the newest retained source; sealed boot artifacts remain usable without it.
#[test]
fn boot_menu_preserves_retained_recovery_and_default_selection() -> Result<()> {
    let menu = Menu::new()?;
    menu.run(false, true)?;
    assert_eq!(
        menu.peers.calls()?.last().unwrap()["args"],
        json!([
            "--profile",
            menu.peers.path("profiles/system-profiles/airlock-recovery"),
            "--set",
            menu.peers.path("store/recovery12")
        ])
    );
    fs::write(
        menu.peers.path("esp/loader/loader.conf"),
        "timeout 10\ndefault xen-lab11\n",
    )?;
    menu.run(false, false)?;
    assert_eq!(
        fields(&menu.peers.path("esp/loader/loader.conf"))?["default"],
        "xen-lab12.conf"
    );
    let menu = Menu::new()?;
    for name in ["recovery12", "recovery11"] {
        fs::remove_dir_all(menu.peers.path("store").join(name))?;
    }
    let retained = menu
        .peers
        .path("esp/loader/entries/airlock-xen-recovery.conf");
    let contents = r#"title Airlock (recovery)
# airlock-generation 1
efi /EFI/airlock/recovery.efi
# airlock-system /nix/store/recovery1
# airlock-role recovery
"#;
    fs::write(&retained, contents)?;
    menu.run(false, true)?;
    assert_eq!(fs::read_to_string(retained)?, contents);
    Ok(())
}

/// Regenerated native entries do not displace sealed equivalents or their configured default.
#[test]
fn boot_menu_keeps_sealed_choices_and_orders_equal_source_labels_numerically() -> Result<()> {
    let menu = Menu::new()?;
    let mut sealed = Vec::new();
    for (name, generation, role) in [
        ("lab12", 12, "normal"),
        ("lab11", 11, "normal"),
        ("recovery12", 12, "recovery"),
    ] {
        let native = menu
            .peers
            .path("esp/loader/entries")
            .join(format!("xen-{name}.conf"));
        let target = menu
            .peers
            .path("esp/loader/entries")
            .join(format!("airlock-xen-{name}.conf"));
        let text = format!(
            r#"title Airlock
# airlock-generation {generation}
efi /EFI/airlock/{name}.efi
# airlock-system /nix/store/{name}
# airlock-role {role}
"#
        );
        fs::write(&target, &text)?;
        sealed.push((native, target, text));
    }
    menu.run(false, false)?;
    for (native, target, text) in sealed {
        assert!(!native.exists());
        assert_eq!(fs::read_to_string(target)?, text);
    }
    assert_eq!(
        fields(&menu.peers.path("esp/loader/loader.conf"))?["default"],
        "airlock-xen-lab12.conf"
    );

    let menu = Menu::new()?;
    menu.add("two", 2, Some("normal"))?;
    for entry in fs::read_dir(menu.peers.path("store"))? {
        let marker = entry?.path().join("etc/airlock/boot-source.json");
        if marker.exists() {
            fs::write(
                marker,
                serde_json::to_vec(
                    &json!({"revision":format!("abcdef0{rest}",rest="1".repeat(33)),"timestamp":1790778600}),
                )?,
            )?;
        }
    }
    menu.run(false, false)?;
    let report = menu.run(true, false)?;
    let mut entries = report["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["role"] == "normal")
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| {
        fields(Path::new(entry["path"].as_str().unwrap())).unwrap()["sort-key"].clone()
    });
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry["generation"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [12, 11, 10, 2]
    );
    Ok(())
}

/// The shipped post-install hook repairs duplicate recovery labels without modifying other entry fields.
#[test]
fn boot_label_hook_is_idempotent_and_handles_empty_entry_directories() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let root = temporary.path();
    let entries = root.join("loader/entries");
    fs::create_dir_all(&entries)?;
    let recovery = " (Recovery: without Xen)";
    let files = [
        (
            "xen-old.conf",
            format!(
                "title NixOS (usb-lab){recovery}{recovery} (with Xen Hypervisor)\nversion Generation 1\nefi /EFI/nixos/old.efi\n"
            ),
        ),
        (
            "xen-new.conf",
            "title NixOS (with Xen Hypervisor)\nversion Generation 3\n".into(),
        ),
        (
            "nixos-old.conf",
            format!(
                "title NixOS (usb-lab){recovery}{recovery}\nlinux /EFI/kernel\noptions init=/nix/store/system/init\n"
            ),
        ),
        (
            "nixos-new.conf",
            "title NixOS\nversion Generation 3\n".into(),
        ),
        ("other.conf", "title Other OS\nefi /EFI/other.efi\n".into()),
    ];
    for (name, text) in &files {
        fs::write(entries.join(name), text)?;
    }
    let label = || {
        let mut command = Command::new(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"));
        command
            .args(["storage", "host", "--offline", "boot-labels", "--esp"])
            .arg(root);
        command
    };
    for _ in 0..2 {
        common::run(&mut label(), None)?.checked()?;
        for (name, original) in &files {
            let (title, rest) = original.split_once('\n').unwrap();
            let mut title = title.to_owned();
            if name.starts_with("xen-") || name.starts_with("nixos-") {
                title = title.replace(recovery, "");
            }
            if name.starts_with("nixos-") {
                title.push_str(recovery);
            }
            assert_eq!(
                fs::read_to_string(entries.join(name))?,
                format!("{title}\n{rest}")
            );
        }
    }
    for entry in fs::read_dir(entries)? {
        fs::remove_file(entry?.path())?;
    }
    common::run(&mut label(), None)?.checked()?;
    Ok(())
}
