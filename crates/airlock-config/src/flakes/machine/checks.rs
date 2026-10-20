//! Retain the upstream Qubes module contract and explicitly selected package build checks.

use super::Build;
use rusix::interop::raw::NixValue;

/// Expose opt-in flake checks without running evaluation or provisioning from quick tests.
pub(super) fn outputs(build: &Build, system: NixValue) -> NixValue {
    let packages = NixValue::builtin("getAttr")
        .call(system.clone())
        .call(build.output("packages"));
    let mut checks = [
        ("config-apply-package", "airlock-config-apply"),
        ("config-archive-package", "airlock-config-archive-candidate"),
        (
            "config-bootstrap-current-package",
            "airlock-config-bootstrap-current",
        ),
        ("config-boot-verify-package", "airlock-config-boot-verify"),
        ("config-control-package", "airlock-config-control"),
        ("codex-package", "codex"),
        ("zellij-package", "zellij"),
        ("qubes-memory-accounting", "qubes-linux-utils"),
        (
            "config-inputs-import-package",
            "airlock-config-inputs-import",
        ),
        ("config-status-package", "airlock-config-status"),
        ("airlock-package", "airlock"),
        ("config-vms-smoke-package", "airlock-config-vms-smoke"),
        (
            "nix-closure-domain-smoke-package",
            "airlock-nix-closure-domain-smoke",
        ),
        ("qube-bootstrap-package", "airlock-qube-bootstrap"),
        ("qube-gui-smoke-package", "airlock-qube-gui-smoke"),
        ("setup-config-package", "airlock-setup-config"),
        (
            "start-network-providers-package",
            "airlock-start-network-providers",
        ),
        ("observe-package", "airlock-observe-dom0"),
    ]
    .map(|(name, package)| (name, packages.clone().select(package)))
    .to_vec();
    checks.push(("module-contract", contract(build, system)));

    NixValue::record(checks)
}

/// Require the composed host to keep Xen, QubesDB, qubesd and qrexec as its control stack.
fn contract(build: &Build, system: NixValue) -> NixValue {
    let config = build.output("nixosConfigurations.test.config");
    let boolean = |value| NixValue::if_else(value, "true", "false");
    let mut environment = vec![
        ("preferLocalBuild", true.into()),
        ("allowSubstitutes", false.into()),
    ];
    for (name, path) in [
        ("airlockDom0Enabled", "airlock.dom0.enable"),
        ("qubesDom0Enabled", "virtualisation.qubes.dom0.enable"),
        ("qubesSecure", "virtualisation.qubes.secure"),
        ("xenEnabled", "virtualisation.xen.enable"),
        ("initrdSystemdEnabled", "boot.initrd.systemd.enable"),
        ("qubesdEnabled", "systemd.services.qubesd.enable"),
        (
            "qubesDbDom0Enabled",
            "systemd.services.qubes-db-dom0.enable",
        ),
        (
            "qrexecPolicyEnabled",
            "systemd.services.qubes-qrexec-policy-daemon.enable",
        ),
    ] {
        environment.push((name, boolean(config.clone().select(path))));
    }
    for (name, choice) in [
        ("dedicatedSysNetOptOut", "dedicated-sys-net"),
        ("dedicatedSysUsbOptOut", "dedicated-sys-usb"),
        ("dom0GuiOptOut", "dom0-is-gui"),
    ] {
        environment.push((
            name,
            boolean(
                NixValue::builtin("elem").call(choice).call(
                    config
                        .clone()
                        .select("virtualisation.qubes.optOutRecommendedConfiguration"),
                ),
            ),
        ));
    }
    for (name, path) in [
        ("xenPackage", "virtualisation.xen.package"),
        ("qemuPackage", "virtualisation.xen.qemu.package"),
        ("libvirtPackage", "virtualisation.libvirtd.package"),
    ] {
        let package = config.clone().select(path);

        environment.push((
            name,
            package
                .clone()
                .attr_or("pname", build.lib.clone().select("getName").call(package)),
        ));
    }
    environment.push((
        "libvirtFirewallBackend",
        config.select("virtualisation.libvirtd.firewallBackend"),
    ));

    build
        .pkgs(system)
        .select("runCommand")
        .call("airlock-module-contract")
        .call(NixValue::record(environment))
        .call(include_str!("module-contract.sh"))
}
