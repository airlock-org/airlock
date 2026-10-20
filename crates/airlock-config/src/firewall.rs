//! Add packet inspection tools to the firewall VM for network troubleshooting.
//! Its new closure lets the real planner trace the impact through network consumers.

use crate::model::{AuthoringError, Firewall, ProvidesNetwork, Vm, VmHandle};
use rusix::{interop::Nixpkgs, nixos::NixosModule};

/// Adds tcpdump from the firewall's own NixOS package set.
/// The resulting closure change drives the dependency-planning regression scenario.
pub(crate) fn diagnostics() -> NixosModule {
    NixosModule::empty().system_packages(vec![Nixpkgs::from_module().get("tcpdump")])
}

/// Declares a reusable firewall branch with an optional packet-inspection contribution.
/// The caller registers the returned handle, then connects the branch's application VMs.
pub(crate) fn branch<R: ProvidesNetwork>(
    name: &str,
    uplink: &VmHandle<R>,
    with_diagnostics: bool,
) -> Result<VmHandle<Firewall>, AuthoringError> {
    let firewall = Vm::firewall(name, uplink)?;
    if with_diagnostics {
        firewall.add_module(diagnostics());
    }

    Ok(firewall)
}
