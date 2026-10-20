//! Structured PCI observation and exact unassignment through upstream Qubes' local admin API.
//! Decoding and snapshot completeness are trusted; device selection executes verified Rust.

use super::{call, properties};
use airlock_deployment_core::pci::{Device, Kind, Observation, UnassignRequest, Usage};
use airlock_nix_core::identity::VmName;
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, fs, path::Path};

/// Encode an exact virtual device as Qubes' admin argument, preserving wildcard device IDs.
fn argument(backend: &str, port: &str, identity: &str) -> String {
    format!("{backend}:{port}:{identity}")
        .replace(':', "+")
        .replace("?******", "_______")
        .replace('*', "_")
}

/// Refuse ambiguous transport encodings instead of resolving them to another resource.
fn identity(value: &str, wildcard: bool) -> Result<&str> {
    ensure!(
        !value.is_empty()
            && value.len() <= 1024
            && (wildcard || value != "*")
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b":.*?-".contains(&b)),
        "invalid PCI device identity"
    );
    Ok(value)
}

/// Physical PCI ports are canonical Qubes BDFs; a whole-port wildcard is explicit.
fn port(value: &str, wildcard: bool) -> Result<&str> {
    let b = value.as_bytes();
    ensure!(
        (wildcard && value == "*")
            || (b.len() == 7
                && b[2] == b'_'
                && b[5] == b'.'
                && (b'0'..=b'7').contains(&b[6])
                && [0, 1, 3, 4].into_iter().all(|i| b[i].is_ascii_hexdigit())),
        "invalid PCI port"
    );
    Ok(value)
}

/// Decode every named frontend row, checking the head against all identity properties.
fn usages(reply: &str, owner: &VmName, kind: Kind) -> Result<Vec<Usage>> {
    let mut uses = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for line in reply.lines() {
        let (head, rest) = line.split_once(' ').context("invalid PCI usage row")?;
        let fields = properties(rest)?;
        let field = |name: &str| {
            fields
                .get(name)
                .map(String::as_str)
                .context("missing PCI usage identity")
        };
        let backend: VmName = field("backend_domain")?.parse()?;
        let port = port(field("port_id")?, true)?;
        let identity = identity(field("device_id")?, true)?;
        ensure!(field("devclass")? == "pci", "non-PCI usage row");
        ensure!(
            fields
                .get("frontend_domain")
                .is_none_or(|v| v == owner.as_str()),
            "PCI frontend disagrees with named observation"
        );
        ensure!(
            matches!(
                field("mode")?,
                "manual" | "required" | "auto-attach" | "ask-to-attach"
            ),
            "unsupported PCI assignment mode"
        );
        ensure!(
            head == argument(backend.as_str(), port, identity) && seen.insert(head.to_owned()),
            "conflicting or duplicate PCI usage identity"
        );
        uses.push(Usage {
            owner: owner.as_str().into(),
            backend: backend.as_str().into(),
            port: port.into(),
            identity: identity.into(),
            mode: field("mode")?.into(),
            options: fields
                .iter()
                .filter_map(|(key, value)| {
                    key.strip_prefix('_')
                        .map(|key| airlock_deployment_core::devices::OptionValue {
                            name: key.into(),
                            value: value.clone(),
                        })
                })
                .collect(),
            kind,
        });
    }
    Ok(uses)
}

/// Decode authoritative Available identities; missing fields never become wildcard ownership.
fn available(reply: &str) -> Result<BTreeMap<String, String>> {
    let mut devices = BTreeMap::new();
    for line in reply.lines() {
        let (head, rest) = line.split_once(' ').context("invalid PCI Available row")?;
        let (raw_port, raw_identity) = head
            .split_once(':')
            .context("missing PCI Available identity")?;
        let port = port(raw_port, false)?;
        let identity = identity(raw_identity, false)?;
        let fields = properties(rest)?;
        for (key, expected) in [
            ("backend_domain", "dom0"),
            ("devclass", "pci"),
            ("port_id", port),
            ("device_id", identity),
        ] {
            ensure!(
                fields.get(key).is_some_and(|v| v == expected),
                "inconsistent PCI Available identity"
            );
        }
        ensure!(
            devices.insert(port.into(), identity.into()).is_none(),
            "duplicate PCI Available port"
        );
    }
    Ok(devices)
}

/// Join all sysfs functions to the independently reported Qubes hardware identity.
fn devices(root: &Path, available: &BTreeMap<String, String>) -> Result<Vec<Device>> {
    let mut devices: BTreeMap<String, Device> = BTreeMap::new();
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let raw_class = fs::read_to_string(path.join("class"))?;
        let class = u32::from_str_radix(
            raw_class
                .trim()
                .strip_prefix("0x")
                .context("invalid PCI class")?,
            16,
        )?;
        ensure!(class < 0x1000000, "invalid PCI class width");
        let address = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("invalid PCI address")?;
        let Some(short) = address.strip_prefix("0000:") else {
            ensure!(class / 65536 != 2, "unsupported network PCI segment");
            continue;
        };
        let spelling = short.replace(':', "_");
        let port = port(&spelling, false)?;
        let identity = available.get(port);
        ensure!(
            identity.is_some() || class / 65536 != 2,
            "network PCI function missing from Qubes inventory"
        );
        ensure!(
            devices
                .insert(
                    port.into(),
                    Device {
                        port: port.into(),
                        identity: identity.cloned().unwrap_or_default(),
                        class
                    }
                )
                .is_none(),
            "duplicate sysfs PCI port"
        );
    }
    ensure!(
        available.keys().all(|port| devices.contains_key(port)),
        "Qubes PCI device missing from sysfs"
    );
    Ok(devices.into_values().collect())
}

/// Read all registered frontends, including undeclared VMs, before making an ownership decision.
/// An API failure is an incomplete observation, never an empty assignment list.
pub(crate) fn observe() -> Result<Observation> {
    snapshot(Path::new("/sys/bus/pci/devices"), |domain, method| {
        call(domain, method, "", &[])
    })
}

/// Share the production snapshot path with inert API peers; only the transport is replaceable.
fn snapshot(
    root: &Path,
    mut query: impl FnMut(&VmName, &'static str) -> Result<Vec<u8>>,
) -> Result<Observation> {
    let dom0: VmName = "dom0".parse()?;
    let inventory = query(&dom0, "admin.vm.List")?;
    let names = domains(std::str::from_utf8(&inventory)?)?;
    let raw = query(&dom0, "admin.vm.device.pci.Available")?;
    let devices = devices(root, &available(std::str::from_utf8(&raw)?)?)?;
    let mut uses = Vec::new();
    for name in names {
        for (method, kind) in [
            ("admin.vm.device.pci.Assigned", Kind::Assigned),
            ("admin.vm.device.pci.Attached", Kind::Attached),
        ] {
            let reply = query(&name, method)?;
            uses.extend(usages(std::str::from_utf8(&reply)?, &name, kind)?);
        }
    }
    Ok(Observation { devices, uses })
}

/// Reject duplicate, malformed or empty inventories; a partial name list cannot prove no foreign use.
fn domains(reply: &str) -> Result<Vec<VmName>> {
    let mut names = std::collections::BTreeSet::new();
    for line in reply.lines() {
        let name: VmName = line
            .split_once(' ')
            .context("invalid Qubes domain inventory")?
            .0
            .parse()?;
        ensure!(names.insert(name), "duplicate Qubes domain inventory row");
    }
    ensure!(
        names.iter().any(|n| n.as_str() == "dom0"),
        "incomplete Qubes domain inventory"
    );
    Ok(names.into_iter().collect())
}

/// The only deployment handoff mutation takes the opaque verified assignment directly.
pub(crate) fn unassign(request: UnassignRequest<'_>) -> Result<()> {
    unassign_with(request, |domain, argument| {
        call(domain, "admin.vm.device.pci.Unassign", argument, &[]).map(|_| ())
    })
}

/// Only encoding and the upstream call are outside the verified resource selector.
fn unassign_with(
    request: UnassignRequest<'_>,
    mut change: impl FnMut(&VmName, &str) -> Result<()>,
) -> Result<()> {
    let (backend, port, identity) = request.device();
    change(
        &request.owner().parse()?,
        &argument(backend, port, identity),
    )
}

#[cfg(test)]
mod tests;

/// Validate the declared physical port without exposing the underlying admin transport.
pub(crate) fn validate_port(value: &str) -> Result<()> {
    port(value, false).map(|_| ())
}

/// A read-only comparison may skip an already matching request, but never grants permission.
pub(crate) fn assignment_current(
    selection: &airlock_deployment_core::devices::AssignmentIntent<'_>,
    uses: &[Usage],
) -> bool {
    let (backend, port, identity) = selection.resource();
    uses.iter().any(|u| {
        matches!(u.kind, Kind::Assigned)
            && u.owner == selection.owner()
            && u.backend == backend
            && u.port == port
            && u.identity == identity
            && u.mode == selection.mode()
            && same_options(&u.options, selection.options())
    })
}

/// Compare the complete option maps, refusing extra observed options rather than ignoring them.
fn same_options(
    a: &[airlock_deployment_core::devices::OptionValue],
    b: &[airlock_deployment_core::devices::OptionValue],
) -> bool {
    a.len() == b.len()
        && a.iter()
            .all(|a| b.iter().any(|b| a.name == b.name && a.value == b.value))
}

/// Quote upstream property serialization; keys are validated by the configuration decoder.
fn property(name: &str, value: &str) -> Result<String> {
    ensure!(
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b)),
        "invalid device property key"
    );
    ensure!(
        value.is_ascii() && !value.bytes().any(|b| b.is_ascii_control()),
        "invalid device property value"
    );
    let encoded = format!("{name}='{value}'", value = value.replace('\'', "\\'"));
    let decoded = properties(&encoded)?;
    ensure!(
        decoded.len() == 1 && decoded.get(name).is_some_and(|actual| actual == value),
        "ambiguous Qubes property encoding"
    );
    Ok(encoded)
}

/// Apply precisely the verified resource/options. Replacement removes only that same virtual assignment.
pub(crate) fn assign(
    request: airlock_deployment_core::devices::AssignmentRequest<'_>,
    uses: &[Usage],
) -> Result<()> {
    assign_with(request, uses, |domain, method, argument, payload| {
        call(domain, method, argument, payload).map(|_| ())
    })
}

/// Inert peers exercise the same exact dispatch path; only the upstream transport is replaceable.
fn assign_with(
    request: airlock_deployment_core::devices::AssignmentRequest<'_>,
    uses: &[Usage],
    mut change: impl FnMut(&VmName, &'static str, &str, &[u8]) -> Result<()>,
) -> Result<()> {
    let (backend, port, identity) = request.resource();
    let owner: VmName = request.owner().parse()?;
    let argument = argument(backend, port, identity);
    let mut fields = Vec::new();
    for (key, value) in [
        ("device_id", identity),
        ("port_id", port),
        ("devclass", "pci"),
        ("backend_domain", backend),
        ("mode", request.mode()),
        ("frontend_domain", request.owner()),
    ] {
        fields.push(property(key, value)?);
    }
    for option in request.options() {
        fields.push(property(
            &format!("_{name}", name = option.name),
            &option.value,
        )?);
    }
    let payload = fields.join(" ");
    for existing in uses {
        if let Some(replacement) = request.replacement(existing) {
            let (backend, port, identity) = replacement.resource();
            change(
                &replacement.owner().parse()?,
                "admin.vm.device.pci.Unassign",
                &self::argument(backend, port, identity),
                &[],
            )?;
        }
    }
    change(
        &owner,
        "admin.vm.device.pci.Assign",
        &argument,
        payload.as_bytes(),
    )?;
    Ok(())
}
