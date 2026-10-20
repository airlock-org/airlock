//! Drive the production Qubes event listener through real local sockets and fixed inert systemd commands.

use crate::common::{self, peers::Peers};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{Read, Write},
    net::Shutdown,
    os::unix::{
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

/// Mutable upstream observations controlled by the test independently of subscribed event data.
struct State {
    // Qubes' current activity state returned after subscription confirmation.
    power: String,
    // Keep state-query sockets open to exercise shutdown cancellation.
    hang: bool,
    // Exact admin requests used to verify subscription ordering and bounded cancellation.
    requests: Vec<Vec<u8>>,
}

/// One owned native listener, its upstream API server and private systemd command namespace.
struct Listener {
    // Owns sockets, runtime lock files, process logs and inert command configuration.
    peers: Peers,
    // Bubblewrap leader owns the production service and relays its exit code.
    child: Child,
    // Namespace arguments retained for a competing observer sharing the same private runtime locks.
    command: Command,
    // Connections handed from the API server to scenario code for fragmented event writes.
    connections: mpsc::Receiver<UnixStream>,
    // Authoritative state-query replies, never inferred from event delivery timing.
    state: Arc<Mutex<State>>,
    // Cancels the server accept loop during cleanup, including after failed assertions.
    stopping: Arc<AtomicBool>,
    // Server is joined before its fixture directory is removed.
    worker: Option<thread::JoinHandle<Result<()>>>,
}

/// Encode one complete upstream event, including the terminating empty keyword field.
fn event(subject: &str, name: &str) -> Vec<u8> {
    format!("1\0{subject}\0{name}\0\0").into_bytes()
}

impl Listener {
    /// Subscribe through an actual Unix socket while sharing only a private input lock directory.
    fn new() -> Result<Self> {
        let mut peers = Peers::new()?;
        peers.rules(
            "systemctl",
            json!([{"exists":"systemctl-failure","exit":1},{}]),
        )?;
        fs::create_dir(peers.path("run"))?;
        let server = UnixListener::bind(peers.path("qubesd.sock"))?;
        server.set_nonblocking(true)?;
        let state = Arc::new(Mutex::new(State {
            power: "Halted".into(),
            hang: false,
            requests: Vec::new(),
        }));
        let stopping = Arc::new(AtomicBool::new(false));
        let (send, connections) = mpsc::channel();
        let observed = Arc::clone(&state);
        let stop = Arc::clone(&stopping);
        let worker = thread::spawn(move || -> Result<()> {
            let mut pending = Vec::new();
            while !stop.load(Ordering::Relaxed) {
                let (mut peer, _) = match server.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                peer.set_read_timeout(Some(Duration::from_secs(2)))?;
                peer.set_write_timeout(Some(Duration::from_secs(2)))?;
                let mut request = Vec::new();
                let mut byte = [0];
                loop {
                    peer.read_exact(&mut byte)?;
                    request.push(byte[0]);
                    ensure!(request.len() <= 1024, "unbounded API request");
                    if byte[0] == 0 {
                        break;
                    }
                }
                let mut state = observed.lock().unwrap();
                state.requests.push(request.clone());
                if request == b"admin.Events+ dom0 name dom0\0" {
                    if send.send(peer).is_err() {
                        break;
                    }
                } else if request == b"admin.vm.CurrentState+ dom0 name sys-gui\0" {
                    if state.hang {
                        pending.push(peer);
                    } else {
                        peer.write_all(
                            format!(
                                "0\0mem=0 mem_static_max=0 cputime=0 power_state={power}",
                                power = state.power
                            )
                            .as_bytes(),
                        )?;
                        peer.shutdown(Shutdown::Both)?;
                    }
                } else {
                    anyhow::bail!("unexpected admin API request: {request:?}");
                }
            }
            Ok(())
        });
        let mut command = peers.command("bwrap")?;
        command.args([
            "--info-fd",
            "1",
            "--die-with-parent",
            "--unshare-user",
            "--uid",
            "0",
            "--gid",
            "0",
            "--ro-bind",
            "/",
            "/",
            "--tmpfs",
            "/run",
        ]);
        if std::path::Path::new("/run/current-system").exists() {
            command.args(["--ro-bind", "/run/current-system", "/run/current-system"]);
        }
        command
            .arg("--bind")
            .arg(peers.path(""))
            .arg(peers.path(""))
            .arg("--bind")
            .arg(peers.path("run"))
            .arg("/run/airlock")
            .args(["--proc", "/proc", "--dev", "/dev"])
            .arg(common::binary("AIRLOCK_DOM0_BIN", "airlock-dom0-service"))
            .args(["input", "events", "--target", "sys-gui"])
            .env("AIRLOCK_INPUT_QUBES_SOCKET", peers.path("qubesd.sock"))
            .env("AIRLOCK_INPUT_SYSTEMCTL", peers.path("bin/systemctl"))
            .env("TMPDIR", peers.path(""))
            .stdin(Stdio::null())
            .stdout(File::create(peers.path("namespace.json"))?)
            .stderr(File::create(peers.path("stderr"))?)
            .process_group(0);
        let child = command.spawn()?;
        let listener = Self {
            peers,
            child,
            command,
            connections,
            state,
            stopping,
            worker: Some(worker),
        };
        listener.calls(1, true, Duration::from_secs(4))?;
        Ok(listener)
    }

    /// Obtain the next subscribed event stream with a bounded deadline.
    fn connection(&self) -> Result<UnixStream> {
        Ok(self.connections.recv_timeout(Duration::from_secs(8))?)
    }

    /// Start a second observer in the same fixture without reusing the first process's output handles.
    fn competitor(&self) -> Command {
        let mut command = Command::new(self.command.get_program());
        command.args(self.command.get_args());
        for (key, value) in self.command.get_envs() {
            if let Some(value) = value {
                command.env(key, value);
            } else {
                command.env_remove(key);
            }
        }
        command
    }

    /// Wait for the requested observable systemd effects, keeping startup cleanup separate.
    fn calls(&self, count: usize, startup: bool, timeout: Duration) -> Result<Vec<Value>> {
        let deadline = Instant::now() + timeout;
        loop {
            let mut calls: Vec<_> = self
                .peers
                .calls()?
                .into_iter()
                .map(|call| call["args"].clone())
                .collect();
            if !startup && !calls.is_empty() {
                calls.remove(0);
            }
            if calls.len() >= count {
                return Ok(calls);
            }
            ensure!(
                Instant::now() < deadline,
                "missing input systemd operations: {calls:?}; {stderr}",
                stderr = fs::read_to_string(self.peers.path("stderr"))?
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Signal the service itself so bubblewrap can relay a clean, intentional shutdown.
    fn stop(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        let info: Value = self.peers.read("namespace.json")?;
        let raw = info["child-pid"]
            .as_i64()
            .context("namespace child PID")?
            .try_into()?;
        rustix::process::kill_process(
            rustix::process::Pid::from_raw(raw).context("invalid child PID")?,
            rustix::process::Signal::TERM,
        )?;
        common::until(|| Ok(self.child.try_wait()?.is_some()))?;
        ensure!(
            self.child.wait()?.success(),
            "input listener failed: {stderr}",
            stderr = fs::read_to_string(self.peers.path("stderr"))?
        );
        Ok(())
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if self.stop().is_err() {
            let _ = rustix::process::kill_process_group(
                rustix::process::Pid::from_child(&self.child),
                rustix::process::Signal::KILL,
            );
            let _ = self.child.wait();
        }
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Fragmented subscription confirmation checks existing activity before deciding whether to reconnect.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_input_subscription_tracks_existing_activity_and_target_events() -> Result<()> {
    let mut listener = Listener::new()?;
    listener.state.lock().unwrap().power = "Running".into();
    let mut peer = listener.connection()?;
    for byte in event("", "connection-established") {
        peer.write_all(&[byte])?;
    }
    let calls = listener.calls(1, false, Duration::from_secs(4))?;
    assert_eq!(
        calls[0],
        json!(["--no-block", "start", "airlock-gui-input-connect.service"])
    );
    assert!(listener.state.lock().unwrap().requests[0].starts_with(b"admin.Events+"));
    peer.write_all(&event("usr-home", "domain-start"))?;
    for name in ["domain-shutdown", "domain-stopped", "domain-start-failed"] {
        peer.write_all(&event("sys-gui", name))?;
    }
    let calls = listener.calls(4, false, Duration::from_secs(4))?;
    for call in &calls[1..] {
        assert_eq!(
            *call,
            json!([
                "--no-block",
                "stop",
                "airlock-gui-input-connect.service",
                "airlock-gui-input.target"
            ])
        );
    }
    fs::write(listener.peers.path("systemctl-failure"), "")?;
    peer.write_all(&event("sys-gui", "domain-start"))?;
    peer.write_all(&event("sys-gui", "domain-stopped"))?;
    assert_eq!(
        listener.calls(6, false, Duration::from_secs(4))?[5][1],
        "stop"
    );
    fs::remove_file(listener.peers.path("systemctl-failure"))?;
    listener.stop()?;
    Ok(())
}

/// Disconnection stops stale senders, and a new subscription respects every upstream active power state.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_input_disconnect_releases_senders_and_rechecks_activity() -> Result<()> {
    let mut listener = Listener::new()?;
    let mut peer = listener.connection()?;
    peer.write_all(&event("", "connection-established"))?;
    assert_eq!(
        listener.calls(1, false, Duration::from_secs(4))?[0][1],
        "stop"
    );
    drop(peer);
    assert_eq!(
        listener.calls(2, false, Duration::from_secs(4))?[1][1],
        "stop"
    );
    let mut peer = listener.connection()?;
    for (index, power) in ["Paused", "Transient", "Starting", "Halting", "Suspended"]
        .into_iter()
        .enumerate()
    {
        listener.state.lock().unwrap().power = power.into();
        peer.write_all(&event("", "connection-established"))?;
        assert_eq!(
            listener.calls(index + 3, false, Duration::from_secs(4))?[index + 2][1],
            "start"
        );
    }
    listener.stop()?;
    Ok(())
}

/// Invalid framing cannot start input, and a silent subscription is cancelled before retrying.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_input_rejects_malformed_and_silent_subscriptions() -> Result<()> {
    for invalid in [
        b"0\0unexpected".to_vec(),
        [b"1\0".as_slice(), &vec![b'x'; 65536]].concat(),
        b"1\0\xff\0start\0\0".to_vec(),
    ] {
        let mut listener = Listener::new()?;
        listener.connection()?.write_all(&invalid)?;
        assert!(
            listener
                .calls(1, false, Duration::from_secs(4))?
                .iter()
                .all(|call| call[1] == "stop")
        );
        listener.stop()?;
    }
    let mut listener = Listener::new()?;
    let _silent = listener.connection()?;
    assert_eq!(
        listener.calls(1, false, Duration::from_secs(12))?[0][1],
        "stop"
    );
    listener.state.lock().unwrap().power = "Running".into();
    listener
        .connection()?
        .write_all(&event("", "connection-established"))?;
    assert_eq!(
        listener.calls(2, false, Duration::from_secs(4))?[1][1],
        "start"
    );
    listener.stop()?;
    Ok(())
}

/// Shutdown interrupts an incomplete current-state reply rather than waiting for its normal timeout.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_input_shutdown_cancels_stalled_state_query() -> Result<()> {
    let mut listener = Listener::new()?;
    listener.state.lock().unwrap().hang = true;
    listener
        .connection()?
        .write_all(&event("", "connection-established"))?;
    common::until(|| {
        Ok(listener
            .state
            .lock()
            .unwrap()
            .requests
            .iter()
            .any(|request| request.starts_with(b"admin.vm.CurrentState+")))
    })?;
    let started = Instant::now();
    listener.stop()?;
    assert!(started.elapsed() < Duration::from_secs(2));
    Ok(())
}

/// A second listener cannot disconnect or replace the first listener's live input ownership.
#[test]
#[ignore = "requires explicitly prepared bubblewrap and unprivileged user namespaces"]
fn namespace_input_observer_contention_cannot_release_owned_senders() -> Result<()> {
    let mut listener = Listener::new()?;
    let mut peer = listener.connection()?;
    peer.write_all(&event("sys-gui", "domain-start"))?;
    listener.calls(1, false, Duration::from_secs(4))?;
    let competitor = common::run(&mut listener.competitor(), None)?;
    assert!(!competitor.status.success());
    assert!(String::from_utf8_lossy(&competitor.stderr).contains("lock"));
    assert_eq!(listener.calls(1, false, Duration::from_secs(4))?.len(), 1);
    peer.write_all(&event("sys-gui", "domain-stopped"))?;
    assert_eq!(
        listener.calls(2, false, Duration::from_secs(4))?[1][1],
        "stop"
    );
    listener.stop()?;
    Ok(())
}
