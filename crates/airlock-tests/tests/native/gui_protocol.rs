//! Rust asserts actual compiled Qubes GUI protocol behavior; preparation compiles the patch implementations explicitly.

use crate::common;
use anyhow::Result;
use serde_json::{Value, json};
use std::{path::PathBuf, process::Command};

/// Locate explicitly prepared C scenarios next to the native test runner by default.
fn fixture(name: &str) -> PathBuf {
    std::env::var_os("AIRLOCK_GUI_C_FIXTURES")
        .map(PathBuf::from)
        .unwrap_or_else(|| common::peer().parent().unwrap().join("gui-c-fixtures"))
        .join(name)
}

/// Execute a finite protocol request through the actual patched C implementation with inert X11 callbacks.
fn protocol(arguments: &[u32], operation: &str) -> Result<Value> {
    common::json(
        Command::new(fixture("protocol"))
            .arg(operation)
            .args(arguments.iter().map(u32::to_string)),
        None,
    )
}

/// Maximize changes both axes atomically and clears only the requested flags.
#[test]
fn qubes_gui_maximize_preserves_atomic_axis_changes() -> Result<()> {
    for (setting, clearing, expected) in [
        (24, 0, json!([[1, 3, 4, 0]])),
        (0, 24, json!([[0, 3, 4, 0]])),
        (8, 0, json!([[1, 3, 0, 0]])),
        (0, 16, json!([[0, 4, 0, 0]])),
        (0, 0, json!([])),
    ] {
        assert_eq!(protocol(&[setting, clearing], "maximize")?, expected);
    }
    Ok(())
}

/// Interactive movement requires a live trusted pointer button and an ordinary mapped window.
#[test]
fn qubes_gui_window_move_checks_pointer_and_window_state() -> Result<()> {
    for (arguments, expected) in [
        ([8, 1, 256, 1, 0, 1], json!([[120, 240, 8, 1]])),
        ([8, 5, 4096, 1, 0, 1], json!([[120, 240, 8, 5]])),
        ([7, 1, 256, 1, 0, 1], json!([])),
        ([8, 0, 256, 1, 0, 1], json!([])),
        ([8, 6, 256, 1, 0, 1], json!([])),
        ([8, 1, 0, 1, 0, 1], json!([])),
        ([8, 1, 256, 0, 0, 1], json!([])),
        ([8, 1, 256, 1, 1, 1], json!([])),
        ([8, 1, 256, 1, 0, 0], json!([])),
    ] {
        assert_eq!(protocol(&arguments, "move")?, expected);
    }
    Ok(())
}

/// The agent forwards only known windows and negotiated movement protocol, preserving minimize support.
#[test]
fn qubes_gui_agent_checks_window_identity_and_protocol_support() -> Result<()> {
    for (arguments, expected) in [
        ([0x10009, 1, 2, 32], json!([[8, 1, 2, 0]])),
        ([0x10008, 1, 2, 32], json!([])),
        ([0x10009, 0, 2, 32], json!([])),
        ([0x10009, 1, 2, 16], json!([])),
        ([0x10009, 1, 5, 32], json!([[4, 0, 1, 0]])),
        ([0x10009, 0, 5, 32], json!([])),
    ] {
        assert_eq!(protocol(&arguments, "agent")?, expected);
    }
    Ok(())
}

/// Reconnection preserves the original virtual addresses and pixels through failed and successful regranting.
#[test]
fn qubes_xorg_reconnection_preserves_application_memory() -> Result<()> {
    common::run(&mut Command::new(fixture("reconnect")), None)?.checked()?;
    Ok(())
}
