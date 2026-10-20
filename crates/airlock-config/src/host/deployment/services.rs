//! Order deployment reconciliation after the guarded desktop and physical input startup.

use rusix::{interop::raw::NixValue, nix_record, nix_text, nixos::ModuleContext};

/// Preserve the split-startup dependency graph and serialized generation maintenance.
pub(super) fn configuration(
    context: &ModuleContext,
    cfg: &NixValue,
    engine: &NixValue,
    setup: NixValue,
) -> NixValue {
    let when = |condition, value| {
        context
            .lib
            .clone()
            .select("mkIf")
            .call(condition)
            .call(value)
    };
    let force = |value| context.lib.clone().select("mkForce").call(value);
    let guarded = context
        .config
        .clone()
        .select("airlock.guiGpu.enable")
        .and(context.config.clone().select("airlock.guiGpu.startOnBoot"));
    let networking = context
        .config
        .clone()
        .select("airlock.qubes.startNetworkProvidersOnBoot.enable");
    let deferred = context
        .config
        .clone()
        .select("airlock.qubes.deferredSetupConfigOnBoot.enable");
    let input = context
        .config
        .clone()
        .select("airlock.inputProxy.sender.enable");
    let input_dependency = optional(
        input.clone(),
        strings(&["airlock-gui-input-connect.service"]),
    );
    let prerequisites = NixValue::concat_lists([
        strings(&["airlock-setup-config.service"]),
        optional(guarded.clone(), strings(&["airlock-start-gui-gpu.service"])),
        optional(
            networking.clone(),
            strings(&["airlock-start-network-providers.service"]),
        ),
        optional(
            deferred.clone(),
            strings(&["airlock-setup-config-deferred.service"]),
        ),
    ]);
    let command = |operation: &str| {
        nix_text!(
            "{engine}/bin/airlock-dom0-service {operation}",
            engine = engine.clone(),
            operation = operation,
        )
    };
    let foreground: NixValue = nix_text!(
        "{engine}/bin/airlock-dom0-service boot-setup{gui} {setup}/bin/airlock-setup-config --apply{scope}",
        engine = engine.clone(),
        gui = NixValue::if_else(guarded.clone(), " --gui", ""),
        setup = setup.clone(),
        scope = NixValue::if_else(
            guarded.clone().equals(false).and(
                context
                    .config
                    .clone()
                    .select("airlock.qubes.setupConfigOnBoot.scope")
                    .equals("boot-critical"),
            ),
            " --boot-critical",
            "",
        ),
    )
    .into();
    let background: NixValue = nix_text!(
        "{engine}/bin/airlock-dom0-service boot-setup{background} {setup}/bin/airlock-setup-config --apply{scope}",
        engine = engine.clone(),
        background = NixValue::if_else(guarded.clone(), " --background", ""),
        setup = setup,
        scope = NixValue::if_else(guarded.clone(), "", " --deferred"),
    )
    .into();

    nix_record! {
        "systemd": nix_record! {
            "paths": nix_record! {
                "airlock-generation-maintenance": when(
                    cfg.clone().select("generationRetention.enable"),
                    nix_record! {
                        "wantedBy": strings(&["multi-user.target"]),
                        "pathConfig": nix_record! {
                            "PathExists": "/var/lib/airlock/machine/maintenance/pending.json",
                            "Unit": "airlock-generation-maintenance.service",
                        },
                    },
                ),
            },
            "services": context.lib.clone().select("mkMerge").call(NixValue::list([
                when(
                    cfg.clone().select("generationRetention.enable"),
                    nix_record! {
                        "airlock-generation-maintenance": nix_record! {
                            "description": "Prune obsolete verified machine generations \
                                and collect unused pool artifacts",
                            "after": strings(&["airlock-machine-autostart.service"]),
                            "restartIfChanged": false,
                            "stopIfChanged": false,
                            "unitConfig": nix_record! {
                                "StartLimitIntervalSec": 0_i64,
                            },
                            "serviceConfig": nix_record! {
                                "Type": "oneshot",
                                "ExecStart": command("generation-maintenance"),
                                "TimeoutStartSec": "infinity",
                                "Restart": "on-failure",
                                "RestartSec": "60s",
                                "Nice": 10_i64,
                                "IOSchedulingClass": "best-effort",
                                "IOSchedulingPriority": 7_i64,
                            },
                        },
                    },
                ),
                when(
                    context.config.clone().select("airlock.qubes.setupConfigOnBoot.enable"),
                    nix_record! {
                        "airlock-machine-autostart": nix_record! {
                            "description": "Start declared autostart VMs after guarded bootstrap",
                            "wantedBy": strings(&["multi-user.target"]),
                            "requires": prerequisites.clone(),
                            "after": prerequisites,
                            "restartIfChanged": false,
                            "stopIfChanged": false,
                            "serviceConfig": nix_record! {
                                "Type": "oneshot",
                                "RemainAfterExit": true,
                                "ExecStart": nix_text!(
                                    "{engine}/bin/airlock-dom0-service boot-start{require}",
                                    engine = engine.clone(),
                                    require = NixValue::if_else(guarded.clone(), " --require-running sys-gui", ""),
                                ),
                                "ExecStartPost": NixValue::list([
                                    command("boot-supervisor complete 'all declared autostart VMs are qrexec-ready'")
                                        .into(),
                                    nix_text!(
                                        "{systemd}/bin/journalctl --sync",
                                        systemd = context.pkgs.clone().select("systemd"),
                                    )
                                    .into(),
                                ]),
                            },
                        },
                        "airlock-setup-config": nix_record! {
                            "serviceConfig": nix_record! {
                                "ExecStart": force(foreground),
                            },
                        },
                    },
                ),
                when(
                    deferred,
                    nix_record! {
                        "airlock-setup-config-deferred": nix_record! {
                            "serviceConfig": nix_record! {
                                "ExecStart": force(background),
                            },
                        },
                    },
                ),
                when(
                    guarded,
                    nix_record! {
                        "airlock-gui-input-connect": when(
                            input,
                            nix_record! {
                                "after": strings(&["airlock-start-gui-gpu.service"]),
                            },
                        ),
                        "airlock-setup-config-deferred": nix_record! {
                            "requires": NixValue::concat_lists([
                                strings(&["airlock-start-gui-gpu.service"]),
                                input_dependency.clone(),
                            ]),
                            "wants": force(strings(&[
                                "libvirtd.service",
                                "qubes-db-dom0.service",
                                "qubesd.service",
                                "qubes-qrexec-policy-daemon.service",
                            ])),
                            "after": force(NixValue::concat_lists([
                                strings(&[
                                    "airlock-setup-config.service",
                                    "airlock-start-gui-gpu.service",
                                    "qubesd.service",
                                    "qubes-db-dom0.service",
                                    "qubes-qrexec-policy-daemon.service",
                                ]),
                                input_dependency,
                            ])),
                            "before": strings(&[
                                "airlock-start-network-providers.service",
                                "airlock-machine-autostart.service",
                            ]),
                        },
                        "airlock-start-network-providers": when(
                            networking,
                            nix_record! {
                                "requires": strings(&["airlock-setup-config-deferred.service"]),
                                "after": strings(&["airlock-setup-config-deferred.service"]),
                                "before": force(strings(&["airlock-machine-autostart.service"])),
                                "environment": nix_record! {
                                    "AIRLOCK_QUBES_TOPOLOGY": "/run/airlock/startup/topology.json",
                                },
                            },
                        ),
                    },
                ),
            ])),
        },
    }
}

/// Include optional dependency edges without changing the order of the remaining edges.
fn optional(condition: NixValue, values: NixValue) -> NixValue {
    NixValue::if_else(condition, values, NixValue::list([]))
}

/// Preserve static service dependency order.
fn strings(values: &[&str]) -> NixValue {
    NixValue::list(values.iter().map(|value| (*value).into()))
}
