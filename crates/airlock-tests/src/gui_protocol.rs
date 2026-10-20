//! Explicit C fixture preparation compiles the actual implementations carried by upstream Qubes patches.

use airlock_vm_service::support;
use anyhow::{Context, Result, ensure};
use std::{fs, path::Path, process::Command, time::Duration};

/// Keep added implementation or complete postimage hunk lines while excluding patch headers.
fn code(root: &Path, package: &str, name: &str, postimage: bool) -> Result<String> {
    let patch = fs::read_to_string(root.join(package).join(name))?;
    let mut in_hunk = false;
    let mut result = Vec::new();
    for line in patch.lines() {
        if line.starts_with("@@") {
            in_hunk = true;
        } else if line.starts_with("diff --git") {
            in_hunk = false;
        } else if (postimage && in_hunk && line.starts_with([' ', '+']))
            || (!postimage && line.starts_with('+') && !line.starts_with("+++"))
        {
            result.push(&line[1..]);
        }
    }
    Ok(result.join("\n"))
}

/// Select one actual function or conditional body and reject an incomplete patch rather than compiling a model.
fn block(source: &str, start: &str) -> Result<String> {
    let source = &source[source
        .find(start)
        .with_context(|| format!("missing patch body {start}"))?..];
    let mut depth = 0;
    let mut opened = false;
    for (index, character) in source.char_indices() {
        if character == '{' {
            depth += 1;
            opened = true;
        } else if character == '}' {
            depth -= 1;
            if opened && depth == 0 {
                return Ok(source[..index + 1].to_owned());
            }
        }
    }
    anyhow::bail!("incomplete C body {start}")
}

/// Compile standalone scenario binaries as an explicit prerequisite; Cargo tests never invoke a compiler themselves.
pub fn prepare(root: &Path, output: &Path) -> Result<()> {
    fs::create_dir_all(output)?;
    let maximize = code(
        root,
        "qubes-gui-daemon",
        "0001-gui-daemon-apply-maximize-window-flags.patch",
        false,
    )?;
    let maximize = &maximize[maximize
        .find("const uint32_t maximize_flags")
        .context("maximize implementation")?..];
    let movement = block(
        &code(
            root,
            "qubes-gui-daemon",
            "0002-gui-daemon-apply-interactive-window-move-requests.patch",
            false,
        )?,
        "static void handle_window_moveresize",
    )?;
    let mut branches = Vec::new();
    for name in [
        "0001-gui-agent-forward-minimize-and-maximize-requests.patch",
        "0002-gui-agent-forward-interactive-window-move-requests.patch",
    ] {
        let source = code(root, "qubes-gui-agent-linux", name, true)?;
        let start = source
            .find("} else if (ev->message_type")
            .context("agent event branch")?
            + 2;
        branches.push(block(&source[start..], "else if")?);
    }
    let agent = format!(
        "static void forward(Ghandles *g, XClientMessageEvent *ev) {{ if (0) {{}} {branches} }}",
        branches = branches.join(" ")
    );
    let flags = fs::read_to_string(
        root.join("qubes-gui-common/0001-protocol-add-maximize-window-flags.patch"),
    )?
    .lines()
    .filter(|line| {
        line.starts_with(" #define WINDOW_FLAG_") || line.starts_with("+#define WINDOW_FLAG_")
    })
    .map(|line| line[1..].to_owned())
    .collect::<Vec<_>>()
    .join("\n");
    let wire = code(
        root,
        "qubes-gui-common",
        "0002-protocol-add-interactive-window-move-message.patch",
        false,
    )?;
    let wire = &wire[wire
        .find("struct msg_window_moveresize")
        .context("movement wire struct")?..];
    let protocol = include_str!("../fixtures/gui/protocol.c")
        .replace("@flags@", &flags)
        .replace("@wire@", wire)
        .replace("@move@", &movement)
        .replace("@maximize@", maximize)
        .replace("@agent@", &agent);
    let reconnect = block(
        &code(
            root,
            "qubes-gui-agent-linux",
            "0003-preserve-xorg-across-guivm-restart.patch",
            false,
        )?,
        "Bool xf86_qubes_reconnect(uint32_t domid, Bool connected) {",
    )?;
    let reconnect = include_str!("../fixtures/gui/reconnect.c").replace("@function@", &reconnect);
    let compiler = std::env::var_os("CC").unwrap_or_else(|| "cc".into());
    for (name, source) in [("protocol", protocol), ("reconnect", reconnect)] {
        ensure!(
            !source.contains("@function@") && !source.contains("@agent@"),
            "unresolved C fixture marker"
        );
        let path = output.join(format!("{name}.c"));
        fs::write(&path, source)?;
        let mut command = Command::new(&compiler);
        command.args(["-std=c11", "-Wall", "-Werror"]);
        if name == "reconnect" {
            command.arg("-Wextra");
        }
        support::run(
            command.arg(path).arg("-o").arg(output.join(name)),
            None,
            Duration::from_secs(30),
        )?
        .checked()?;
    }
    Ok(())
}
