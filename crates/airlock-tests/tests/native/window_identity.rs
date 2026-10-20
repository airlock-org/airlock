//! Native X11 property reads trust the owning client and reject malformed or stale VM identity.

use crate::common::{self, desktop::Display};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::process::Command;
use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ConnectionExt, CreateWindowAux, PropMode, WindowClass},
    wrapper::ConnectionExt as _,
};

impl Display {
    /// Place typed properties on a window owned by this process, then ask the actual native reader.
    fn resolve(&self, properties: Value, provenance: &str, fault: &str) -> Result<Value> {
        let connection = self.connection();
        let window = connection.generate_id()?;
        connection
            .create_window(
                x11rb::COPY_FROM_PARENT as u8,
                window,
                self.root(),
                0,
                0,
                64,
                64,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::new(),
            )?
            .check()?;
        let result = (|| -> Result<Value> {
            for (name, value) in properties.as_object().unwrap() {
                let atom = connection
                    .intern_atom(false, name.as_bytes())?
                    .reply()?
                    .atom;
                if name == "_QUBES_VMNAME" && value.is_string() {
                    let mut bytes = value.as_str().unwrap().as_bytes().to_vec();
                    if fault == "bad-ascii" {
                        bytes = vec![255];
                    }
                    if fault == "oversized" {
                        bytes = vec![b'a'; 300];
                    }
                    let kind = if fault == "wrong-type" {
                        AtomEnum::CARDINAL
                    } else {
                        AtomEnum::STRING
                    };
                    connection
                        .change_property8(PropMode::REPLACE, window, atom, kind, &bytes)?
                        .check()?;
                } else if let Some(number) = value.as_u64() {
                    if name == "_QUBES_LABEL" && number <= 255 && fault != "wrong-format" {
                        connection
                            .change_property8(
                                PropMode::REPLACE,
                                window,
                                atom,
                                AtomEnum::CARDINAL,
                                &[number as u8],
                            )?
                            .check()?;
                    } else {
                        connection
                            .change_property32(
                                PropMode::REPLACE,
                                window,
                                atom,
                                AtomEnum::CARDINAL,
                                &[number as u32],
                            )?
                            .check()?;
                    }
                } else {
                    let kind = if name == "_QUBES_VMNAME" {
                        AtomEnum::CARDINAL
                    } else {
                        AtomEnum::STRING
                    };
                    connection
                        .change_property8(PropMode::REPLACE, window, atom, kind, b"invalid")?
                        .check()?;
                }
            }
            let destroyed = provenance == "destroyed" || fault == "destroyed";
            if destroyed {
                connection.destroy_window(window)?.check()?;
            }
            connection.flush()?;
            connection.get_input_focus()?.reply()?;
            let trusted = std::env::current_exe()?;
            let guid = if provenance == "trusted" {
                trusted.as_path()
            } else {
                std::path::Path::new("/nonexistent-untrusted-daemon")
            };
            common::json(
                Command::new(common::binary(
                    "AIRLOCK_SYS_GUI_BIN",
                    "airlock-sys-gui-service",
                ))
                .args(["window-identity", "--guid"])
                .arg(guid)
                .arg(format!("{window:x}"))
                .env("DISPLAY", self.name()),
                None,
            )
        })();
        // Destroy may already have run for a stale-window scenario; ignore that X error during cleanup.
        let _ = connection.destroy_window(window)?.check();
        connection.flush()?;
        result
    }
}

/// Construct the exact daemon-owned property encoding used by Qubes application windows.
fn properties() -> Value {
    json!({"_QUBES_VMNAME": "usr-home", "_QUBES_LABEL": 6, "_QUBES_LABEL_COLOR": 0x3465a4})
}

/// Trusted daemon names and custom label IDs retain identity, while arbitrary local windows cannot impersonate a VM.
#[test]
#[ignore = "requires explicitly prepared Xvfb"]
fn desktop_x11_identity_checks_resource_owner_and_property_types() -> Result<()> {
    let display = Display::new()?;
    let unknown = json!({"kind": "unknown", "name": "Unknown VM", "color": null});
    for name in ["usr-home", "usr-browser", "a", &"a".repeat(31)] {
        let mut fields = properties();
        fields["_QUBES_VMNAME"] = json!(name);
        assert_eq!(
            display.resolve(fields, "trusted", "")?,
            json!({"kind": "vm", "name": name, "color": "#3465a4"})
        );
    }
    for name in [
        json!(null),
        json!(""),
        json!("1vm"),
        json!("a".repeat(32)),
        json!("<b>vm</b>"),
        json!("vm\n"),
        json!("vm\0"),
        json!("a b"),
    ] {
        let mut fields = properties();
        fields["_QUBES_VMNAME"] = name;
        assert_eq!(display.resolve(fields, "trusted", "")?, unknown);
    }
    for label in [json!(null), json!(0), json!(256), json!(true), json!("6")] {
        let mut fields = properties();
        fields["_QUBES_LABEL"] = label;
        assert_eq!(display.resolve(fields, "trusted", "")?, unknown);
    }
    for label in [9, 255] {
        let mut fields = properties();
        fields["_QUBES_LABEL"] = json!(label);
        fields["_QUBES_LABEL_COLOR"] = json!(0x6b7a3d);
        assert_eq!(
            display.resolve(fields.clone(), "trusted", "")?,
            json!({"kind": "vm", "name": "usr-home", "color": "#6b7a3d"})
        );
        assert_eq!(display.resolve(fields, "untrusted", "")?, unknown);
    }
    for color in [
        json!(null),
        json!(-1),
        json!(0x1000000),
        json!(true),
        json!("red; width:10000px"),
    ] {
        let mut fields = properties();
        fields["_QUBES_LABEL_COLOR"] = color;
        assert_eq!(
            display.resolve(fields, "trusted", "")?,
            json!({"kind": "vm", "name": "usr-home", "color": null})
        );
    }
    for fault in [
        "destroyed",
        "wrong-type",
        "wrong-format",
        "oversized",
        "bad-ascii",
    ] {
        assert_eq!(display.resolve(properties(), "trusted", fault)?, unknown);
    }
    assert_eq!(display.resolve(properties(), "untrusted", "")?, unknown);
    assert_eq!(display.resolve(properties(), "destroyed", "")?, unknown);
    assert_eq!(
        display.resolve(json!({}), "trusted", "")?,
        json!({"kind": "absent", "name": "Unknown VM", "color": null})
    );
    assert_eq!(display.resolve(json!({}), "destroyed", "")?, unknown);
    assert_eq!(
        display.resolve(json!({}), "untrusted", "")?,
        json!({"kind": "local", "name": "sys-gui", "color": null})
    );
    ensure!(
        display.connection().get_input_focus()?.reply().is_ok(),
        "reader disconnected the X server"
    );
    Ok(())
}
