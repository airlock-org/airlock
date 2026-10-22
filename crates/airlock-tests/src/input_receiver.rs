//! Exercise installed input RPCs against real uinput and evdev only in a marked disposable VM.

use airlock_vm_service::support;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    time::{Duration, Instant},
};

/// Execute the installed native readiness launcher with an empty command search path.
fn readiness() -> Result<Value> {
    let output = support::run(
        &mut command("airlock.InputReceiverReady", "dom0", ""),
        None,
        Duration::from_secs(5),
    )?
    .checked()?;
    let value: Value = serde_json::from_slice(&output)?;
    ensure!(
        value["schema"] == "airlock.input-receiver.v1",
        "unsupported readiness response"
    );
    Ok(value)
}

/// Preserve the installed entry point, its sidecar lookup and the authenticated qrexec environment.
fn command(service: &str, caller: &str, argument: &str) -> Command {
    let mut command = Command::new(format!("/etc/qubes-rpc/{service}"));
    command
        .env("QREXEC_REMOTE_DOMAIN", caller)
        .env("QREXEC_SERVICE_ARGUMENT", argument)
        .env("PATH", "");
    command
}

/// A receiver and its protocol pipe remain owned until device-removal verification completes.
struct Receiver {
    // Installed service process; cancellation and reaping are guaranteed on assertion failure.
    child: Child,
    // Keeping stdin open preserves the upstream receiver's device lifetime.
    pipe: Option<ChildStdin>,
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.pipe.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Set one native x86_64 Linux capability bit in the pinned upstream protocol's word array.
fn set(bytes: &mut [u8], offset: usize, bit: usize) {
    bytes[offset + bit / 8] |= 1 << (bit % 8);
}

/// Encode protocol v1 from qubes-app-linux-input-proxy's protocol.h, not a replacement receiver.
fn hello(name: &str, kind: &str) -> Vec<u8> {
    // Ten native unsigned-long bitmaps occupy 176 bytes on x86_64:
    // prop, ev, key, rel, abs, msc, led, snd, ff, sw. A 128-byte name
    // precedes 64 Linux input_absinfo records of six signed 32-bit fields.
    let mut caps = vec![0_u8; 176 + 128 + 64 * 24];
    set(&mut caps, 8, 1); // EV_KEY
    if matches!(kind, "keyboard" | "composite") {
        for key in [28, 30, 44] {
            set(&mut caps, 16, key);
        }
    }
    if matches!(kind, "mouse" | "composite") {
        set(&mut caps, 8, 2); // EV_REL
        set(&mut caps, 16, 272); // BTN_LEFT
        set(&mut caps, 112, 0);
        set(&mut caps, 112, 1);
    }
    if kind == "tablet" {
        set(&mut caps, 8, 3); // EV_ABS
        set(&mut caps, 16, 330); // BTN_TOUCH
        for axis in [0, 1] {
            set(&mut caps, 120, axis);
            let maximum = 176 + 128 + axis * 24 + 8;
            caps[maximum..maximum + 4].copy_from_slice(&1024_i32.to_le_bytes());
        }
    }
    caps[176..176 + name.len()].copy_from_slice(name.as_bytes());
    let mut bytes = Vec::new();
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend((caps.len() as u32).to_le_bytes());
    bytes.extend(caps);
    bytes
}

/// Find only the synthetic device owned by the currently open RPC connection.
fn event_node(name: &str) -> Result<Option<PathBuf>> {
    for entry in fs::read_dir("/sys/class/input")? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("event") {
            continue;
        }
        let observed = match fs::read_to_string(entry.path().join("device/name")) {
            Ok(name) => name,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if observed.trim_end_matches('\n') == name {
            return Ok(Some(Path::new("/dev/input").join(entry.file_name())));
        }
    }
    Ok(None)
}

/// Wait for a real registration/removal under a finite deadline; all observation is read-only.
fn wait(mut condition: impl FnMut() -> Result<bool>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if condition()? {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "input receiver transition timed out"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Send one native input_event after the guest evdev reader has opened the new device.
fn event(pipe: &mut ChildStdin, kind: u16, code: u16, value: i32) -> Result<()> {
    let mut bytes = vec![0_u8; 16]; // Native timeval; receiver supplies its own event timing.
    bytes.extend(kind.to_le_bytes());
    bytes.extend(code.to_le_bytes());
    bytes.extend(value.to_le_bytes());
    pipe.write_all(&bytes)?;
    Ok(())
}

/// Verify authorization, registration, event delivery and disconnect for each installed receiver route.
pub(crate) fn run() -> Result<()> {
    ensure!(
        Path::new("/etc/airlock-input-test").is_file(),
        "run only in the disposable input test VM"
    );
    let services = [
        ("airlock.InputDevice", "composite"),
        ("qubes.InputKeyboard", "keyboard"),
        ("qubes.InputMouse", "mouse"),
        ("qubes.InputTablet", "tablet"),
    ];
    for service in services
        .iter()
        .map(|(service, _)| *service)
        .chain(["airlock.InputReceiverReady"])
    {
        for (caller, argument) in [("sys-gui", ""), ("dom0", "unexpected")] {
            // Upstream keyboard/mouse/tablet shell adapters check caller identity;
            // qrexec service argument policy is enforced by the native Airlock RPCs.
            if !service.starts_with("airlock.") && !argument.is_empty() {
                continue;
            }
            ensure!(
                !support::run(
                    &mut command(service, caller, argument),
                    None,
                    Duration::from_secs(5)
                )?
                .status
                .success(),
                "{service} accepted an unauthorized request"
            );
        }
    }
    ensure!(
        readiness()?["devices"]
            .as_array()
            .context("devices")?
            .is_empty(),
        "unexpected preexisting forwarded input"
    );
    let mut passed = Vec::new();
    for (index, (service, kind)) in services.iter().enumerate() {
        eprintln!("Testing installed input RPC {service} ({kind})");
        let name = format!("Airlock smoke {index}");
        let full_name = format!("dom0: {name}");
        let errors = PathBuf::from(format!("/tmp/input-receiver-{index}.stderr"));
        let mut child = command(service, "dom0", "")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(fs::File::create(&errors)?)
            .spawn()?;
        let pipe = child.stdin.take().context("receiver protocol pipe")?;
        let mut receiver = Receiver {
            child,
            pipe: Some(pipe),
        };
        receiver
            .pipe
            .as_mut()
            .context("protocol")?
            .write_all(&hello(&name, kind))?;
        wait(|| {
            ensure!(
                receiver.child.try_wait()?.is_none(),
                "{service} exited before registration: {error}",
                error = fs::read_to_string(&errors).unwrap_or_default()
            );
            Ok(event_node(&full_name)?.is_some_and(|node| node.exists()))
        })
        .with_context(|| {
            format!(
                "{service} registration: {error}",
                error = fs::read_to_string(&errors).unwrap_or_default()
            )
        })?;
        let node = event_node(&full_name)?.context("registered evdev node")?;
        let mut events = fs::OpenOptions::new()
            .read(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open(node)?;
        let observation = readiness()?;
        ensure!(
            observation["devices"]
                .as_array()
                .context("devices")?
                .iter()
                .any(|device| device["name"] == full_name
                    && (!matches!(*kind, "keyboard" | "composite") || device["keyboard"] == true)
                    && (*kind == "keyboard" || device["pointer"] == true)),
            "readiness omitted {service}'s registered capability"
        );
        let expected: Vec<(u16, u16, i32)> = match *kind {
            "keyboard" => vec![(1, 30, 1)],
            "mouse" => vec![(2, 0, 17)],
            "composite" => vec![(1, 30, 1), (2, 0, 17)],
            _ => vec![(3, 0, 100)],
        };
        for &(kind, code, value) in &expected {
            event(
                receiver.pipe.as_mut().context("protocol")?,
                kind,
                code,
                value,
            )?;
        }
        event(receiver.pipe.as_mut().context("protocol")?, 0, 0, 0)?; // SYN_REPORT
        let mut bytes = Vec::new();
        wait(|| {
            let mut buffer = [0_u8; 24 * 64];
            match events.read(&mut buffer) {
                Ok(size) => bytes.extend_from_slice(&buffer[..size]),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error.into()),
            }
            Ok(expected.iter().all(|&(kind, code, value)| {
                bytes.chunks_exact(24).any(|event| {
                    u16::from_le_bytes(event[16..18].try_into().unwrap()) == kind
                        && u16::from_le_bytes(event[18..20].try_into().unwrap()) == code
                        && i32::from_le_bytes(event[20..24].try_into().unwrap()) == value
                })
            }))
        })?;
        receiver.pipe.take();
        wait(|| Ok(receiver.child.try_wait()?.is_some()))?;
        ensure!(
            receiver.child.wait()?.success(),
            "{service} failed on disconnect: {error}",
            error = fs::read_to_string(&errors).unwrap_or_default()
        );
        wait(|| Ok(event_node(&full_name)?.is_none()))?;
        ensure!(
            readiness()?["devices"]
                .as_array()
                .context("devices")?
                .is_empty(),
            "disconnected receiver left stale readiness"
        );
        passed.push(service);
    }
    println!(
        "{}",
        json!({"schema":"airlock.input-smoke.v1","status":"passed","services":passed,"event_delivery":true,"disconnect":true,"physical_input_verified":false})
    );
    Ok(())
}
