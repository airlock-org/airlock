//! The GTK setup window keeps dismissal within one session and reopens only on explicit user action.

use crate::common::{self, desktop::Display, peers::Peers};
use airlock_tests::dialogue::Dialogue;
use anyhow::{Context, Result, ensure};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::Command,
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ClientMessageEvent, ConnectionExt, EventMask},
};

/// A private display and bus contain the real GTK application without activating the workstation desktop.
struct Session {
    // Holds the virtual X server and native observation connection through application cleanup.
    display: Display,
    // Cancels the isolated bus and any accidental descendants when the scenario finishes.
    _bus: Dialogue,
    // Private RPC replies, home directory and invocation evidence.
    peers: Peers,
    // Bus address supplied only to the selected application children.
    address: String,
}

impl Session {
    /// Create a bus without service directories and a harmless setup-status RPC peer.
    fn new(status: &str) -> Result<Self> {
        let display = Display::new()?;
        let mut peers = Peers::new()?;
        fs::create_dir(peers.path("runtime"))?;
        fs::set_permissions(peers.path("runtime"), fs::Permissions::from_mode(0o700))?;
        fs::write(peers.path("status"), status)?;
        peers.rules(
            "qvm-run",
            json!([{"contains": "airlock.SetupStatus", "stdout_file": "status"}]),
        )?;
        let configuration = peers.path("bus.conf");
        fs::write(
            &configuration,
            r#"<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow own="*"/>
    <allow send_destination="*"/>
    <allow receive_sender="*"/>
  </policy>
</busconfig>
"#,
        )?;
        let mut bus = Dialogue::spawn(
            Command::new(common::tool("dbus-daemon")?)
                .arg(format!(
                    "--config-file={path}",
                    path = configuration.display()
                ))
                .args(["--nofork", "--print-address=1"]),
        )?;
        let address = String::from_utf8(bus.line(Duration::from_secs(5))?)?;
        ensure!(
            address.starts_with("unix:") && bus.running()?,
            "private session bus did not start"
        );
        Ok(Self {
            display,
            _bus: bus,
            peers,
            address,
        })
    }

    /// Build the actual GTK application's child environment using inert RPC and isolated desktop services.
    fn command(&self, arguments: &[&str]) -> Result<Command> {
        let mut command = self
            .peers
            .command(common::binary("AIRLOCK_SETUP_BIN", "airlock-sys-gui-setup"))?;
        command
            .args(arguments)
            .env("DISPLAY", self.display.name())
            .env("GDK_BACKEND", "x11")
            .env("GSK_RENDERER", "cairo")
            .env("GTK_USE_PORTAL", "0")
            .env("GTK_A11Y", "none")
            .env("GSETTINGS_BACKEND", "memory")
            .env("GIO_USE_VFS", "local")
            .env("DBUS_SESSION_BUS_ADDRESS", &self.address)
            .env("XDG_RUNTIME_DIR", self.peers.path("runtime"))
            .env("HOME", self.peers.path(""))
            .env("AIRLOCK_QVM_RUN", self.peers.path("bin/qvm-run"));
        Ok(command)
    }

    /// Observe application windows using X11 metadata rather than application implementation state.
    fn windows(&self) -> Result<Vec<u32>> {
        let connection = self.display.connection();
        let mut windows = Vec::new();
        for window in connection
            .query_tree(self.display.root())?
            .reply()?
            .children
        {
            let Ok(property) = connection
                .get_property(false, window, AtomEnum::WM_NAME, AtomEnum::ANY, 0, 1024)?
                .reply()
            else {
                continue;
            };
            if property.value.starts_with(b"AirlockOS Setup") {
                windows.push(window);
            }
        }
        Ok(windows)
    }

    /// Wait under a deadline for the application's first actual X11 window.
    fn window(&self) -> Result<u32> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(window) = self.windows()?.first() {
                return Ok(*window);
            }
            ensure!(Instant::now() < deadline, "setup window did not appear");
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// Send the same close request as a window manager and wait for its observable disappearance.
    fn close(&self, window: u32) -> Result<()> {
        let connection = self.display.connection();
        let protocol = connection
            .intern_atom(false, b"WM_PROTOCOLS")?
            .reply()?
            .atom;
        let delete = connection
            .intern_atom(false, b"WM_DELETE_WINDOW")?
            .reply()?
            .atom;
        let event = ClientMessageEvent::new(32, window, protocol, [delete, 0, 0, 0, 0]);
        connection
            .send_event(false, window, EventMask::NO_EVENT, event)?
            .check()?;
        connection.flush()?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.windows()?.is_empty() {
            ensure!(
                Instant::now() < deadline,
                "closed setup window remained visible"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        Ok(())
    }
}

/// A completed setup leaves the desktop undisturbed even on automatic session startup.
#[test]
#[ignore = "requires explicitly prepared GTK setup binary, Xvfb and D-Bus"]
fn desktop_completed_setup_does_not_open_automatically() -> Result<()> {
    let session = Session::new("complete")?;
    common::run(&mut session.command(&["--auto"])?, None)?.checked()?;
    assert!(session.windows()?.is_empty());
    let calls = session.peers.calls()?;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0]["args"].as_array().context("RPC arguments")?.last(),
        Some(&json!("airlock.SetupStatus"))
    );
    Ok(())
}

/// Dismissing one session's setup keeps automatic launch quiet while a manual launch reopens the existing application.
#[test]
#[ignore = "requires explicitly prepared GTK setup binary, Xvfb and D-Bus"]
fn desktop_setup_dismissal_is_session_local_and_manual_launch_reopens() -> Result<()> {
    let session = Session::new("incomplete")?;
    let mut application = Dialogue::spawn(&mut session.command(&[])?)?;
    session.close(session.window()?)?;
    assert!(application.running()?);
    common::run(&mut session.command(&["--auto"])?, None)?.checked()?;
    assert!(session.windows()?.is_empty());
    common::run(&mut session.command(&[])?, None)?.checked()?;
    session.window()?;
    Ok(())
}
