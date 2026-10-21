//! Follow real prepared storage through offline image assembly and native first-boot setup.
//! Only upstream Qubes objects and administrative command replies are inert peers.

use super::*;
use crate::{
    common::store::Store,
    coordinator::fixture::{Machine, SETUP},
};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

const DESCRIPTORS: &str =
    "/nix/store/cccccccccccccccccccccccccccccccc-boot-descriptors/descriptors";

/// Expose a writable descriptor output at a real Nix identity inside the existing private-store namespace.
fn child() -> Result<()> {
    let machine = Machine::new()?;
    fs::create_dir(machine.path("view/cccccccccccccccccccccccccccccccc-boot-descriptors"))?;
    fs::create_dir(machine.path("descriptor-output"))?;
    let launcher = machine.path("host/sw/bin/airlock-setup-config");
    fs::copy(
        common::binary("AIRLOCK_EXEC_BIN", "airlock-exec"),
        &launcher,
    )?;
    fs::write(
        launcher.with_file_name("airlock-setup-config.json"),
        serde_json::to_vec(&json!({
            "program":common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"),
            "args":["setup-config","run","--script","@self@"],
            "env":{"AIRLOCK_QUBES_XML":"/var/lib/qubes/qubes.xml"}
        }))?,
    )?;
    fs::write(machine.path("etc/usb-dev"), "airlock-usb-dev-v2\n")?;
    let executable = std::env::current_exe()?;
    let original = machine.program(
        &executable,
        &[
            "--ignored",
            "--exact",
            "setup::image_handoff::namespace_image_handoff_and_first_boot",
            "--nocapture",
        ],
    )?;
    let args = original
        .get_args()
        .map(std::ffi::OsStr::to_owned)
        .collect::<Vec<_>>();
    let index = args
        .iter()
        .position(|arg| arg == executable.as_os_str())
        .context("namespace test executable")?;
    let mut command = Command::new(original.get_program());
    command
        .args(&args[..index])
        .arg("--bind")
        .arg(machine.path("descriptor-output"))
        .arg(Path::new(DESCRIPTORS).parent().unwrap())
        .arg("--bind")
        .arg(machine.path("etc"))
        .arg("/etc/airlock")
        .args(["--setenv", "PATH"])
        .arg(std::env::join_paths(
            std::env::split_paths(&std::env::var_os("PATH").context("test tool path")?)
                .filter_map(|path| fs::canonicalize(path).ok()),
        )?)
        .args(&args[index..]);
    for (key, value) in original.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    command
        .env_remove("AIRLOCK_TEST_MACHINE_ROOT")
        .env_remove("AIRLOCK_NIX_ROOT")
        .env("AIRLOCK_USE_HOST_NIX_STORE", "1")
        .env("AIRLOCK_IMAGE_HANDOFF_CHILD", "1")
        .env("PATH", std::env::var_os("PATH").context("test tool path")?);
    checked(&mut command)?;
    Ok(())
}

/// Include both streams when a native JSON command rejects a generated artifact.
fn checked(command: &mut Command) -> Result<()> {
    let output = common::run(command, None)?;
    anyhow::ensure!(
        output.status.success(),
        "{program:?} failed: {stdout}; {stderr}",
        program = command.get_program(),
        stdout = String::from_utf8_lossy(&output.stdout),
        stderr = String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Copy prepared outputs as the image assembler does, preserving sparse writable disks.
fn copy(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    common::run(
        Command::new("cp")
            .args(["-a", "--sparse=always", "--"])
            .arg(source.join("."))
            .arg(target),
        None,
    )?
    .checked()?;
    Ok(())
}

/// Rebase observed offline object paths to their installed location; policy decisions stay native.
fn relocate(value: &mut Value, before: &str, after: &str) {
    match value {
        Value::String(text) => *text = text.replace(before, after),
        Value::Array(values) => values
            .iter_mut()
            .for_each(|value| relocate(value, before, after)),
        Value::Object(values) => values
            .values_mut()
            .for_each(|value| relocate(value, before, after)),
        _ => {}
    }
}

/// One topology, committed pool, offline Qubes objects and actual setup are exercised together.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_image_handoff_and_first_boot() -> Result<()> {
    if std::env::var("AIRLOCK_IMAGE_HANDOFF_CHILD").as_deref() != Ok("1") {
        return child();
    }
    let store = Store::new()?;
    let root_source = "/nix/store/dddddddddddddddddddddddddddddddd-root/root.img";
    let root_image = store
        .physical
        .join("dddddddddddddddddddddddddddddddd-root/root.img");
    fs::create_dir_all(root_image.parent().unwrap())?;
    fs::copy(&store.root, &root_image)?;
    let specs = json!([{"name":"sys-gui", "domain":"sys-gui", "system":store.system,
        "root_source":root_source, "kernel_name":"test", "closure_source":store.closure,
        "physical_store_root":store.physical}]);
    let specs_path = store.temporary.path().join("specs.json");
    fs::write(&specs_path, serde_json::to_vec(&specs)?)?;
    let pool = Path::new("/var/lib/airlock/closure-store");
    checked(
        store
            .command(&["image", "prepare-pool"])
            .arg(&store.pool)
            .arg(DESCRIPTORS)
            .arg(&specs_path)
            .arg(pool),
    )
    .context("prepare immutable pool")?;
    let kernel = store.temporary.path().join("kernel");
    fs::create_dir(&kernel)?;
    fs::write(kernel.join("vmlinuz"), "kernel")?;
    let mut desired = topology();
    desired["kernels"]["test"] = json!({"source":kernel});
    desired["domains"]["sys-gui"] = json!({
        "class":"StandaloneVM", "label":"green", "netvm":null,
        "prefs":{"autostart":false,"guivm":null}, "features":{}, "services":{}, "devices":{},
        "backend":{"type":"nix-closure", "system":{"source":store.system},
            "root":{"source":root_source}, "kernel":{"name":"test"},
            "state":{"persistent_size":33554432},
            "store":{"source":format!("{DESCRIPTORS}/sys-gui"), "mode":"shared-closure",
                "device":"/dev/xvde", "blobDevice":"/dev/xvdf", "mountpoint":"/nix/store", "viewBackend":"erofs-objectmap"}}
    });
    let topology_path = Path::new("/etc/airlock/qubes-topology.json");
    fs::write(topology_path, serde_json::to_vec(&desired)?)?;
    let tools = store.temporary.path().join("tools");
    let modules = store.temporary.path().join("python");
    fs::create_dir(&tools)?;
    fs::create_dir(&modules)?;
    symlink(&store.service, tools.join("airlock-dom0-service"))?;
    symlink(
        common::binary(
            "AIRLOCK_QUBES_FIXTURE_MODULE",
            "examples/libqubes_objects.so",
        ),
        modules.join("qubes.so"),
    )?;
    symlink(
        common::binary("AIRLOCK_QUBES_MODULE", "libairlock_qubes_native.so"),
        modules.join("airlock_qubes.so"),
    )?;
    let runtime = store.temporary.path().join("runtime-image");
    let mut image = Command::new(common::binary(
        "AIRLOCK_PREPARE_RUNTIME_BIN",
        "airlock-prepare-runtime",
    ));
    store.environment(&mut image);
    image
        .args([
            topology_path.as_os_str(),
            store.pool.as_os_str(),
            runtime.as_os_str(),
        ])
        .env(
            "PATH",
            std::env::join_paths(
                std::iter::once(tools)
                    .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
            )?,
        )
        .env("PYTHONPATH", modules)
        .env("AIRLOCK_QUBES_FIXTURE", "image")
        .env(
            "AIRLOCK_TEST_IMAGE_PLAN",
            store.temporary.path().join("image-plan.json"),
        )
        .env(
            "AIRLOCK_TEST_IMAGE_RELOAD",
            store.temporary.path().join("image-reload.json"),
        )
        .env(
            "AIRLOCK_TEST_IMAGE_CLOSES",
            store.temporary.path().join("image-closes.jsonl"),
        )
        .env(
            "AIRLOCK_TEST_IMAGE_OBSERVED",
            store.temporary.path().join("observed.json"),
        );
    checked(&mut image).context("assemble offline image")?;

    // The installed topology is the same declaration used above, not a separate fixture.
    copy(&store.pool, pool)?;
    copy(&runtime.join("pool-selections"), pool)?;
    copy(&runtime.join("qubes"), Path::new("/var/lib/qubes"))?;
    fs::copy(
        runtime.join("receipt.json"),
        "/var/lib/airlock/prepared-runtime.json",
    )?;
    let mut snapshots: Vec<Value> =
        serde_json::from_slice(&fs::read(store.temporary.path().join("observed.json"))?)?;
    let mut observed = snapshots.remove(0);
    relocate(
        &mut observed,
        runtime.join("qubes").to_str().unwrap(),
        "/var/lib/qubes",
    );
    let features = observed["features"].clone();
    let image_plan: Value =
        serde_json::from_slice(&fs::read(store.temporary.path().join("image-plan.json"))?)?;
    let kernelopts = image_plan["domains"][0]["kernelopts"].clone();
    let mut setup = Setup::new(
        desired.clone(),
        json!({"sys-gui":{
            "prefs":{"label":"green", "autostart":false,"netvm":null,"guivm":null,"kernel":"test","kernelopts":kernelopts},
            "features":features
        }}),
        json!({}),
    )?;
    common::peer_link(&setup.peers.path("bin"), "storage-bridge")?;
    fs::write(
        setup.peers.path("qubes.json"),
        serde_json::to_vec(&observed)?,
    )?;
    let disks = ["root", "private"]
        .map(|name| PathBuf::from(format!("/var/lib/qubes/appvms/sys-gui/{name}.img")));
    setup.peers.rules(
        "qvm-volume",
        json!([
            {"args":["info","sys-gui:private"],
                "stdout":format!("size {size}\n", size=fs::metadata(&disks[1])?.len())},
            {}
        ]),
    )?;
    // Stand in for private user data created after image assembly but before a retry.
    {
        use std::io::Write;
        fs::OpenOptions::new()
            .write(true)
            .open(&disks[1])?
            .write_all(b"private user data must survive setup")?;
    }
    let before = disks
        .iter()
        .map(fs::read)
        .collect::<std::io::Result<Vec<_>>>()?;
    fs::write(setup.peers.path("capabilities"), "control_d\n")?;
    // Boot uses the installed topology; setup's launcher binds its XML observation separately.
    let boot = || -> Result<Command> {
        let environment = setup.command("--apply")?;
        let mut command = Command::new(&store.service);
        for (key, value) in environment.get_envs() {
            if let Some(value) = value {
                command.env(key, value);
            } else {
                command.env_remove(key);
            }
        }
        command
            .args(["boot-setup", "--gui", SETUP, "--apply"])
            .env("AIRLOCK_DOM0_SERVICE", &store.service)
            .env("AIRLOCK_QUBES_XML", "/var/lib/qubes/qubes.xml")
            .env("AIRLOCK_QUBES_KERNEL_BASE", "/var/lib/qubes/vm-kernels")
            .env(
                "AIRLOCK_QUBES_OBJECTS_BIN",
                setup.peers.path("bin/storage-bridge"),
            )
            .env("AIRLOCK_TEST_STORAGE_BRIDGE_ROOT", setup.peers.path(""))
            .env("AIRLOCK_QUBES_STORAGE_STATE", setup.peers.path("storage"));
        Ok(command)
    };
    // Retained generations must suffice even when admission inputs are unavailable.
    // Dropping the checked map at any nested setup helper now makes this scenario fail.
    fs::remove_file(Path::new(DESCRIPTORS).join("sys-gui/store-paths"))?;
    let receipt = Path::new("/var/lib/airlock/machine/boot-configuration.json");
    // Fail after actual setup succeeds, while the coordinator waits to observe XML before recording completion.
    let fifo = setup.peers.path("held.xml");
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )?;
    let mut interrupted = boot()?
        .env("AIRLOCK_QUBES_XML", &fifo)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let complete = setup.peers.path("logs/setup-config.json");
    let deadline = Instant::now() + Duration::from_secs(20);
    while !complete.exists() && Instant::now() < deadline {
        if let Some(status) = interrupted.try_wait()? {
            anyhow::bail!("boot exited before the interruption boundary: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let completed = complete.exists();
    interrupted.kill()?;
    interrupted.wait()?;
    anyhow::ensure!(completed, "setup did not reach the interruption boundary");
    let result: Value = serde_json::from_slice(&fs::read(&complete)?)?;
    anyhow::ensure!(
        result["status"] == "passed",
        "setup failed before interruption: {result}; {log}",
        log = fs::read_to_string(setup.peers.path("logs/setup-config.commands.log"))?
    );
    assert!(!receipt.exists());
    let mut resumed = boot()?;
    let output = common::run(&mut resumed, None)?;
    anyhow::ensure!(
        output.status.success(),
        "resume failed: {stdout}; {stderr}",
        stdout = String::from_utf8_lossy(&output.stdout),
        stderr = String::from_utf8_lossy(&output.stderr)
    );
    let first: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(first["reused"], false);
    assert_eq!(common::json(&mut boot()?, None)?["reused"], true);
    for (path, bytes) in disks.iter().zip(&before) {
        assert_eq!(&fs::read(path)?, bytes);
    }
    assert!(
        !setup
            .peers
            .calls()?
            .iter()
            .any(|call| call["command"] == "qvm-volume"
                && matches!(
                    call["args"][0].as_str(),
                    Some("import" | "resize" | "clone")
                ))
    );

    // Every selection disagreement must fail before even the inert Qubes adapters run.
    let prepared = Path::new("/var/lib/airlock/prepared-runtime.json");
    let original = fs::read(prepared)?;
    for (pointer, value) in [
        ("/boot_handoff/generations/sys-gui", json!(999)),
        ("/boot_handoff/generations", json!({})),
        ("/boot_handoff/status", json!("failed")),
        ("/schema", json!("unsupported")),
    ] {
        let mut corrupt: Value = serde_json::from_slice(&original)?;
        *corrupt.pointer_mut(pointer).unwrap() = value;
        fs::write(prepared, serde_json::to_vec(&corrupt)?)?;
        let calls = setup.peers.calls()?.len();
        assert!(
            !common::run(&mut boot()?, None)?.status.success(),
            "{pointer}"
        );
        assert_eq!(setup.peers.calls()?.len(), calls, "{pointer}");
        fs::write(prepared, &original)?;
    }
    fs::remove_file(prepared)?;
    let calls = setup.peers.calls()?.len();
    assert!(!common::run(&mut boot()?, None)?.status.success());
    assert_eq!(setup.peers.calls()?.len(), calls);
    fs::write(prepared, &original)?;

    let manifest = pool.join("domains/sys-gui/generations/1/manifest.json");
    let committed = fs::read(&manifest)?;
    fs::write(&manifest, b"corrupted")?;
    assert!(!common::run(&mut boot()?, None)?.status.success());
    assert_eq!(setup.peers.calls()?.len(), calls);
    fs::write(&manifest, &committed)?;
    // Restoring contents changes observed metadata, so reapply before caching again.
    assert_eq!(common::json(&mut boot()?, None)?["reused"], false);
    assert_eq!(common::json(&mut boot()?, None)?["reused"], true);
    for (path, bytes) in disks.iter().zip(&before) {
        assert_eq!(&fs::read(path)?, bytes);
    }
    Ok(())
}
