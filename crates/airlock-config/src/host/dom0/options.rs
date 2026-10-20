//! Declare dom0 resource and development controls shared by the machine profiles.

use rusix::{interop::raw::NixValue, nix_record, nixos::ModuleContext};

/// Keep application frontends out of dom0 while retaining upstream control-stack packages.
pub(super) fn declarations(context: &ModuleContext) -> NixValue {
    let cfg = context.config.clone().select("airlock.dom0");
    let lib = &context.lib;
    let string = lib.clone().select("types.str");
    let boolean = lib.clone().select("types.bool");
    let unsigned = lib.clone().select("types.ints.unsigned");
    let enable = |description| lib.clone().select("mkEnableOption").call(description);
    let option = |kind, default, description| {
        lib.clone().select("mkOption").call(nix_record! {
            "type": kind,
            "default": default,
            "description": description,
        })
    };
    let development = |description| {
        option(
            boolean.clone(),
            context
                .config
                .clone()
                .select("airlock.image.devMode.enable"),
            description,
        )
    };
    let identity = lib.clone().select("types.submodule").call(nix_record! {
        "options": nix_record! {
            "revision": lib.clone().select("mkOption").call(nix_record! {
                "type": string.clone(),
            }),
            "timestamp": lib.clone().select("mkOption").call(nix_record! {
                "type": unsigned.clone(),
            }),
        },
    });
    let filter = NixValue::function(|packages| {
        lib.clone()
            .select("filter")
            .call(NixValue::function(|package| {
                let name = package
                    .clone()
                    .attr_or("pname", lib.clone().select("getName").call(package));
                let frontend = NixValue::builtin("elem").call(name).call(NixValue::list(
                    [
                        "qubes-manager",
                        "qubes-desktop-linux-manager",
                        "qubes-artwork",
                    ]
                    .map(Into::into),
                ));

                cfg.clone()
                    .select("enable")
                    .and(cfg.clone().select("gui.enable"))
                    .and(frontend)
                    .equals(false)
            }))
            .call(packages)
    });

    nix_record! {
        "options": nix_record! {
            "environment": nix_record! {
                "systemPackages": lib.clone().select("mkOption").call(nix_record! {
                    "apply": filter,
                }),
            },
            "airlock": nix_record! {
                "image": nix_record! {
                    "devMode": nix_record! {
                        "enable": option(
                            boolean.clone(),
                            false.into(),
                            "Select development capabilities for the whole image at build time.",
                        ),
                    },
                },
                "dom0": nix_record! {
                    "enable": enable("the Airlock Qubes-on-NixOS control domain"),
                    "user": option(
                        string.clone(),
                        "airlock".into(),
                        "Local user receiving Qubes control-group access.",
                    ),
                    "gui": nix_record! {
                        "enable": enable("Qubes display relay integration in dom0"),
                    },
                    "bootMenu": nix_record! {
                        "enable": enable("the concise Airlock USB boot menu"),
                        "source": option(
                            lib.clone().select("types.nullOr").call(identity),
                            NixValue::null(),
                            "Committed source identity for this host's boot entries.",
                        ),
                    },
                    "observability": nix_record! {
                        "enable": development("Install read-only development observation commands."),
                    },
                    "testingTools": nix_record! {
                        "enable": development(
                            "Install development bootstrap and smoke-test commands.",
                        ),
                    },
                    "dom0Resources": nix_record! {
                        "memoryMiB": option(
                            unsigned.clone(),
                            4096_i64.into(),
                            "Initial Xen dom0 memory in MiB.",
                        ),
                        "maxMemoryMiB": option(
                            unsigned,
                            cfg.clone().select("dom0Resources.memoryMiB"),
                            "Maximum Xen dom0 memory in MiB.",
                        ),
                    },
                    "devModeNetworkProxy": nix_record! {
                        "enable": development(
                            "Expose the development loopback HTTP proxy through Qubes qrexec.",
                        ),
                        "target": option(
                            string.clone(),
                            "sys-firewall".into(),
                            "Network VM serving qubes.UpdatesProxy.",
                        ),
                        "listenAddress": option(
                            string.clone(),
                            "127.0.0.1".into(),
                            "Loopback address for the development proxy.",
                        ),
                        "port": option(
                            lib.clone().select("types.port"),
                            8082_i64.into(),
                            "Loopback TCP port for the development proxy.",
                        ),
                    },
                    "extraOptOutRecommendedConfiguration": option(
                        lib.clone().select("types.listOf").call(string),
                        NixValue::list([]),
                        "Additional documented exceptions to upstream Qubes recommendations.",
                    ),
                    "libvirtFirewallBackend": option(
                        lib.clone()
                            .select("types.enum")
                            .call(NixValue::list(["iptables".into(), "nftables".into()])),
                        "iptables".into(),
                        "Compatibility backend accepted by the selected NixOS libvirtd module.",
                    ),
                },
            },
        },
    }
}
