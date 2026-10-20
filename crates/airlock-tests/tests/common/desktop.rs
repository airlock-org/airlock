//! Private X11 server ownership for graphical process and trusted window-property scenarios.

use super::tool;
use airlock_tests::dialogue::Dialogue;
use anyhow::Result;
use std::{process::Command, time::Duration};
use x11rb::{connection::Connection, rust_connection::RustConnection};

/// One explicitly prepared virtual display whose native windows belong only to test processes.
pub struct Display {
    // Cancels the owned virtual server and its descendants when the test exits.
    _server: Dialogue,
    // Native X client connection used for property, resource-owner and window lifetime checks.
    connection: RustConnection,
    // Root window of the selected virtual screen.
    root: u32,
    // Child-local display selector; never alters the workstation session environment.
    name: String,
}

impl Display {
    /// Start private Xvfb and connect only after its displayfd readiness frame.
    pub fn new() -> Result<Self> {
        let mut server = Dialogue::spawn(Command::new(tool("Xvfb")?).args([
            "-displayfd",
            "1",
            "-nolisten",
            "tcp",
            "-screen",
            "0",
            "800x600x24",
        ]))?;
        let number = server.reply(Duration::from_secs(5))?;
        let name = format!(":{number}");
        let (connection, screen) = x11rb::connect(Some(&name))?;
        let root = connection.setup().roots[screen].root;
        Ok(Self {
            _server: server,
            connection,
            root,
            name,
        })
    }

    /// Borrow the actual X11 client connection used to create or observe fixture windows.
    pub fn connection(&self) -> &RustConnection {
        &self.connection
    }

    /// Return the selected screen's root, independently of the host display's screen configuration.
    pub fn root(&self) -> u32 {
        self.root
    }

    /// Supply the display only to a selected child process.
    pub fn name(&self) -> &str {
        &self.name
    }
}
