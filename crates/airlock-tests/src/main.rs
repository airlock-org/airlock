//! Inert process peers keep integration scenarios in Rust without exposing test commands in services.

mod replies;

mod rpc;

mod menus;

mod machine;

mod submission;

mod gui_protocol;

mod configuration;

mod image_gate;

mod workstation;

mod maintenance;

mod flow;

mod network;

mod builder;

mod terminal;

use anyhow::{Context, Result, bail, ensure};
use serde_json::json;
use std::{
    fs,
    io::{BufRead, Read, Write},
    os::unix::process::CommandExt,
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

/// Hold inherited pipes open until the test's cancellation or cleanup kills this process.
fn hold() {
    std::thread::sleep(Duration::from_secs(30));
}

/// Inspect real inherited CPU affinity independently of the build wrapper's recorded selection.
fn build_cpus() -> Result<Vec<usize>> {
    let affinity = rustix::thread::sched_getaffinity(None)?;
    Ok((0..rustix::thread::CpuSet::MAX_CPU)
        .filter(|cpu| affinity.is_set(*cpu))
        .collect())
}

/// Observe affinity in a command, its new thread and a separately executed child process.
fn observe_build_cpus(args: &[String]) -> Result<()> {
    let thread = std::thread::spawn(build_cpus)
        .join()
        .map_err(|_| anyhow::anyhow!("CPU observer thread panicked"))??;
    let child = Command::new(std::env::current_exe()?)
        .env("AIRLOCK_TEST_PEER_MODE", "build-cpu-child")
        .output()?;
    ensure!(child.status.success(), "CPU observer child failed");
    let child: Vec<usize> = serde_json::from_slice(&child.stdout)?;
    println!(
        "{}",
        json!({
            "stage": args.last(),
            "cpus": build_cpus()?,
            "thread_cpus": thread,
            "child_cpus": child,
            "cargo_jobs": std::env::var("CARGO_BUILD_JOBS")?,
            "nix_config": std::env::var("NIX_CONFIG")?,
        })
    );
    Ok(())
}

/// Act only on fixture paths and protocol data explicitly supplied by the Rust test.
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let name = Path::new(&args[0])
        .file_name()
        .context("peer executable name")?;
    if name == "cargo"
        && let Some(record) = std::env::var_os("AIRLOCK_TEST_CPU_BOOTSTRAP_RECORD")
    {
        fs::write(
            record,
            serde_json::to_vec(&json!({"cpus":build_cpus()?,"args":args[1..]}))?,
        )?;
        std::process::exit(37);
    }
    if name == "mkfs.erofs" {
        return Ok(fs::write(&args[args.len() - 2], b"index fixture")?);
    }
    if name == "terminal-relay" {
        let root =
            std::env::var_os("AIRLOCK_TEST_TERMINAL_ROOT").context("terminal fixture root")?;
        return terminal::execute(Path::new(&root), &args[1..]);
    }
    if let Some(root) = std::env::var_os("AIRLOCK_TEST_BUILDER_ROOT") {
        return builder::execute(Path::new(&root), &name.to_string_lossy(), &args[1..]);
    }
    if let Some(root) = std::env::var_os("AIRLOCK_TEST_NETWORK_ROOT") {
        return network::execute(Path::new(&root), &name.to_string_lossy(), &args[1..]);
    }
    if let Some(root) = std::env::var_os("AIRLOCK_TEST_FLOW") {
        return flow::execute(Path::new(&root), &name.to_string_lossy());
    }
    if let Some(root) = std::env::var_os("AIRLOCK_TEST_WORKSTATION") {
        return workstation::execute(Path::new(&root), &name.to_string_lossy(), &args[1..]);
    }
    if let Some(root) = std::env::var_os("AIRLOCK_TEST_IMAGE_GATE") {
        return image_gate::execute(Path::new(&root), &name.to_string_lossy(), &args[1..]);
    }
    if let Some(root) = std::env::var_os("AIRLOCK_TEST_CONFIGURATION") {
        return configuration::execute(Path::new(&root), &name.to_string_lossy(), &args[1..]);
    }
    if name == "storage-bridge" {
        let root =
            std::env::var_os("AIRLOCK_TEST_STORAGE_BRIDGE_ROOT").context("storage fixture root")?;
        return airlock_tests::attachment::execute(Path::new(&root));
    }
    if let Some(capture) = std::env::var_os("AIRLOCK_TEST_SUBMISSION") {
        return submission::execute(Path::new(&capture), &args[1..]);
    }
    if let Some(root) = std::env::var_os("AIRLOCK_TEST_MACHINE_ROOT") {
        return machine::execute(Path::new(&root), &name.to_string_lossy(), &args[1..]);
    }
    if let Some(configuration) = std::env::var_os("AIRLOCK_TEST_MENUS") {
        return menus::execute(
            Path::new(&configuration),
            &name.to_string_lossy(),
            &args[1..],
        );
    }
    if let Some(root) = std::env::var_os("AIRLOCK_TEST_RPC_ROOT") {
        return rpc::execute(Path::new(&root), &name.to_string_lossy(), &args[1..]);
    }
    if let Some(specification) = std::env::var_os("AIRLOCK_TEST_REPLIES") {
        return replies::execute(
            Path::new(&specification),
            &name.to_string_lossy(),
            &args[1..],
        );
    }
    let mode = std::env::var("AIRLOCK_TEST_PEER_MODE")
        .unwrap_or_else(|_| name.to_string_lossy().into_owned());
    match mode.as_str() {
        "build-cpu-child" => println!("{}", serde_json::to_string(&build_cpus()?)?),
        "airlock-dom0-service" => {
            let request: serde_json::Value = serde_json::from_reader(std::io::stdin())?;
            let mut calls = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(std::env::var_os("AIRLOCK_TEST_IMAGE_CALLS").context("image calls")?)?;
            writeln!(calls, "{}", json!({"args":args[1..],"request":request}))?;
            let response = if args.iter().any(|arg| arg == "inspect-prepared") {
                json!({
                    "view_size": 4096,
                    "root_image": std::env::var("AIRLOCK_TEST_IMAGE_ROOT")?,
                    "generation": 7,
                    "root_artifact": {
                        "sha256": "a".repeat(64)
                    },
                    "closure_hash": "b".repeat(64),
                    "view_image": "/view.img",
                    "blob_map": "/blob.map"
                })
            } else if args.iter().any(|arg| arg == "image-volumes") {
                json!([{"name":"store","pool":"airlock-closure"}])
            } else if args.iter().any(|arg| arg == "verify-snapshot") {
                ensure!(
                    std::env::var("AIRLOCK_TEST_IMAGE_DAMAGE").as_deref() != Ok("snapshot"),
                    "snapshot rejected"
                );
                json!({})
            } else {
                bail!("unexpected image storage operation");
            };
            println!("{response}");
        }
        "airlock-sys-gui-service" => {
            fs::write(
                std::env::var_os("AIRLOCK_ARGS_RECEIPT").context("argument receipt")?,
                serde_json::to_vec(&args[1..])?,
            )?;
        }
        "xdg-user-dir" => {
            println!("{home}/Media", home = std::env::var("HOME")?);
        }
        "capture-open-peer" => {
            let mut calls = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(std::env::var_os("AIRLOCK_TEST_CAPTURE_CALLS").context("capture calls")?)?;
            writeln!(calls, "{}", json!(args[1..]))?;
        }
        "capture-systemctl" => println!("DISPLAY=:99"),
        "capture-xdpyinfo" => ensure!(
            std::env::var("DISPLAY").as_deref() == Ok(":99"),
            "unexpected display"
        ),
        "capture-qrexec" => {
            ensure!(
                !Path::new(&std::env::var("AIRLOCK_TEST_CAPTURE_FAILURE")?).exists(),
                "receiving VM unavailable"
            );
            let service = args.get(2).context("capture RPC service")?;
            if service == "airlock.AppSessionReady" {
                return Ok(());
            }
            let (name, kind) = service.split_once('+').context("capture RPC kind")?;
            let operation = if name == "airlock.CaptureImport" {
                "capture-receive"
            } else {
                "capture-open"
            };
            let home = std::env::var("AIRLOCK_TEST_CAPTURE_REMOTE")?;
            return Err(Command::new(
                std::env::var_os("AIRLOCK_TEST_CAPTURE_VM").context("capture VM service")?,
            )
            .args([operation, kind])
            .env("HOME", &home)
            .env("XDG_STATE_HOME", format!("{home}/state"))
            .env("QREXEC_REMOTE_DOMAIN", "sys-gui")
            .exec()
            .into());
        }
        "nix" => {
            fs::write(
                std::env::var_os("AIRLOCK_TEST_NIX_ARGS").context("Nix verification receipt")?,
                serde_json::to_vec(&args[1..])?,
            )?;
            std::process::exit(
                std::env::var("AIRLOCK_TEST_NIX_EXIT")
                    .unwrap_or_else(|_| "0".into())
                    .parse()?,
            );
        }
        "journalctl" => {
            println!("early boot evidence");
            std::io::stdout().flush()?;
            std::process::exit(std::env::var("AIRLOCK_TEST_EXIT_CODE")?.parse()?);
        }
        "qrexec-client" => {
            let error =
                Command::new(std::env::var_os("AIRLOCK_TEST_RECEIVER").context("receiver")?)
                    .args(["storage", "artifact", "receive-incremental", "--pool-root"])
                    .arg(std::env::var_os("AIRLOCK_TEST_POOL").context("pool")?)
                    .args(["--expected-domain", "usr-home", "--approved-by", "sys-ctrl"])
                    .exec();
            return Err(error.into());
        }
        "receipt-peer" => {
            let mut request = String::new();
            std::io::stdin().lock().read_line(&mut request)?;
            println!("{}", std::env::var("AIRLOCK_TEST_RECEIPT")?);
        }
        "input-qrexec" => {
            let node = args[4]
                .rsplit(' ')
                .next()
                .context("sender command")?
                .trim_matches('\'');
            let child = Command::new(std::env::current_exe()?)
                .arg("hold")
                .env("AIRLOCK_TEST_PEER_MODE", "airlock-test-peer")
                .stdin(Stdio::null())
                .spawn()?;
            let path = std::env::var_os("AIRLOCK_TEST_CALLS").context("calls")?;
            let mut calls = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            writeln!(
                calls,
                "{}",
                json!({"args":args[1..],"node":node,"pid":std::process::id(),"child":child.id()})
            )?;

            if node.ends_with("event2")
                && Path::new(&std::env::var("AIRLOCK_TEST_FAILURE")?).exists()
            {
                std::process::exit(1);
            }
            hold();
        }
        "boot-cache-maintenance" => maintenance::execute(&args[1..])?,
        "airlock-test-peer" => match args.get(1).map(String::as_str) {
            Some("observe-build-cpus") => observe_build_cpus(&args[2..])?,
            Some("host-capabilities") => println!("{}", airlock_tests::system::capabilities()),
            Some("systemd-order-setup") => {
                ensure!(
                    Command::new(&args[2])
                        .args(["--user", "stop", &args[3]])
                        .status()?
                        .success(),
                    "inert daemon stop failed"
                );
                if args[4] == "true" {
                    ensure!(
                        Command::new(&args[2])
                            .args(["--user", "start", &args[3]])
                            .status()?
                            .success(),
                        "inert daemon start failed"
                    );
                }
                ensure!(args[5] == "true", "selected setup failure");
            }
            Some("systemd-order-start") => fs::write(&args[2], "started\n")?,
            Some("hold") => hold(),
            Some("prepare-gui-tests") => gui_protocol::prepare(
                Path::new(args.get(2).context("patch source")?),
                Path::new(args.get(3).context("C fixture output")?),
            )?,
            Some("prepare-memory") => {
                let header = Path::new(&args[2]);
                let output = Path::new(&args[3]);
                ensure!(header.is_file(), "missing memory accounting header");
                let temporary = tempfile::tempdir()?;
                let source = temporary.path().join("memory.c");
                fs::write(&source, include_bytes!("../fixtures/memory_accounting.c"))?;
                let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
                airlock_vm_service::support::run(
                    Command::new(compiler)
                        .args([
                            "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
                            "-I",
                        ])
                        .arg(header.parent().context("header parent")?)
                        .arg(source)
                        .arg("-o")
                        .arg(output),
                    None,
                    Duration::from_secs(30),
                )?
                .checked()?;
            }
            Some("large-output") => {
                let bytes = vec![b'x'; 2 * 1024 * 1024];
                if args.get(2).map(String::as_str) == Some("stderr") {
                    std::io::stderr().write_all(&bytes)?;
                } else {
                    std::io::stdout().write_all(&bytes)?;
                }
            }
            Some("orphan-output") => {
                let child = Command::new(std::env::current_exe()?).arg("hold").spawn()?;
                fs::write(&args[2], child.id().to_string())?;
            }
            Some("summary") => {
                let mut bytes = Vec::new();
                std::io::stdin().read_to_end(&mut bytes)?;
                ensure!(bytes.len() >= 7, "missing header");
                println!("{}", bytes.len());
                println!("{}", std::str::from_utf8(&bytes[..7])?);
            }
            Some("exit") => {}
            Some("cat") => {
                std::io::copy(&mut std::io::stdin(), &mut std::io::stdout())?;
            }
            Some("escape") => {
                Command::new(std::env::current_exe()?)
                    .args(["detached", &args[2]])
                    .spawn()?;
                hold();
            }
            Some("detached") => {
                rustix::process::setsid()?;
                fs::write(&args[2], std::process::id().to_string())?;
                hold();
            }
            Some("change-group") => {
                let parent = rustix::process::getppid().context("parent process")?;
                rustix::process::setpgid(None, Some(rustix::process::getpgid(Some(parent))?))?;
                fs::write(&args[2], std::process::id().to_string())?;
                hold();
            }
            Some("recover-mounted") => {
                let source = &args[2];
                let target = &args[3];
                ensure!(
                    Command::new("mount")
                        .args(["--bind", source, target])
                        .status()?
                        .success(),
                    "mount failed"
                );
                let result = Command::new(&args[4])
                    .args(["storage", "pool", "recover", "--pool-root", &args[5]])
                    .output();
                let visible = fs::read_to_string(Path::new(target).join("sentinel"));
                let unmounted = Command::new("umount").arg(target).status()?;
                let result = result?;
                ensure!(unmounted.success(), "unmount failed");
                ensure!(
                    visible? == "preserve mounted bytes",
                    "mounted path lost its contents"
                );
                ensure!(
                    !result.status.success(),
                    "mounted temporary tree was removed"
                );
                ensure!(
                    String::from_utf8_lossy(&result.stderr)
                        .contains("refusing cleanup of mounted path")
                        || String::from_utf8_lossy(&result.stdout)
                            .contains("refusing cleanup of mounted path"),
                    "unexpected rejection: {result:?}"
                );
                ensure!(
                    fs::read_to_string(Path::new(source).join("sentinel"))?
                        == "preserve mounted bytes",
                    "mounted bytes changed"
                );
            }
            _ => bail!("unknown peer operation"),
        },
        _ => bail!("unknown peer mode: {mode}"),
    }
    Ok(())
}
