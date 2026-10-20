//! Describe host activation and reboot requirements in the runtime's existing deployment format.

use crate::topology::{
    normalize::{is_set, map_attrs, non_null, require, source},
    schema::Schema,
};
use rusix::{
    interop::{NixExpression, raw::NixValue},
    nix_record, nix_text,
};

/// Deployment plan format consumed by the configuration controller.
const SCHEMA: &str = "airlock.system.v1";

/// Export closed option validation and normalization with the caller's NixOS library.
pub(crate) fn library() -> NixValue {
    NixValue::function(|lib| {
        let schema = Schema { lib: &lib };
        let normalize = NixValue::function(|value| {
            lib.clone()
                .select("evalModules")
                .call(nix_record! {
                    "modules": NixValue::list([
                        nix_record! {
                            "options": declarations(&schema),
                        },
                        nix_record! {
                            "config": value,
                        },
                    ]),
                })
                .select("config.dom0")
                .bind(normalize)
        });

        nix_record! {
            "schema": SCHEMA,
            "mkSystem": normalize,
            "activationActions": strings(&["none", "run"]),
            "systemdActions": strings(&["none", "restart", "reload"]),
            "switchActions": strings(&["switch", "boot", "test", "dry-activate"]),
        }
    })
}

/// Validate profile activation, service operations and boot-sensitive components before lowering.
fn declarations(schema: &Schema<'_>) -> NixValue {
    let string = schema.kind("str");
    let profile = schema.record(schema.source_fields().merge_attrs(nix_record! {
        "activationCommand": schema.option(
            relative_path(schema),
            Some("bin/airlock-dom0-profile-activate".into()),
            "Executable path within the admitted profile.",
        ),
        "activationAction": schema.option(
            schema.enumeration(&["none", "run"]),
            Some("run".into()),
            "Whether to execute the profile's activation command.",
        ),
        "message": schema.option(
            string.clone(),
            Some("".into()),
            "Description recorded with the activated profile.",
        ),
    }));
    let service = schema.record(nix_record! {
        "action": schema.option(
            schema.enumeration(&["none", "restart", "reload"]),
            Some("none".into()),
            "Requested live operation on an existing systemd service.",
        ),
        "reason": schema.optional(
            string.clone(),
            "Explanation recorded for the service operation.",
        ),
    });
    let components = NixValue::record(["system", "kernel", "xen", "bootloader", "qubes"].map(
        |name| {
            let mut options = schema.source_fields().merge_attrs(nix_record! {
                "enable": schema.optional(
                    schema.kind("bool"),
                    "Enable this request; defaults to whether an artifact was supplied.",
                ),
                "reason": schema.option(
                    string.clone(),
                    Some(
                        if name == "system" {
                            "new dom0 NixOS system generation".to_owned()
                        } else {
                            format!("dom0 {name} change requires reboot")
                        }
                        .into(),
                    ),
                    "Explanation shown in the deployment plan.",
                ),
            });
            if name == "system" {
                options = options.merge_attrs(nix_record! {
                    "switchAction": schema.option(
                        schema.enumeration(&["switch", "boot", "test", "dry-activate"]),
                        Some("switch".into()),
                        "NixOS activation mode requested for the host generation.",
                    ),
                });
            }

            (
                name,
                schema.option(
                    schema.nullable(schema.record(options)),
                    Some(NixValue::null()),
                    "Optional host component change.",
                ),
            )
        },
    ));
    let dom0 = schema.record(nix_record! {
        "profile": schema.option(
            profile,
            Some(nix_record! {}),
            "Host profile artifact and activation request.",
        ),
        "packages": schema.option(
            schema.list(schema.record(nix_record! {
                "name": schema.option(
                    schema.matching("[A-Za-z0-9_.:+@-]+"),
                    None,
                    "Package identity recorded by the host profile.",
                ),
            })),
            Some(NixValue::list([])),
            "Packages represented by the host profile.",
        ),
        "systemd": schema.option(
            schema.record(nix_record! {
                "services": schema.option(
                    schema.named_attrs(service, "[A-Za-z0-9_.:+@-]+"),
                    Some(nix_record! {}),
                    "Explicit service activation requests.",
                ),
            }),
            Some(nix_record! {}),
            "Live systemd operations.",
        ),
        "nixos": schema.option(
            schema.record(components),
            Some(nix_record! {}),
            "Host system and boot component requests.",
        ),
    });

    nix_record! {
        "schema": schema.option(
            schema.enumeration(&[SCHEMA]),
            Some(SCHEMA.into()),
            "Host deployment plan protocol.",
        ),
        "dom0": schema.option(
            dom0,
            Some(nix_record! {}),
            "Host activation state and requested operations.",
        ),
    }
}

/// Confine activation to a nonempty path within the admitted profile.
fn relative_path(schema: &Schema<'_>) -> NixValue {
    schema
        .kind("addCheck")
        .call(schema.matching("[A-Za-z0-9_.+-]+(/[A-Za-z0-9_.+-]+)*"))
        .call(NixValue::function(|path| {
            NixValue::builtin("all")
                .call(NixValue::function(|part| {
                    part.clone()
                        .equals(".")
                        .equals(false)
                        .and(part.equals("..").equals(false))
                }))
                .call(NixValue::builtin("split").call("/").call(path))
        }))
}

/// Produce normalized host state and ordered live/reboot operations.
fn normalize(dom0: NixValue) -> NixValue {
    let profile = dom0.clone().select("profile");
    let profile = source(&profile).merge_attrs(nix_record! {
        "activation_command": profile.clone().select("activationCommand"),
        "activation_action": profile.clone().select("activationAction"),
        "message": profile.select("message"),
    });
    let services = map_attrs(dom0.clone().select("systemd.services"), |name, service| {
        let reason = non_null(
            service.clone().select("reason"),
            nix_text!("dom0 systemd service {name} changed", name = name).into(),
        );

        service.merge_attrs(nix_record! {
            "reason": reason,
        })
    });
    let components = NixValue::record(["system", "kernel", "xen", "bootloader", "qubes"].map(
        |name| {
            (
                name,
                component(name, dom0.clone().select("nixos").select(name)),
            )
        },
    ));
    let normalized = nix_record! {
        "profile": profile,
        "packages": dom0.select("packages"),
        "systemd": nix_record! {
            "services": services,
        },
        "nixos": components,
    };

    normalized.bind(|dom0| {
        let profile = dom0.clone().select("profile");
        let system = dom0.clone().select("nixos.system");
        let service_actions =
            NixValue::builtin("concatLists").call(NixValue::builtin("attrValues").call(map_attrs(
                dom0.clone().select("systemd.services"),
                |name, service| {
                    optional(
                        service
                            .clone()
                            .select("action")
                            .equals("none")
                            .equals(false),
                        service.merge_attrs(nix_record! {
                            "type": "systemd-service",
                            "name": name,
                        }),
                    )
                },
            )));
        let live = NixValue::concat_lists([
            optional(
                is_set(&profile),
                profile.clone().merge_attrs(nix_record! {
                    "type": "dom0-profile",
                    "action": profile.select("activation_action"),
                }),
            ),
            optional(
                system.clone().select("enable"),
                action("dom0-nixos-system", &system).merge_attrs(nix_record! {
                    "action": system.clone().select("switch_action"),
                }),
            ),
            service_actions,
        ]);
        let reboot = NixValue::concat_lists(
            std::iter::once(optional(
                system
                    .clone()
                    .select("enable")
                    .and(system.clone().select("switch_action").equals("boot")),
                action("dom0-nixos-system", &system).merge_attrs(nix_record! {
                    "switch_action": system.select("switch_action"),
                }),
            ))
            .chain(["bootloader", "kernel", "qubes", "xen"].map(|name| {
                let component = dom0.clone().select("nixos").select(name);

                optional(
                    component.clone().select("enable"),
                    action(&format!("dom0-nixos-{name}"), &component),
                )
            })),
        );

        reboot.bind(|reboot| {
            nix_record! {
                "schema": SCHEMA,
                "dom0": dom0,
                "plan": nix_record! {
                    "live_actions": live,
                    "reboot_actions": reboot.clone(),
                    "reboot_required": reboot.clone().equals(NixValue::list([])).equals(false),
                    "reboot_reasons": NixValue::builtin("map")
                        .call(NixValue::function(|action| action.select("reason")))
                        .call(reboot),
                },
            }
        })
    })
}

/// Resolve artifact defaults and reject an enabled system switch without a system artifact.
fn component(name: &str, value: NixValue) -> NixValue {
    let value = non_null(value, nix_record! {});
    value.bind(|value| {
        source(&value).bind(|source| {
            let enabled = non_null(
                value.clone().attr_or("enable", NixValue::null()),
                is_set(&source),
            );
            let reason = value.clone().attr_or(
                "reason",
                if name == "system" {
                    "new dom0 NixOS system generation".to_owned()
                } else {
                    format!("dom0 {name} change requires reboot")
                },
            );
            let mut normalized = source.clone().merge_attrs(nix_record! {
                "enable": enabled.clone(),
                "reason": reason,
            });
            if name == "system" {
                normalized = require(
                    enabled.equals(false).or(is_set(&source)),
                    "An enabled host system requires a source or sourceRef.",
                    normalized.merge_attrs(nix_record! {
                        "switch_action": value.attr_or("switchAction", "switch"),
                    }),
                );
            }

            normalized
        })
    })
}

/// Copy the artifact and explanation fields shared by host activation actions.
fn action(kind: &str, component: &NixValue) -> NixValue {
    nix_record! {
        "type": kind,
        "source": component.clone().select("source"),
        "source_ref": component.clone().select("source_ref"),
        "reason": component.clone().select("reason"),
    }
}

/// Include an action only when its request is enabled.
fn optional(enabled: NixValue, action: NixValue) -> NixValue {
    NixValue::if_else(enabled, NixValue::list([action]), NixValue::list([]))
}

/// Preserve supported operation spellings at the exported library boundary.
fn strings(values: &[&str]) -> NixValue {
    NixValue::list(values.iter().map(|value| (*value).into()))
}
