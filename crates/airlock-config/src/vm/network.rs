//! Compose upstream Qubes network services and preserve their Xen frontend lifecycle.
//! Physical interfaces belong to NetworkManager; QubesDB supplies addressing on VM uplinks.

use airlock_build::{Application, Workspace, script::substitute};
use rusix::{
    interop::{
        NixExpression, NixPath, PackageFunction,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody, ModuleContext},
};

/// Shared runtime packages selected by the surrounding Qubes VM composition.
pub(super) struct Packages {
    /// Commands used by Qubes initialization, including the selected systemd package.
    pub qubes_path: NixValue,
    /// Python with the QubesDB, D-Bus and desktop integration modules.
    pub python: NixValue,
    /// Firewall, forwarding and proxy tools required by network-provider VMs.
    pub provider_packages: NixValue,
}

/// Accept package dependencies at the build boundary while requesting NixOS context normally.
pub(super) fn constructor() -> PackageFunction {
    PackageFunction::from_function_attrs(["qubesPath", "python", "providerPackages"], |args| {
        let packages = Packages {
            qubes_path: args.clone().select("qubesPath"),
            python: args.clone().select("python"),
            provider_packages: args.select("providerPackages"),
        };

        (Vec::<(&str, NixValue)>::new(), module(&packages))
    })
}

/// Install role-appropriate networking without changing Qubes policy or control authority.
pub(super) fn module(packages: &Packages) -> NixValue {
    ContextualModule::new(|context| {
        let settings = context.config.clone().select("airlock.qubeGuest");
        let role = settings.clone().select("role");
        let driver = role.clone().equals("sys-net");
        let provider = driver.clone().or(role.equals("sys-firewall"));
        let config = configuration(&context, packages, driver, provider);

        ModuleBody::new(
            context
                .lib
                .select("mkIf")
                .call(settings.select("enable"))
                .call(config),
        )
        .key("airlock.vm.network")
    })
    .into_expression()
}

/// Build the local guest adapter from the same prepared source used by the VM composition.
fn native(context: &ModuleContext) -> NixValue {
    let workspace = Workspace::new(NixPath::from_source_text(
        context
            .pkgs
            .clone()
            .select("airlockAppsSource")
            .to_text()
            .into_expr::<String>(),
    ));
    Application::VmService
        .build(&context.pkgs, &workspace)
        .as_expression()
}

/// Construct scripts with explicit store dependencies and keep shared tools bound once.
fn configuration(
    context: &ModuleContext,
    packages: &Packages,
    driver: NixValue,
    provider: NixValue,
) -> NixValue {
    let package = |name| context.pkgs.clone().select(name);
    let script = |name, value| {
        airlock_build::native::writer(&context.pkgs, false)
            .call(name)
            .call(value)
    };
    let agent = package("qubes-core-agent-linux");
    let systemd = context.config.clone().select("systemd.package");
    let path = NixValue::concat_lists([
        packages.qubes_path.clone(),
        packages.provider_packages.clone(),
    ]);
    let dnat = script(
        "airlock-qubes-setup-dnat-to-ns",
        crate::commands::contract(
            &context.pkgs,
            "dnat",
            [
                ("python", packages.python.clone()),
                ("agent", agent.clone()),
            ],
        ),
    );

    dnat.bind(|dnat| {
        let proxy = package("runCommand")
            .call("airlock-qubes-network-proxy-setup")
            .call(nix_record! {})
            .call(substitute(
                include_str!("network/proxy-setup.sh"),
                [("agent", agent.clone()), ("dnat", dnat.clone())],
            ));
        let setup_ip = package("runCommand")
            .call("airlock-qubes-setup-ip")
            .call(nix_record! {})
            .call(substitute(
                include_str!("network/setup-ip.sh"),
                [
                    ("agent", agent.clone()),
                    ("agentSource", package("qubes-core-agent-linux.src")),
                    ("dnat", dnat.clone()),
                    ("qubesdb", package("qubes-core-qubesdb")),
                    ("systemd", systemd.clone()),
                    ("networkmanager", package("networkmanager")),
                ],
            ));
        let firewall = script(
            "airlock-qubes-iptables-start",
            crate::commands::contract(
                &context.pkgs,
                "firewall",
                [("agent", agent.clone()), ("service", native(context))],
            ),
        );
        let tinyproxy = package("writeText")
            .call("tinyproxy-updates.conf")
            .call(substitute(
                include_str!("network/tinyproxy.conf"),
                [
                    ("agent", agent.clone()),
                    ("tinyproxy", package("tinyproxy")),
                ],
            ));
        let tools = Tools {
            proxy,
            setup_ip,
            firewall,
            tinyproxy,
        };

        let settings = settings(context, driver.clone(), provider.clone());
        let services = services(
            context,
            packages,
            &tools,
            &path,
            driver.clone(),
            provider.clone(),
        );
        let files = files(context, &path, dnat, driver, provider);

        context
            .lib
            .clone()
            .select("mkMerge")
            .call(NixValue::list([settings, services, files]))
    })
}

/// Store-backed commands and configuration consumed by network services.
struct Tools {
    /// Upstream proxy initialization patched to use the Qubes Python environment.
    proxy: NixValue,
    /// Upstream interface add/remove command patched for the Nix store.
    setup_ip: NixValue,
    /// Atomic installation of the upstream IPv4 and IPv6 firewall rules.
    firewall: NixValue,
    /// Updates proxy configuration with upstream filtering and bounded connections.
    tinyproxy: NixValue,
}

/// Assign resolver and physical-network ownership and create the provider's proxy account.
fn settings(context: &ModuleContext, driver: NixValue, provider: NixValue) -> NixValue {
    let when = |condition: NixValue, value: NixValue| {
        context
            .lib
            .clone()
            .select("mkIf")
            .call(condition)
            .call(value)
    };

    nix_record! {
        "boot": nix_record! {
            // WPA Supplicant's upstream sandbox binds /dev/rfkill. Load the
            // radio-control module before services start, including on wired
            // hosts or when wireless hardware arrives after initial coldplug.
            "kernelModules": when(driver.clone(), NixValue::list(["rfkill".into()])),
        },
        "networking": nix_record! {
            "useDHCP": context.lib.clone().select("mkDefault").call(false),
            // setup-ip writes the resolver file for Qubes uplinks; resolvconf would race it.
            "resolvconf": nix_record! {
                "enable": when(driver.clone().equals(false), false.into()),
            },
            "firewall": nix_record! {
                "enable": when(provider.clone(), false.into()),
            },
            "networkmanager": nix_record! {
                "enable": when(driver.clone(), true.into()),
                "settings": when(
                    driver.clone(),
                    nix_record! {
                        "main": nix_record! {
                            "dns": "default",
                        },
                        "keyfile": nix_record! {
                            "unmanaged-devices": "mac:fe:ff:ff:ff:ff:ff",
                        },
                        "device": nix_record! {
                            "wifi.scan-rand-mac-address": "yes",
                        },
                        "connection": nix_record! {
                            "wifi.cloned-mac-address": "stable",
                            "connection.stable-id": "${CONNECTION}/${BOOT}",
                            "ipv6.ip6-privacy": "2",
                        },
                    },
                ),
            },
        },
        "hardware": nix_record! {
            "enableRedistributableFirmware": when(driver, true.into()),
        },
        "users": nix_record! {
            "groups": nix_record! {
                "tinyproxy": when(provider.clone(), nix_record! {}),
            },
            "users": nix_record! {
                "tinyproxy": when(
                    provider,
                    nix_record! {
                        "isSystemUser": true,
                        "group": "tinyproxy",
                    },
                ),
            },
        },
    }
}

/// Preserve service ordering from private storage through firewall and forwarding readiness.
fn services(
    context: &ModuleContext,
    packages: &Packages,
    tools: &Tools,
    path: &NixValue,
    driver: NixValue,
    provider: NixValue,
) -> NixValue {
    let package = |name| context.pkgs.clone().select(name);
    let when = |condition: NixValue, value: NixValue| {
        context
            .lib
            .clone()
            .select("mkIf")
            .call(condition)
            .call(value)
    };
    let names = |values: &[&str]| NixValue::list(values.iter().map(|value| (*value).into()));
    let agent = package("qubes-core-agent-linux");
    let systemd = context.config.clone().select("systemd.package");
    let once = |fields: NixValue| {
        nix_record! {
            "Type": "oneshot",
            "RemainAfterExit": true,
        }
        .merge_attrs(fields)
    };

    nix_record! {
        "systemd": nix_record! {
            "services": nix_record! {
                "airlock-network-driver-networkmanager-state": when(
                    driver.clone(),
                    nix_record! {
                        "description": "Prepare persistent NetworkManager state for the network driver domain",
                        "wants": names(&["airlock-qubes-rw-init.service"]),
                        "requires": names(&["airlock-qubes-rw-init.service"]),
                        "after": names(&["airlock-qubes-rw-init.service"]),
                        "before": names(&["NetworkManager.service"]),
                        "path": NixValue::list([package("coreutils")]),
                        "script": airlock_build::native::hook(&context.pkgs, "airlock-networkmanager-state", crate::commands::contract(&context.pkgs, "networkmanager-state", [("service", native(context))])),
                        "serviceConfig": once(nix_record! {}),
                    },
                ),
                "NetworkManager": when(
                    driver,
                    nix_record! {
                        "wants": names(&[
                            "airlock-network-driver-networkmanager-state.service",
                            "qubes-sysinit.service",
                        ]),
                        "requires": names(&["airlock-network-driver-networkmanager-state.service"]),
                        "after": names(&[
                            "airlock-network-driver-networkmanager-state.service",
                            "qubes-sysinit.service",
                        ]),
                        "unitConfig": nix_record! {
                            "ConditionPathExists": "/run/qubes-service/network-manager",
                        },
                    },
                ),
                "qubes-antispoof": when(
                    provider.clone(),
                    nix_record! {
                        "description": "Qubes anti-spoofing firewall rules",
                        "before": names(&["network-pre.target"]),
                        "requiredBy": names(&["network-pre.target"]),
                        "path": NixValue::list([package("nftables")]),
                        "serviceConfig": once(nix_record! {
                            "ExecStart": nix_text!(
                                "{nft}/bin/nft -f {agent}/etc/qubes/qubes-antispoof.nft",
                                nft = package("nftables"),
                                agent = agent.clone(),
                            ),
                        }),
                    },
                ),
                "qubes-iptables": when(
                    provider.clone(),
                    nix_record! {
                        "description": "Qubes base firewall settings",
                        "requires": names(&["qubes-antispoof.service"]),
                        "after": names(&["qubes-antispoof.service"]),
                        "before": names(&["network-pre.target"]),
                        "requiredBy": names(&["network-pre.target"]),
                        "path": path.clone(),
                        "serviceConfig": once(nix_record! {
                            "ExecStart": tools.firewall.clone().to_text(),
                        }),
                    },
                ),
                "qubes-network": when(
                    provider.clone(),
                    nix_record! {
                        "description": "Qubes network forwarding setup",
                        "wantedBy": names(&["multi-user.target"]),
                        "wants": names(&["qubes-iptables.service", "qubes-sysinit.service"]),
                        "requires": names(&["qubes-iptables.service"]),
                        "after": names(&[
                            "network-pre.target",
                            "qubes-iptables.service",
                            "qubes-sysinit.service",
                        ]),
                        "before": names(&["network.target"]),
                        "path": path.clone(),
                        "unitConfig": nix_record! {
                            "ConditionPathExists": "/run/qubes-service/qubes-network",
                        },
                        "serviceConfig": once(nix_record! {
                            "ExecStart": nix_text!(
                                "{proxy}/bin/network-proxy-setup",
                                proxy = tools.proxy.clone(),
                            ),
                            "ExecStop": nix_text!(
                                "{agent}/lib/qubes/init/network-proxy-stop.sh",
                                agent = agent.clone(),
                            ),
                        }),
                    },
                ),
                "qubes-firewall": when(
                    provider.clone(),
                    nix_record! {
                        "description": "Qubes firewall updater",
                        "wantedBy": names(&["multi-user.target"]),
                        "wants": names(&["qubes-iptables.service"]),
                        "after": names(&["qubes-iptables.service"]),
                        "before": names(&["qubes-network.service"]),
                        "path": path.clone(),
                        "unitConfig": nix_record! {
                            "ConditionPathExists": "/run/qubes-service/qubes-firewall",
                        },
                        "serviceConfig": nix_record! {
                            "Type": "notify-reload",
                            "ExecStart": nix_text!("{agent}/bin/qubes-firewall", agent = agent),
                            "Restart": "on-failure",
                            "RestartSec": "5s",
                        },
                    },
                ),
                "qubes-updates-proxy": when(
                    provider.clone(),
                    nix_record! {
                        "description": "Qubes updates proxy",
                        "wantedBy": names(&["multi-user.target"]),
                        "after": names(&["qubes-iptables.service"]),
                        "unitConfig": nix_record! {
                            "ConditionPathExists": "|/run/qubes-service/qubes-updates-proxy",
                        },
                        "serviceConfig": nix_record! {
                            "ExecStart": nix_text!(
                                "{proxy}/bin/tinyproxy -d -c {config}",
                                proxy = package("tinyproxy"),
                                config = tools.tinyproxy.clone(),
                            ),
                            "Restart": "on-failure",
                            "RestartSec": "5s",
                            "RuntimeDirectory": "tinyproxy-updates",
                        },
                    },
                ),
                "qubes-network-uplink@": nix_record! {
                    "description": "Qubes network uplink setup",
                    "after": names(&[
                        "network-pre.target",
                        "qubes-db.service",
                        "qubes-sysinit.service",
                    ]),
                    "requires": names(&[
                        "network-pre.target",
                        "qubes-db.service",
                        "qubes-sysinit.service",
                    ]),
                    "before": names(&["network.target"]),
                    "wants": names(&["network.target"]),
                    "path": NixValue::concat_lists([
                        packages.qubes_path.clone(),
                        NixValue::list([
                            package("iproute2"),
                            package("networkmanager"),
                            package("nftables"),
                        ]),
                    ]),
                    "serviceConfig": once(nix_record! {
                        "ExecStart": nix_text!(
                            "{setup}/bin/setup-ip add %i",
                            setup = tools.setup_ip.clone(),
                        ),
                        "ExecStop": nix_text!(
                            "{setup}/bin/setup-ip remove %i",
                            setup = tools.setup_ip.clone(),
                        ),
                    }),
                },
                // Reconcile frontends whose add event occurred before the root filesystem's udev rules.
                "qubes-network-uplink": nix_record! {
                    "description": "Configure Qubes uplinks already discovered in the initrd",
                    "wantedBy": names(&["multi-user.target"]),
                    "requires": names(&["qubes-db.service", "qubes-sysinit.service"]),
                    "wants": names(&["systemd-udev-settle.service"]),
                    "after": names(&[
                        "qubes-db.service",
                        "qubes-sysinit.service",
                        "systemd-udev-settle.service",
                    ]),
                    "path": NixValue::list([package("coreutils"), systemd]),
                    "script": airlock_build::native::hook(&context.pkgs, "airlock-existing-uplinks", crate::commands::contract(&context.pkgs, "existing-uplinks", [("service", native(context))])),
                    "serviceConfig": once(nix_record! {}),
                },
                "xendriverdomain": when(
                    provider,
                    nix_record! {
                        "enable": true,
                        "wantedBy": names(&["multi-user.target"]),
                        "wants": names(&["proc-xen.mount"]),
                        "requires": names(&["proc-xen.mount"]),
                        "after": names(&["proc-xen.mount", "systemd-modules-load.service"]),
                        "path": path.clone(),
                        "overrideStrategy": "asDropin",
                        "serviceConfig": nix_record! {
                            "LogsDirectory": "xen",
                            "RuntimeDirectory": "xen",
                            "RuntimeDirectoryMode": "0755",
                        },
                    },
                ),
            },
        },
    }
}

/// Install upstream routing rules and bounded proxy RPCs with role-specific dispatcher hooks.
fn files(
    context: &ModuleContext,
    path: &NixValue,
    dnat: NixValue,
    driver: NixValue,
    provider: NixValue,
) -> NixValue {
    let package = |name| context.pkgs.clone().select(name);
    let when = |condition: NixValue, value: NixValue| {
        context
            .lib
            .clone()
            .select("mkIf")
            .call(condition)
            .call(value)
    };
    let script = |name, value| {
        airlock_build::native::writer(&context.pkgs, false)
            .call(name)
            .call(value)
    };
    let agent = package("qubes-core-agent-linux");
    let hook = script(
        "airlock-qubes-nmhook",
        crate::commands::contract(
            &context.pkgs,
            "networkmanager-hook",
            [
                ("service", native(context)),
                (
                    "path",
                    context
                        .lib
                        .clone()
                        .select("escapeShellArg")
                        .call(context.lib.clone().select("makeBinPath").call(path.clone())),
                ),
                ("dnat", dnat),
            ],
        ),
    );
    let external = script(
        "airlock-qubes-external-ip",
        crate::commands::contract(
            &context.pkgs,
            "external-ip",
            [
                (
                    "path",
                    context
                        .lib
                        .clone()
                        .select("escapeShellArg")
                        .call(context.lib.clone().select("makeBinPath").call(path.clone())),
                ),
                ("service", native(context)),
                ("iproute", package("iproute2")),
                ("grep", package("gnugrep")),
                ("qubesdb", package("qubes-core-qubesdb")),
            ],
        ),
    );
    let xen = package("symlinkJoin").call(nix_record! {
        "name": "airlock-xen-driver-domain-etc",
        "stripPrefix": "/etc/xen",
        "paths": NixValue::list([
            package("qubes-vmm-xen"),
            agent.clone(),
            package("qubes-linux-utils"),
        ]),
    });
    let proxy = script(
        "airlock-qubes-updates-proxy-rpc-service",
        crate::commands::contract(&context.pkgs, "updates-rpc", [("socat", package("socat"))]),
    );
    let rules = NixValue::record(
        [
            "qubes-antispoof",
            "qubes-ipv4",
            "qubes-ipv6",
            "qubes-ipv6-disabled",
        ]
        .map(|name| {
            (
                format!("qubes/{name}.nft"),
                when(
                    provider.clone(),
                    nix_record! {
                        "source": nix_text!(
                            "{agent}/etc/qubes/{name}.nft",
                            agent = agent.clone(),
                            name = name,
                        ),
                        "mode": "0444",
                    },
                ),
            )
        }),
    );

    nix_record! {
        // Restart explicitly on hotplug: SYSTEMD_WANTS can miss an interface renamed by udev.
        // Never wait in a udev worker or add a device dependency that races activation.
        "services": nix_record! {
            "udev": nix_record! {
                "extraRules": uplink_rules(context),
            },
        },
        "environment": nix_record! {
            "etc": rules.merge_attrs(nix_record! {
                "NetworkManager/dispatcher.d/qubes-nmhook": when(
                    driver.clone(),
                    nix_record! {
                        "source": hook,
                        "mode": "symlink",
                    },
                ),
                "NetworkManager/dispatcher.d/30-qubes-external-ip": when(
                    driver,
                    nix_record! {
                        "source": external,
                        "mode": "symlink",
                    },
                ),
                "systemd/network/80-qubes-vif.link": nix_record! {
                    "source": nix_text!(
                        "{agent}/lib/systemd/network/80-qubes-vif.link",
                        agent = agent,
                    ),
                    "mode": "0444",
                },
                "xen": when(
                    provider.clone(),
                    nix_record! {
                        "source": xen,
                    },
                ),
                "qubes-rpc/qubes.UpdatesProxy": when(
                    provider.clone(),
                    nix_record! {
                        "source": proxy,
                        "mode": "symlink",
                    },
                ),
                "qubes/rpc-config/qubes.UpdatesProxy": when(
                    provider,
                    nix_record! {
                        "text": nix_text!(r#"
                    skip-service-descriptor=true
                    exit-on-service-eof=true
                "#),
                        "mode": "0444",
                    },
                ),
            }),
        },
    }
}

/// Render complete udev rules while retaining the systemd package's store dependency.
fn uplink_rules(context: &ModuleContext) -> NixValue {
    let rules = [("add", "restart"), ("remove", "stop")].map(|(action, operation)| {
        let prefix = format!(
            "SUBSYSTEM==\"net\", ACTION==\"{action}\", DRIVERS==\"vif\", KERNEL!=\"vif*\", RUN+=\"",
        );

        NixValue::concat_text([
            prefix.into(),
            nix_text!(
                "{systemd}/bin/systemctl --no-block {operation} --job-mode=replace ",
                systemd = context.config.clone().select("systemd.package"),
                operation = operation,
            )
            .into(),
            "qubes-network-uplink@%k.service\"\n".into(),
        ])
    });

    NixValue::concat_text(rules)
}
