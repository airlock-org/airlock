//! Read-only observation of the host's existing Xen and Qubes control stack.

use serde_json::{Value, json};
use std::{env, fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

/// Find an executable without invoking a shell or changing host state.
fn available(name: &str) -> bool {
    env::split_paths(&env::var_os("PATH").unwrap_or_default()).any(|directory| {
        fs::metadata(directory.join(name))
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    })
}

/// Capture a probe's output; failed probes have no observation, rather than a false zero.
fn probe(program: &str, args: &[&str], accept_failure: bool) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !accept_failure && !output.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned(),
    )
}

/// Collect one state snapshot using only filesystem reads and upstream query commands.
fn observation() -> Value {
    let capabilities = fs::read_to_string("/proc/xen/capabilities").unwrap_or_default();
    let version = probe("xl", &["info"], false).and_then(|text| {
        text.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            (key.trim() == "xen_version").then(|| value.trim().to_owned())
        })
    });
    let domains = probe(
        "qvm-ls",
        &["--raw-data", "--fields", "NAME", "--no-spinner"],
        false,
    );
    let services = [
        "qubes-db-dom0.service",
        "qubesd.service",
        "qubes-qrexec-policy-daemon.service",
        "qubes-core.service",
        "xenstored.service",
        "libvirtd.service",
    ]
    .map(|name| {
        let state = probe("systemctl", &["is-active", name], true)
            .filter(|state| !state.is_empty())
            .unwrap_or_else(|| "unknown".into());
        json!({"name": name, "active": state == "active", "state": state})
    });
    json!({
        "schema": "airlock.dom0-observation.v1",
        "host": {
            "hostname": probe("hostname", &[], false).unwrap_or_default(),
            "kernel": probe("uname", &["-r"], false).unwrap_or_default(),
            "architecture": probe("uname", &["-m"], false).unwrap_or_default(),
            "virtualization": probe("systemd-detect-virt", &[], true).unwrap_or_default(),
        },
        "xen": {
            "proc_xen": Path::new("/proc/xen").exists(),
            "control_domain": capabilities.split_whitespace().any(|word| word == "control_d"),
            "capabilities": capabilities.trim_end(),
            "xl_available": available("xl"),
            "xen_version": version.unwrap_or_default(),
        },
        "services": services,
        "qubes": {
            "qvm_ls_available": available("qvm-ls"),
            "qvm_run_available": available("qvm-run"),
            "domains_count": domains.map(|text| text.lines().count()),
        },
    })
}

fn main() {
    println!("{report}", report = observation());
}
