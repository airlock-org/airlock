//! Explicit shared-closure generation, isolation, retention and rollback scenarios through upstream adapters.

use super::{
    common::{self, Session},
    config,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

/// One observed native boot identity; source paths remain distinct from runtime VM state.
struct Boot {
    /// Declared immutable system generation expected inside the VM.
    system: String,
    /// Immutable root image descriptor selected for the VM's writable root import.
    root: String,
    /// Source closure descriptor used to materialize a filtered view.
    store: String,
    /// Materialized EROFS metadata image.
    view: String,
    /// Canonical object mapping for this exact generation's filtered view.
    map: String,
    /// Published retained pool generation identifier.
    generation: u64,
    /// Expected metadata block device in the guest.
    device: String,
    /// Expected filtered external-data block device in the guest.
    blob: String,
    /// Selected shared-view format, currently EROFS.
    filesystem: String,
}

/// Map a logical artifact through the explicitly selected private workstation store when present.
fn physical(source: &str) -> PathBuf {
    if std::env::var("AIRLOCK_USE_HOST_NIX_STORE").as_deref() != Ok("1")
        && source.starts_with("/nix/store/")
        && let Some(root) =
            std::env::var_os("AIRLOCK_NIX_ROOT").filter(|root| !root.is_empty() && root != "/")
    {
        return PathBuf::from(root).join(source.trim_start_matches('/'));
    }
    source.into()
}

/// Read one required Qubes feature; empty metadata never becomes an accepted identity.
fn feature(session: &mut Session, guest: &str, key: &str) -> Result<String> {
    let value = session.command(
        "qvm-features",
        &[guest, key],
        &format!("nix-closure-domain-{guest}-feature.log"),
    )?;
    ensure!(!value.is_empty(), "missing {key} feature for {guest}");
    Ok(value)
}

impl Boot {
    /// Check every physical source and supported layout before probing or switching a generation.
    fn observe(session: &mut Session, guest: &str) -> Result<Self> {
        let boot = Self {
            system: feature(session, guest, "airlock-system-source")?,
            root: feature(session, guest, "airlock-root-source")?,
            store: feature(session, guest, "airlock-store-source")?,
            view: feature(session, guest, "airlock-store-view-image")?,
            map: feature(session, guest, "airlock-store-blob-map")?,
            generation: feature(session, guest, "airlock-store-generation")?.parse()?,
            device: feature(session, guest, "airlock-store-device")?,
            blob: feature(session, guest, "airlock-store-blob-device")?,
            filesystem: feature(session, guest, "airlock-store-filesystem")?,
        };
        ensure!(
            feature(session, guest, "airlock-store-mode")? == "shared-closure"
                && feature(session, guest, "airlock-store-view-backend")? == "erofs-objectmap"
                && boot.filesystem == "erofs",
            "unsupported shared-store layout for {guest}"
        );
        ensure!(
            physical(&boot.root).is_file()
                && physical(&boot.store).is_dir()
                && physical(&boot.store).join("store-paths").is_file()
                && physical(&boot.store).join("manifest.json").is_file()
                && physical(&boot.view).is_file()
                && physical(&boot.map).is_file(),
            "native boot artifacts are missing for {guest}"
        );
        Ok(boot)
    }

    /// Require the existing runtime service to report this exact immutable system and store layout.
    fn runtime(&self, session: &mut Session, guest: &str, output: &str) -> Result<()> {
        let text = session.command(
            "qrexec-client",
            &[
                "-d",
                guest,
                "-T",
                "-w",
                "120",
                "DEFAULT:QUBESRPC airlock.NixStoreRuntimeCheck+ dom0",
            ],
            output,
        )?;
        let fields = common::fields(&text);
        for (key, expected) in [
            ("system", self.system.as_str()),
            ("store_device", self.device.as_str()),
            ("store_format", self.filesystem.as_str()),
            ("store_blob_device", self.blob.as_str()),
        ] {
            ensure!(
                fields.get(key).map(String::as_str) == Some(expected),
                "runtime service did not report expected {key} for {guest}"
            );
        }
        ensure!(
            fields
                .get("store_options")
                .is_some_and(|options| options.split(',').any(|option| option == "ro")),
            "runtime service did not report a read-only store for {guest}"
        );
        Ok(())
    }
}

/// Wait for a responding agent under the existing configurable finite deadline.
fn ready(session: &mut Session, guest: &str, output: &str) -> Result<()> {
    let seconds: u64 = common::setting("AIRLOCK_NIX_CLOSURE_WAIT_SECONDS", "240").parse()?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(seconds))
        .context("qrexec wait too large")?;
    while Instant::now() < deadline {
        let timeout = Duration::from_secs(
            common::setting("AIRLOCK_NIX_CLOSURE_QREXEC_TIMEOUT_SECONDS", "45").parse()?,
        )
        .min(deadline.saturating_duration_since(Instant::now()));
        if session
            .remote_for(guest, ("uname", &["-a"]), None, output, timeout)
            .is_ok_and(|reply| reply.contains("Linux"))
        {
            return Ok(());
        }
        thread::sleep(
            Duration::from_secs(3).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
    anyhow::bail!("{guest} qrexec did not become ready within {seconds}s")
}

/// Stop only an existing running target, retaining the original upstream shutdown behavior.
fn shutdown(session: &mut Session, guest: &str, output: &str) -> Result<()> {
    if session
        .command("qvm-check", &[guest], "nix-closure-domain-exists.log")
        .is_ok()
        && session
            .command(
                "qvm-check",
                &["--running", guest],
                "nix-closure-domain-power.log",
            )
            .is_ok()
    {
        session.command("qvm-shutdown", &["--wait", guest], output)?;
    }
    Ok(())
}

/// Build a native test declaration from an existing VM's immutable artifacts, without inventing sources.
fn declaration(session: &mut Session, source: &str, label: &str) -> Result<Value> {
    let system = feature(session, source, "airlock-system-source")?;
    let root = feature(session, source, "airlock-root-source")?;
    let store = feature(session, source, "airlock-store-source")?;
    let role = feature(session, source, "airlock-role")?;
    let kernel = session.command(
        "qvm-prefs",
        &[source, "kernel"],
        "nix-closure-domain-source-kernel.log",
    )?;
    ensure!(
        !kernel.is_empty(),
        "source domain {source} has no kernel preference"
    );
    Ok(
        json!({"class":"StandaloneVM","label":label,"netvm":null,"template":null,"prefs":{"autostart":false,"default_user":common::setting("AIRLOCK_QUBE_USER","user"),"include_in_backups":false,"kernel":kernel,"memory":1024,"maxmem":1024,"qrexec_timeout":180,"vcpus":1,"virt_mode":"pvh"},"features":{"airlock-backend":"nix-closure","airlock-role":role,"airlock-test":"nix-closure-domain","os":"Linux","qrexec":"1"},"services":{"network-manager":false},"backend":{"type":"nix-closure","template":null,"system":{"source":system,"source_ref":null},"kernel":{"name":kernel,"source":null,"source_ref":null},"initrd":{"name":null,"source":null,"source_ref":null},"modules":{"name":null,"source":null,"source_ref":null},"root":{"mode":"qubes-root-image","source":root,"source_ref":null,"read_only":false},"store":{"mode":"shared-closure","source":store,"source_ref":null,"read_only":true,"device":"/dev/xvde","blob_device":"/dev/xvdf","blobDevice":"/dev/xvdf","mountpoint":"/nix/store","view_backend":"erofs-objectmap","viewBackend":"erofs-objectmap"},"state":{"persistent":null,"volatile":null}}}),
    )
}

/// Apply one of the existing direct/config-flow test generations and retain its compatible receipt.
fn apply(
    session: &mut Session,
    domain: &str,
    peer: &str,
    second: bool,
    mode: &str,
    staging: &str,
    message: &str,
) -> Result<Value> {
    let output = if second {
        "nix-closure-domain-config-apply-gen2.json"
    } else {
        "nix-closure-domain-config-apply.json"
    };
    if mode == "direct" {
        let primary = common::setting(
            if second {
                "AIRLOCK_NIX_CLOSURE_DIRECT_GEN2_SOURCE_VM"
            } else {
                "AIRLOCK_NIX_CLOSURE_DIRECT_PRIMARY_SOURCE_VM"
            },
            if second {
                "sys-config-vault"
            } else {
                "sys-config-staging"
            },
        );
        let peer_source = common::setting(
            "AIRLOCK_NIX_CLOSURE_DIRECT_PEER_SOURCE_VM",
            "sys-config-validator",
        );
        let mut topology = json!({"schema":"airlock.qubes-topology.v1","prefs":{},"templates":{},"kernels":{},"domains":{},"qrexec_policy":{"rules":[]}});
        topology["domains"][domain] = declaration(session, &primary, "green")?;
        topology["domains"][peer] = declaration(session, &peer_source, "blue")?;
        topology["qrexec_policy"]["rules"] = json!([{"service":"airlock.NixStoreRuntimeCheck","argument":"*","source":"dom0","target":domain,"action":"allow"},{"service":"airlock.NixStoreRuntimeCheck","argument":"*","source":"dom0","target":peer,"action":"allow"},{"service":"airlock.NixStoreRuntimeCheck","argument":"*","source":"@anyvm","target":"@anyvm","action":"deny"}]);
        let path = session.file(if second {
            "nix-closure-domain-direct-topology-gen2.json"
        } else {
            "nix-closure-domain-direct-topology.json"
        });
        common::save(&path, &topology)?;
        let setup = common::setting("AIRLOCK_SETUP_CONFIG_COMMAND", "airlock-setup-config");
        session.run(
            Command::new(setup)
                .arg("--apply")
                .arg("--topology")
                .arg(&path),
            None,
            if second {
                "nix-closure-domain-apply-gen2.log"
            } else {
                "nix-closure-domain-apply.log"
            },
        )?;
        let setup: Value = serde_json::from_slice(&fs::read(session.file("setup-config.json"))?)?;
        ensure!(
            setup["status"] == "passed",
            "direct native setup did not pass"
        );
        let record = json!({"schema":"airlock.config-apply.v1","status":"passed","reason":"direct nix-closure topology setup","topology":path,"setup":setup,"nix_domain_materializations_result":{"domains":[{"domain":domain},{"domain":peer}]}});
        common::save(&session.file(output), &record)?;
        Ok(record)
    } else {
        let user = common::setting("AIRLOCK_QUBE_USER", "user");
        let repo = format!("/home/{user}/airlock-config");
        if second {
            let mut inputs: Value = serde_json::from_str(&session.remote(
                staging,
                "cat",
                &[&format!("{repo}/smoke-inputs.json")],
                None,
                "nix-closure-domain-gen2-inputs.log",
            )?)?;
            inputs["message"] = json!(message);
            session.remote(
                staging,
                "tee",
                &[&format!("{repo}/smoke-inputs.json")],
                Some(serde_json::to_vec(&inputs)?),
                "nix-closure-domain-staging-update-gen2.log",
            )?;
            session.remote(
                staging,
                "git",
                &["-C", &repo, "add", "src", "smoke-inputs.json"],
                None,
                "nix-closure-domain-gen2-add.log",
            )?;
            session.remote(
                staging,
                "git",
                &[
                    "-C",
                    &repo,
                    "-c",
                    "user.name=Airlock Closure Smoke",
                    "-c",
                    "user.email=airlock-closure-smoke@localhost",
                    "commit",
                    "-m",
                    "update nix closure smoke domain generation",
                ],
                None,
                "nix-closure-domain-staging-update-gen2.log",
            )?;
        } else {
            config::stage(
                session,
                staging,
                &json!({"created":domain,"peer":peer,"message":message,"peerMessage":message,"action":"switch"}),
                "add nix closure smoke domain",
            )?;
        }
        session.run(
            Command::new("airlock-config-apply").arg("--apply").env(
                "AIRLOCK_CONFIG_APPLY_NIX_OFFLINE",
                common::setting("AIRLOCK_CONFIG_APPLY_NIX_OFFLINE", "0"),
            ),
            None,
            if second {
                "nix-closure-domain-apply-gen2.log"
            } else {
                "nix-closure-domain-apply.log"
            },
        )?;
        let record: Value = serde_json::from_slice(&fs::read(session.file("config-apply.json"))?)?;
        common::save(&session.file(output), &record)?;
        Ok(record)
    }
}

/// Verify Xen attaches only filtered external-data devices for both view components.
fn libvirt(session: &mut Session, domain: &str, output: &str) -> Result<()> {
    let xml = match session.command("virsh", &["-c", "xen:///", "dumpxml", domain], output) {
        Ok(xml) => xml,
        Err(_) => session.command(
            "virsh",
            &["-c", "xen:///", "dumpxml", &format!("domain-{domain}")],
            output,
        )?,
    };
    let mut reader = quick_xml::Reader::from_str(&xml);
    let mut targets = std::collections::BTreeSet::new();
    loop {
        match reader.read_event()? {
            quick_xml::events::Event::Start(element) | quick_xml::events::Event::Empty(element)
                if element.name().as_ref() == b"target" =>
            {
                for attribute in element.attributes() {
                    let attribute = attribute?;
                    if attribute.key.as_ref() == b"dev" {
                        targets.insert(String::from_utf8(attribute.value.into_owned())?);
                    }
                }
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    ensure!(
        targets.contains("xvde")
            && targets.contains("xvdf")
            && xml.contains("/dev/mapper/airlock-")
            && !xml.contains("/var/lib/airlock/closure-store/objects/segments/"),
        "libvirt XML exposes incorrect or unfiltered store backing devices for {domain}"
    );
    Ok(())
}

/// Inspect the immutable root image and extract the filtered EROFS view, always detaching after inspection.
fn images(session: &mut Session, domain: &str, boot: &Boot) -> Result<()> {
    let listing = session.command(
        "debugfs",
        &[
            "-R",
            "ls -p /nix/store",
            physical(&boot.root).to_str().context("root image path")?,
        ],
        "nix-closure-domain-root-image-check.log",
    )?;
    ensure!(
        !listing.lines().any(|line| {
            let parts = line.split('/').collect::<Vec<_>>();
            parts.len() >= 6 && ![".", ".."].contains(&parts[5])
        }),
        "root image contains unexpected /nix/store entries"
    );
    ensure!(
        fs::read_to_string(physical(&boot.store).join("store-paths"))?
            .lines()
            .any(|source| source == boot.system),
        "store closure omits the selected system"
    );
    let generation = boot.generation.to_string();
    let attach = session.command(
        "airlock-dom0-service",
        &[
            "storage",
            "pool",
            "attach-blob-view",
            "--domain",
            domain,
            "--generation",
            &generation,
        ],
        "nix-closure-domain-store-attach.json",
    )?;
    let result: Result<()> = (|| {
        let attach: Value = serde_json::from_str(&attach)?;
        let device = attach["device"]
            .as_str()
            .filter(|device| device.starts_with("/dev/mapper/airlock-"))
            .context("unexpected filtered device")?;
        let resolved = fs::canonicalize(device)?;
        for backing in attach["backing_paths"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            ensure!(
                fs::canonicalize(backing)? != resolved,
                "filtered device resolves to canonical segment backing storage"
            );
        }
        let temporary = tempfile::Builder::new()
            .prefix("nix-closure-domain-store-extract-")
            .tempdir_in(&session.directory)?;
        session.command(
            "fsck.erofs",
            &[
                &format!("--device={device}"),
                &format!(
                    "--extract={directory}",
                    directory = temporary.path().display()
                ),
                physical(&boot.view)
                    .to_str()
                    .context("store view image path")?,
            ],
            "nix-closure-domain-store-view-check.log",
        )?;
        let relative = boot
            .system
            .strip_prefix("/nix/store/")
            .context("system source is not in /nix/store")?;
        ensure!(
            temporary
                .path()
                .join(relative)
                .join("init")
                .metadata()?
                .permissions()
                .mode()
                & 0o111
                != 0,
            "store view does not contain executable selected system init"
        );
        Ok(())
    })();
    let detach = session.command(
        "airlock-dom0-service",
        &[
            "storage",
            "pool",
            "detach-blob-view",
            "--domain",
            domain,
            "--generation",
            &generation,
        ],
        "nix-closure-domain-store-detach.log",
    );
    result?;
    detach?;
    Ok(())
}

/// Run the guest boot/persistence/service probes using individual upstream programs, not a generated shell script.
fn guest(
    session: &mut Session,
    domain: &str,
    boot: &Boot,
    role: &str,
    message: Option<&str>,
    output: &str,
) -> Result<()> {
    ready(session, domain, "nix-closure-domain-qrexec.log")?;
    let cmdline = session.remote(
        domain,
        "cat",
        &["/proc/cmdline"],
        None,
        "nix-closure-domain-cmdline.log",
    )?;
    let options = cmdline
        .split_whitespace()
        .filter_map(|word| word.split_once('='))
        .collect::<BTreeMap<_, _>>();
    let observed = session.remote(
        domain,
        "readlink",
        &["-f", "/run/current-system"],
        None,
        "nix-closure-domain-current-system.log",
    )?;
    ensure!(
        options.get("airlock.system") == Some(&boot.system.as_str())
            && observed == boot.system
            && options.get("airlock.storedev") == Some(&boot.device.as_str())
            && options.get("airlock.storefmt").copied().unwrap_or("erofs") == boot.filesystem
            && options.get("airlock.storeblobdev") == Some(&boot.blob.as_str()),
        "guest boot identity differs from the selected native generation"
    );
    session.remote(
        domain,
        "test",
        &["-x", &format!("{system}/init", system = boot.system)],
        None,
        "nix-closure-domain-guest-init.log",
    )?;
    session.remote(
        domain,
        "test",
        &["-b", &boot.blob],
        None,
        "nix-closure-domain-blob-device.log",
    )?;
    let block = boot
        .blob
        .strip_prefix("/dev/")
        .context("store blob device is not under /dev")?;
    ensure!(
        session.remote(
            domain,
            "cat",
            &[&format!("/sys/class/block/{block}/ro")],
            None,
            "nix-closure-domain-blob-readonly.log"
        )? == "1",
        "guest blob block device is writable"
    );
    session.runtime_store(domain, output)?;
    for service in [
        "qubes-db.service",
        "qubes-qrexec-agent.service",
        "airlock-qubes-rw-init.service",
    ] {
        session.remote(
            domain,
            "systemctl",
            &["is-active", service],
            None,
            "nix-closure-domain-guest-services.log",
        )?;
    }
    let user = common::setting("AIRLOCK_QUBE_USER", "user");
    let home = format!("/home/{user}");
    for directory in [
        "/rw",
        "/rw/home",
        "/usr/local",
        home.as_str(),
        "/run/qubes",
        "/var/log/qubes",
    ] {
        session.remote(
            domain,
            "test",
            &["-d", directory],
            None,
            "nix-closure-domain-guest-directories.log",
        )?;
    }
    session.remote(
        domain,
        "test",
        &["-w", &home],
        None,
        "nix-closure-domain-guest-home.log",
    )?;
    session.remote(
        domain,
        "touch",
        &[&format!("{home}/airlock-closure-smoke-home")],
        None,
        "nix-closure-domain-guest-home.log",
    )?;
    let observed_role = session
        .remote(
            domain,
            "cat",
            &["/etc/airlock/domain-role"],
            None,
            "nix-closure-domain-role.log",
        )
        .unwrap_or_default();
    ensure!(
        role.is_empty() || observed_role == role,
        "unexpected guest domain role: {observed_role}"
    );
    let applied = session
        .remote(
            domain,
            "cat",
            &["/etc/airlock/applied-message"],
            None,
            "nix-closure-domain-applied-message.log",
        )
        .unwrap_or_default();
    if let Some(message) = message {
        ensure!(
            applied == message,
            "guest did not boot the expected applied message"
        );
    }
    fs::write(
        session.file(output),
        format!(
            "cmdline={cmdline}\nsystem_path={system}\nstore_device={device}\nstore_format={filesystem}\nstore_blob_device={blob}\ncurrent_system={observed}\nguest_user={user}\nhome_dir={home}\ndomain_role={observed_role}\napplied_message={applied}\nruntime_probe=passed\n",
            system = boot.system,
            device = boot.device,
            filesystem = boot.filesystem,
            blob = boot.blob
        ),
    )?;
    Ok(())
}

/// Check complementary object visibility through each guest's already filtered store mount.
fn visibility(
    session: &mut Session,
    domain: &str,
    own: &str,
    other: &str,
    shared: &str,
    output: &str,
) -> Result<()> {
    let reply = session.remote(
        domain,
        "airlock-vm-service",
        &["runtime", "store-visibility", own, other, shared],
        None,
        output,
    )?;
    ensure!(
        reply.lines().any(|line| line == "visibility_probe=passed"),
        "guest did not complete the visibility probe"
    );
    Ok(())
}

/// Require one JSON field rather than accepting null as an identity or a zero count.
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .context(format!("missing {key} in shared-store report"))
}

/// Invoke the existing canonical sharing report implementation used by offline tests too.
fn sharing(
    session: &mut Session,
    domain: &str,
    peer: &str,
    boots: (&Boot, &Boot),
    second: bool,
) -> Result<Value> {
    let label = if second { "gen2" } else { "initial" };
    let output = session.file(if second {
        "nix-closure-domain-sharing-proof-gen2.json"
    } else {
        "nix-closure-domain-sharing-proof.json"
    });
    session.run(
        Command::new("airlock-test-report")
            .arg("sharing")
            .arg(label)
            .arg(domain)
            .arg(peer)
            .arg(physical(&boots.0.map))
            .arg(physical(&boots.1.map))
            .arg(&output),
        None,
        "nix-closure-domain-sharing.log",
    )?;
    Ok(serde_json::from_slice(&fs::read(output)?)?)
}

/// Read one pool JSON result while retaining diagnostics separately from its parsed receipt.
fn pool(session: &mut Session, args: &[&str], output: &str) -> Result<Value> {
    let stdout = session.run(
        Command::new("airlock-dom0-service")
            .args(["storage", "pool"])
            .args(args),
        None,
        &format!("{output}.log"),
    )?;
    let value = serde_json::from_str(&stdout)?;
    common::save(&session.file(output), &value)?;
    Ok(value)
}

/// Check exact generation membership in one pool report field.
fn contains(value: &Value, key: &str, domain: &str, generation: u64) -> bool {
    value[key].as_array().is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| entry["domain"] == domain && entry["generation"] == generation)
    })
}

/// Restore the boot metadata returned by the existing stopped-domain generation switch adapter.
fn restore(session: &mut Session, domain: &str, generation: u64) -> Result<()> {
    let stdout = session.command(
        "airlock-dom0-service",
        &[
            "storage",
            "domain",
            "switch-store",
            domain,
            &generation.to_string(),
        ],
        "nix-closure-domain-rollback-switch.stdout",
    )?;
    fs::write(
        session.file("nix-closure-domain-rollback-switch.stdout"),
        stdout,
    )?;
    session.run(
        Command::new("airlock-test-report")
            .arg("extract")
            .arg("--any")
            .arg(session.file("nix-closure-domain-rollback-switch.stdout"))
            .arg(session.file("nix-closure-domain-rollback-switch.json")),
        None,
        "nix-closure-domain-rollback-extract.log",
    )?;
    let selected: Value = serde_json::from_slice(&fs::read(
        session.file("nix-closure-domain-rollback-switch.json"),
    )?)?;
    ensure!(
        selected["generation"] == generation,
        "switch selected a different retained generation"
    );
    let get = |key: &str, default: &str| selected[key].as_str().unwrap_or(default).to_owned();
    let system = text(&selected, "system")?;
    let root = get("root_source", "");
    if !root.is_empty() {
        let root = physical(&root);
        let root = if root.is_dir() {
            root.join("root.img")
        } else {
            root
        };
        ensure!(root.is_file(), "rollback root image is missing");
        session.run(
            Command::new("qvm-volume")
                .args(["import", &format!("{domain}:root")])
                .arg(root),
            None,
            "nix-closure-domain-rollback-root.log",
        )?;
    }
    let kernel = get("kernel_name", "");
    if !kernel.is_empty() {
        session.command(
            "qvm-prefs",
            &[domain, "kernel", &kernel],
            "nix-closure-domain-rollback-kernel.log",
        )?;
    }
    let filesystem = get("filesystem", "erofs");
    let blob = get("blob_device", "/dev/xvdf");
    let options = format!(
        "console=hvc0 init=/sbin/init airlock.system={system} airlock.storedev=/dev/xvde airlock.storefmt={filesystem}{suffix}",
        suffix = if filesystem == "erofs" {
            format!(" airlock.storeblobdev={blob}")
        } else {
            String::new()
        }
    );
    session.command(
        "qvm-prefs",
        &[domain, "kernelopts", &options],
        "nix-closure-domain-rollback-kernelopts.log",
    )?;
    let mut features = vec![
        ("system-source", system.into()),
        ("root-source", root),
        ("store-source", get("source", "")),
        ("store-mode", "shared-closure".into()),
        ("store-view-backend", get("view_backend", "erofs-objectmap")),
        ("store-filesystem", filesystem.clone()),
        ("store-generation", generation.to_string()),
        ("store-closure-hash", get("closure_hash", "")),
        (
            "store-view-image",
            get("view_image", &get("metadata_image", "")),
        ),
        ("store-mountpoint", "/nix/store".into()),
        ("store-device", "/dev/xvde".into()),
        ("store-read-only", "True".into()),
    ];
    if filesystem == "erofs" {
        features.extend([
            ("store-blob-device", blob),
            ("store-blob-volume", get("blob_volume", "")),
            ("store-blob-map", get("blob_map", "")),
        ]);
    }
    for (name, value) in features {
        session.command(
            "qvm-features",
            &[domain, &format!("airlock-{name}"), &value],
            "nix-closure-domain-rollback-identity.log",
        )?;
    }
    Ok(())
}

/// Parse explicit optional probes before any live changes occur.
fn enabled(key: &str) -> Result<bool> {
    match common::setting(key, "0").as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        _ => anyhow::bail!("invalid boolean {key}"),
    }
}

/// Retain useful machine observations even when the mutating scenario fails partway through.
fn diagnostics(session: &mut Session, domain: &str, peer: &str) {
    for (program, args, name) in [
        (
            "qvm-ls",
            vec!["--raw-data", "--fields", "NAME,STATE,CLASS,LABEL,TEMPLATE"],
            "qvm-ls.log",
        ),
        ("qvm-prefs", vec![domain], "qvm-prefs.log"),
        ("qvm-features", vec![domain], "qvm-features.log"),
        ("qvm-features", vec![peer], "peer-qvm-features.log"),
        ("qvm-volume", vec!["list", domain], "qvm-volumes.log"),
        (
            "journalctl",
            vec![
                "-b",
                "--no-pager",
                "-u",
                "qubesd",
                "-u",
                "qubes-db-dom0",
                "-u",
                "qubes-qrexec-policy-daemon",
            ],
            "journal.log",
        ),
    ] {
        let _ = session.command(program, &args, &format!("nix-closure-domain-{name}"));
    }
    for (source, dest) in [
        ("/var/lib/qubes/qubes.xml", "qubes.xml"),
        ("/var/log/libvirt/libxl/libxl-driver.log", "libxl.log"),
        (
            "/var/lib/airlock/qubes-policy.d/90-airlock.policy",
            "policy.log",
        ),
    ] {
        let _ = fs::copy(source, session.file(&format!("nix-closure-domain-{dest}")));
    }
    for (source, dest) in [
        ("config-apply-topology.json", "applied-topology.json"),
        (
            "config-apply-nix-domain-materializations.json",
            "materializations.json",
        ),
        ("config-apply-nix-domain-builds.json", "builds.json"),
        ("setup-config.json", "setup.json"),
    ] {
        let _ = fs::copy(
            session.file(source),
            session.file(&format!("nix-closure-domain-{dest}")),
        );
    }
}

/// Run the explicitly selected generation/isolation scenario; ordinary tests exercise helpers only.
pub(super) fn execute() -> Result<()> {
    let domain = common::setting("AIRLOCK_NIX_CLOSURE_DOMAIN", "airlock-closure-smoke");
    let peer = common::setting("AIRLOCK_NIX_CLOSURE_PEER_DOMAIN", &format!("{domain}-peer"));
    common::vm(&domain)?;
    common::vm(&peer)?;
    ensure!(domain != peer, "primary and peer must differ");
    let mode = common::setting("AIRLOCK_NIX_CLOSURE_APPLY_MODE", "direct");
    ensure!(
        ["direct", "config-flow"].contains(&mode.as_str()),
        "unsupported closure apply mode"
    );
    let reboot = enabled("AIRLOCK_NIX_CLOSURE_REBOOT_PROBE")?;
    let gui = enabled("AIRLOCK_NIX_CLOSURE_GUI_PROBE")?;
    let staging = common::setting("AIRLOCK_CONFIG_STAGING_VM", "sys-config-staging");
    let validator = common::setting("AIRLOCK_CONFIG_VALIDATOR_VM", "sys-config-validator");
    let vault = common::setting("AIRLOCK_CONFIG_VAULT_VM", "sys-config-vault");
    let message = common::setting(
        "AIRLOCK_NIX_CLOSURE_APPLY_MESSAGE",
        &format!(
            "nix-closure-smoke-{timestamp}",
            timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs()
        ),
    );
    let second_message = common::setting(
        "AIRLOCK_NIX_CLOSURE_APPLY_MESSAGE_GEN2",
        &format!("{message}-gen2"),
    );
    let role = if mode == "direct" {
        String::new()
    } else {
        common::setting("AIRLOCK_NIX_CLOSURE_EXPECTED_ROLE", "nix-closure-smoke")
    };
    let mut session = Session::new("nix-closure-domain-smoke", None, false)?;
    let mut record = json!({"schema":"airlock.nix-closure-domain-smoke.v1","domain":domain,"peer_domain":peer,"staging":staging,"validator":validator,"vault":vault,"apply_message":message,"apply_message_gen2":second_message,"apply_mode":mode,"reboot_status":"not-run","shutdown_start_status":"not-run","gui_status":"skipped"});
    let result: Result<()> = (|| {
        session.control_domain()?;
        for target in [&staging, &validator, &vault] {
            session.command("qvm-check", &[target], "nix-closure-domain-role-exists.log")?;
        }
        if mode == "config-flow" {
            for target in [&staging, &validator, &vault] {
                shutdown(
                    &mut session,
                    target,
                    "nix-closure-domain-config-shutdown.log",
                )?;
                let prefix = if target == &validator {
                    "AIRLOCK_NIX_CLOSURE_VALIDATOR"
                } else {
                    "AIRLOCK_NIX_CLOSURE_CONFIG_VM"
                };
                let default = if target == &validator { "1024" } else { "512" };
                for property in ["memory", "maxmem"] {
                    session.command(
                        "qvm-prefs",
                        &[
                            target,
                            property,
                            &common::setting(
                                &format!("{prefix}_{upper}", upper = property.to_ascii_uppercase()),
                                default,
                            ),
                        ],
                        "nix-closure-domain-config-memory.log",
                    )?;
                }
                session.start(target)?;
                ready(&mut session, target, "nix-closure-domain-config-qrexec.log")?;
            }
            session.remote(
                &staging,
                "airlock-config-staging-init",
                &[],
                None,
                "nix-closure-domain-staging-init.log",
            )?;
            session.remote(
                &validator,
                "airlock-config-validator-init",
                &[],
                None,
                "nix-closure-domain-validator-init.log",
            )?;
        } else {
            for (key, default) in [
                (
                    "AIRLOCK_NIX_CLOSURE_DIRECT_PRIMARY_SOURCE_VM",
                    "sys-config-staging",
                ),
                (
                    "AIRLOCK_NIX_CLOSURE_DIRECT_PEER_SOURCE_VM",
                    "sys-config-validator",
                ),
                (
                    "AIRLOCK_NIX_CLOSURE_DIRECT_GEN2_SOURCE_VM",
                    "sys-config-vault",
                ),
            ] {
                declaration(&mut session, &common::setting(key, default), "green")?;
            }
        }
        shutdown(
            &mut session,
            &domain,
            "nix-closure-domain-primary-pre-apply-shutdown.log",
        )?;
        shutdown(
            &mut session,
            &peer,
            "nix-closure-domain-peer-pre-apply-shutdown.log",
        )?;
        let applied = apply(
            &mut session,
            &domain,
            &peer,
            false,
            &mode,
            &staging,
            &message,
        )?;
        ensure!(
            applied["schema"] == "airlock.config-apply.v1"
                && applied["status"] == "passed"
                && applied["setup"]["status"] == "passed",
            "closure apply failed"
        );
        for target in [&domain, &peer] {
            ensure!(
                applied["nix_domain_materializations_result"]["domains"]
                    .as_array()
                    .is_some_and(|entries| entries
                        .iter()
                        .any(|entry| entry["domain"] == target.as_str())),
                "apply omitted materialization for {target}"
            );
            let classes = session.command(
                "qvm-ls",
                &["--raw-data", "--fields", "NAME,CLASS"],
                "nix-closure-domain-classes.log",
            )?;
            ensure!(
                classes
                    .lines()
                    .any(|line| line == format!("{target}|StandaloneVM")),
                "{target} is not a StandaloneVM"
            );
            let volumes = session.command(
                "qvm-volume",
                &["list", target],
                "nix-closure-domain-volume-list.log",
            )?;
            for volume in ["store", "store-data"] {
                session.command(
                    "qvm-volume",
                    &["info", &format!("{target}:{volume}")],
                    "nix-closure-domain-store-volume.log",
                )?;
                ensure!(
                    common::has_volume(&volumes, volume),
                    "volume list omits {target}:{volume}"
                );
            }
        }
        let policy = fs::read_to_string(common::path(
            "AIRLOCK_POLICY_FILE",
            "/var/lib/airlock/qubes-policy.d/90-airlock.policy",
        ))?;
        for rule in [
            format!("airlock.NixStoreRuntimeCheck * dom0 {domain} allow"),
            format!("airlock.NixStoreRuntimeCheck * dom0 {peer} allow"),
            "airlock.NixStoreRuntimeCheck * @anyvm @anyvm deny".into(),
        ] {
            ensure!(
                policy.lines().any(|line| line == rule),
                "runtime policy omits {rule}"
            );
        }
        let initial = Boot::observe(&mut session, &domain)?;
        let peer_boot = Boot::observe(&mut session, &peer)?;
        let options = session.command(
            "qvm-prefs",
            &[&domain, "kernelopts"],
            "nix-closure-domain-kernelopts.log",
        )?;
        let options = common::fields(&options.split_whitespace().collect::<Vec<_>>().join("\n"));
        for (key, expected) in [
            ("init", "/sbin/init"),
            ("airlock.system", initial.system.as_str()),
            ("airlock.storedev", initial.device.as_str()),
            ("airlock.storefmt", initial.filesystem.as_str()),
            ("airlock.storeblobdev", initial.blob.as_str()),
        ] {
            ensure!(
                options.get(key).map(String::as_str) == Some(expected),
                "kernelopts do not match {key}"
            );
        }
        let proof = sharing(&mut session, &domain, &peer, (&initial, &peer_boot), false)?;
        images(&mut session, &domain, &initial)?;
        for (target, boot) in [(&domain, &initial), (&peer, &peer_boot)] {
            session.start(target)?;
            ready(&mut session, target, "nix-closure-domain-qrexec.log")?;
            libvirt(
                &mut session,
                target,
                if target == &domain {
                    "nix-closure-domain-libvirt.xml"
                } else {
                    "nix-closure-domain-peer-libvirt.xml"
                },
            )?;
            boot.runtime(
                &mut session,
                target,
                if target == &domain {
                    "nix-closure-domain-runtime-service.log"
                } else {
                    "nix-closure-domain-peer-runtime-service.log"
                },
            )?;
        }
        guest(
            &mut session,
            &domain,
            &initial,
            &role,
            (mode == "config-flow").then_some(message.as_str()),
            "nix-closure-domain-guest-probe.log",
        )?;
        visibility(
            &mut session,
            &domain,
            text(&proof, "left_unique_object")?,
            text(&proof, "right_unique_object")?,
            text(&proof, "shared_object")?,
            "nix-closure-domain-visibility.log",
        )?;
        visibility(
            &mut session,
            &peer,
            text(&proof, "right_unique_object")?,
            text(&proof, "left_unique_object")?,
            text(&proof, "shared_object")?,
            "nix-closure-domain-peer-visibility.log",
        )?;
        let user = common::setting("AIRLOCK_QUBE_USER", "user");
        let marker = format!("/home/{user}/airlock-closure-smoke-persistent");
        session.remote(
            &domain,
            "tee",
            &[&marker],
            Some(message.as_bytes().to_vec()),
            "nix-closure-domain-persistence.log",
        )?;
        let before = pool(
            &mut session,
            &["status"],
            "nix-closure-domain-status-before.json",
        )?["segment_bytes_total"]
            .as_u64()
            .context("missing pool byte count")?;
        shutdown(
            &mut session,
            &domain,
            "nix-closure-domain-gen2-pre-shutdown.log",
        )?;
        let applied = apply(
            &mut session,
            &domain,
            &peer,
            true,
            &mode,
            &staging,
            &second_message,
        )?;
        ensure!(
            applied["status"] == "passed",
            "second generation apply failed"
        );
        let second = Boot::observe(&mut session, &domain)?;
        ensure!(
            second.generation != initial.generation,
            "second apply did not change generation"
        );
        let mut proof2 = sharing(&mut session, &domain, &peer, (&second, &peer_boot), true)?;
        let after = pool(
            &mut session,
            &["status"],
            "nix-closure-domain-status-after.json",
        )?["segment_bytes_total"]
            .as_u64()
            .context("missing pool byte count")?;
        let added = after
            .checked_sub(before)
            .context("pool size decreased unexpectedly during second publication")?;
        ensure!(
            added
                < proof2["left_logical_size"]
                    .as_u64()
                    .context("missing logical size")?,
            "second generation rewrote too much object data"
        );
        for (key, value) in [
            ("initial_generation", json!(initial.generation)),
            ("new_generation", json!(second.generation)),
            ("segment_bytes_before", json!(before)),
            ("segment_bytes_after", json!(after)),
            ("physical_bytes_added_for_new_generation", json!(added)),
            ("existing_object_reuse_verified", json!(true)),
        ] {
            proof2[key] = value;
        }
        common::save(
            &session.file("nix-closure-domain-sharing-proof-gen2.json"),
            &proof2,
        )?;
        session.start(&domain)?;
        ready(&mut session, &domain, "nix-closure-domain-qrexec-gen2.log")?;
        libvirt(&mut session, &domain, "nix-closure-domain-libvirt.xml")?;
        second.runtime(
            &mut session,
            &domain,
            "nix-closure-domain-runtime-service.log",
        )?;
        guest(
            &mut session,
            &domain,
            &second,
            &role,
            (mode == "config-flow").then_some(second_message.as_str()),
            "nix-closure-domain-guest-probe.log",
        )?;
        ready(
            &mut session,
            &peer,
            "nix-closure-domain-peer-qrexec-after-gen2.log",
        )?;
        peer_boot.runtime(
            &mut session,
            &peer,
            "nix-closure-domain-peer-runtime-service.log",
        )?;
        let running = pool(
            &mut session,
            &["status"],
            "nix-closure-domain-running-status.json",
        )?;
        for volume in ["store", "store-data"] {
            ensure!(
                running["running"]
                    .as_array()
                    .is_some_and(
                        |entries| entries.iter().any(|entry| entry["domain"] == domain
                            && entry["generation"] == second.generation
                            && entry["volume"] == volume)
                    ),
                "running references omit {volume}"
            );
        }
        let gc = pool(
            &mut session,
            &["gc", "--keep-generations", "0"],
            "nix-closure-domain-gc-running.json",
        )?;
        ensure!(
            contains(&gc, "running_generations", &domain, second.generation)
                && contains(&gc, "retained_generations", &domain, second.generation)
                && !contains(&gc, "removed_generations", &domain, second.generation)
                && gc["stale_running_refs"]
                    .as_array()
                    .is_some_and(Vec::is_empty),
            "GC did not protect running generation"
        );
        shutdown(
            &mut session,
            &domain,
            "nix-closure-domain-rollback-pre-shutdown.log",
        )?;
        restore(&mut session, &domain, initial.generation)?;
        pool(
            &mut session,
            &[
                "set-retention",
                "--domain",
                &domain,
                "--generation",
                &second.generation.to_string(),
                "--retain",
                "false",
            ],
            "nix-closure-domain-set-retention-gen2.json",
        )?;
        let gc = pool(
            &mut session,
            &["gc", "--keep-generations", "0"],
            "nix-closure-domain-gc-after-rollback.json",
        )?;
        ensure!(
            contains(&gc, "removed_generations", &domain, second.generation)
                && gc["removed_object_blobs"]
                    .as_array()
                    .is_some_and(|entries| !entries
                        .iter()
                        .any(|entry| entry == &proof["shared_object"])),
            "GC failed to collect unreferenced generation or removed shared data"
        );
        let restored = Boot::observe(&mut session, &domain)?;
        ensure!(
            restored.generation == initial.generation,
            "rollback selected wrong generation"
        );
        session.start(&domain)?;
        ready(
            &mut session,
            &domain,
            "nix-closure-domain-qrexec-after-rollback.log",
        )?;
        libvirt(&mut session, &domain, "nix-closure-domain-libvirt.xml")?;
        restored.runtime(
            &mut session,
            &domain,
            "nix-closure-domain-runtime-service.log",
        )?;
        guest(
            &mut session,
            &domain,
            &restored,
            &role,
            (mode == "config-flow").then_some(message.as_str()),
            "nix-closure-domain-guest-probe-after-rollback.log",
        )?;
        if reboot {
            session.remote(
                &domain,
                "systemctl",
                &["reboot"],
                None,
                "nix-closure-domain-reboot.log",
            )?;
            thread::sleep(Duration::from_secs(8));
            session.start(&domain)?;
            guest(
                &mut session,
                &domain,
                &restored,
                &role,
                (mode == "config-flow").then_some(message.as_str()),
                "nix-closure-domain-guest-probe-after-reboot.log",
            )?;
            restored.runtime(
                &mut session,
                &domain,
                "nix-closure-domain-runtime-service.log",
            )?;
            record["reboot_status"] = json!("passed");
        } else {
            record["reboot_status"] = json!("skipped");
        }
        shutdown(&mut session, &domain, "nix-closure-domain-shutdown.log")?;
        session.start(&domain)?;
        guest(
            &mut session,
            &domain,
            &restored,
            &role,
            (mode == "config-flow").then_some(message.as_str()),
            "nix-closure-domain-guest-probe-after-restart.log",
        )?;
        ensure!(
            session.remote(
                &domain,
                "cat",
                &[&marker],
                None,
                "nix-closure-domain-persistence.log"
            )? == message,
            "private marker did not survive restart"
        );
        record["shutdown_start_status"] = json!("passed");
        if gui {
            let helper = common::setting("AIRLOCK_GUI_SMOKE_COMMAND", "airlock-qube-gui-smoke");
            if std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .any(|directory| directory.join(&helper).is_file())
            {
                session.run(
                    Command::new(helper).env("AIRLOCK_TEST_QUBE", &domain),
                    None,
                    "nix-closure-domain-gui.log",
                )?;
                record["gui_status"] = json!("passed");
            }
        }
        for (key, value) in [
            ("system_source", restored.system.as_str()),
            ("root_source", restored.root.as_str()),
            ("store_source", restored.store.as_str()),
            ("store_view_image", restored.view.as_str()),
            ("store_mode", "shared-closure"),
            ("store_device", restored.device.as_str()),
        ] {
            record[key] = json!(value);
        }
        for (key, value) in [
            ("root_source_physical", &restored.root),
            ("store_source_physical", &restored.store),
            ("store_view_image_physical", &restored.view),
        ] {
            record[key] = json!(physical(value));
        }
        Ok(())
    })();
    diagnostics(&mut session, &domain, &peer);
    evidence(&session, &mut record)?;
    session.finish(&result, record)?;
    result
}

/// Assemble the established receipt schema from retained native scenario evidence.
fn evidence(session: &Session, record: &mut Value) -> Result<()> {
    record["logs"] = json!({});
    record["logs"]["command"] = json!(session.file("nix-closure-domain-smoke.commands.log"));
    record["logs"]["staging_update"] = json!(session.file("nix-closure-domain-staging-update.log"));
    record["logs"]["apply"] = json!(session.file("nix-closure-domain-apply.log"));
    record["logs"]["apply_result"] = json!(session.file("nix-closure-domain-config-apply.json"));
    record["logs"]["gen2_staging_update"] =
        json!(session.file("nix-closure-domain-staging-update-gen2.log"));
    record["logs"]["gen2_apply"] = json!(session.file("nix-closure-domain-apply-gen2.log"));
    record["logs"]["gen2_apply_result"] =
        json!(session.file("nix-closure-domain-config-apply-gen2.json"));
    record["logs"]["qvm_ls"] = json!(session.file("nix-closure-domain-qvm-ls.log"));
    record["logs"]["qvm_prefs"] = json!(session.file("nix-closure-domain-qvm-prefs.log"));
    record["logs"]["qvm_features"] = json!(session.file("nix-closure-domain-qvm-features.log"));
    record["logs"]["qvm_volumes"] = json!(session.file("nix-closure-domain-qvm-volumes.log"));
    record["logs"]["root_volume"] = json!(session.file("nix-closure-domain-root-volume.log"));
    record["logs"]["store_volume"] = json!(session.file("nix-closure-domain-store-volume.log"));
    record["logs"]["peer_store_volume"] =
        json!(session.file("nix-closure-domain-peer-store-volume.log"));
    record["logs"]["kernelopts"] = json!(session.file("nix-closure-domain-kernelopts.log"));
    record["logs"]["root_image_check"] =
        json!(session.file("nix-closure-domain-root-image-check.log"));
    record["logs"]["store_view_check"] =
        json!(session.file("nix-closure-domain-store-view-check.log"));
    record["logs"]["runtime_service"] =
        json!(session.file("nix-closure-domain-runtime-service.log"));
    record["logs"]["peer_runtime_service"] =
        json!(session.file("nix-closure-domain-peer-runtime-service.log"));
    record["logs"]["guest_probe"] = json!(session.file("nix-closure-domain-guest-probe.log"));
    record["logs"]["peer_guest_probe"] =
        json!(session.file("nix-closure-domain-peer-guest-probe.log"));
    record["logs"]["visibility"] = json!(session.file("nix-closure-domain-visibility.log"));
    record["logs"]["peer_visibility"] =
        json!(session.file("nix-closure-domain-peer-visibility.log"));
    record["logs"]["sharing_proof"] = json!(session.file("nix-closure-domain-sharing-proof.json"));
    record["logs"]["gen2_sharing_proof"] =
        json!(session.file("nix-closure-domain-sharing-proof-gen2.json"));
    record["logs"]["running_status"] =
        json!(session.file("nix-closure-domain-running-status.json"));
    record["logs"]["gc_running"] = json!(session.file("nix-closure-domain-gc-running.json"));
    record["logs"]["rollback_switch"] =
        json!(session.file("nix-closure-domain-rollback-switch.json"));
    record["logs"]["rollback_identity"] =
        json!(session.file("nix-closure-domain-rollback-identity.log"));
    record["logs"]["guest_probe_after_rollback"] =
        json!(session.file("nix-closure-domain-guest-probe-after-rollback.log"));
    record["logs"]["retention"] = json!(session.file("nix-closure-domain-set-retention-gen2.json"));
    record["logs"]["gc_after_rollback"] =
        json!(session.file("nix-closure-domain-gc-after-rollback.json"));
    record["logs"]["guest_probe_after_reboot"] =
        json!(session.file("nix-closure-domain-guest-probe-after-reboot.log"));
    record["logs"]["guest_probe_after_restart"] =
        json!(session.file("nix-closure-domain-guest-probe-after-restart.log"));
    record["logs"]["persistence"] = json!(session.file("nix-closure-domain-persistence.log"));
    record["logs"]["console"] = json!(session.file("nix-closure-domain-console.log"));
    record["logs"]["journal"] = json!(session.file("nix-closure-domain-journal.log"));
    record["logs"]["libvirt_xml"] = json!(session.file("nix-closure-domain-libvirt.xml"));
    record["logs"]["peer_libvirt_xml"] = json!(session.file("nix-closure-domain-peer-libvirt.xml"));
    record["apply_result"] = fs::read(session.file("nix-closure-domain-config-apply.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    record["setup_result"] = fs::read(session.file("nix-closure-domain-setup.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    record["nix_domain_builds"] = fs::read(session.file("nix-closure-domain-builds.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    record["nix_domain_materializations"] =
        fs::read(session.file("nix-closure-domain-materializations.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
    record["sharing_proof"] = fs::read(session.file("nix-closure-domain-sharing-proof.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    record["gen2_apply_result"] =
        fs::read(session.file("nix-closure-domain-config-apply-gen2.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
    record["gen2_sharing_proof"] =
        fs::read(session.file("nix-closure-domain-sharing-proof-gen2.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
    record["running_status"] = fs::read(session.file("nix-closure-domain-running-status.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    record["gc_running"] = fs::read(session.file("nix-closure-domain-gc-running.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    record["rollback_switch"] = fs::read(session.file("nix-closure-domain-rollback-switch.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    record["gc_after_rollback"] =
        fs::read(session.file("nix-closure-domain-gc-after-rollback.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(Value::Null);
    record["runtime_service_output"] =
        fs::read_to_string(session.file("nix-closure-domain-runtime-service.log"))
            .map(Value::String)
            .unwrap_or(Value::Null);
    record["guest_probe_output"] =
        fs::read_to_string(session.file("nix-closure-domain-guest-probe.log"))
            .map(Value::String)
            .unwrap_or(Value::Null);
    Ok(())
}
