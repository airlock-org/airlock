//! Validate topology declarations and lower them into the runtime's Qubes configuration format.

pub(crate) mod schema;

mod options;

mod access;

mod services;

pub(crate) mod normalize;

use rusix::{
    interop::{NixExpression, raw::NixValue},
    nix_record,
};

/// Wire format shared with runtime topology admission.
const SCHEMA: &str = airlock_machine_core::model::TOPOLOGY_SCHEMA;

/// Export normalization with an explicit NixOS library so all callers use the same option semantics.
pub(crate) fn library() -> NixValue {
    NixValue::function(|lib| {
        let schema = schema::Schema { lib: &lib };
        let normalize = NixValue::function(|value| {
            let merged = lib
                .clone()
                .select("evalModules")
                .call(nix_record! {
                    "modules": NixValue::list([
                        nix_record! {
                            "options": schema.fields(),
                        },
                        nix_record! {
                            "config": value,
                        },
                    ]),
                })
                .select("config");

            merged.bind(normalize::topology)
        });

        nix_record! {
            "schema": SCHEMA,
            "mkTopology": normalize,
            "domainClasses": strings(schema::CLASSES),
            "domainBackendTypes": strings(schema::BACKENDS),
            "labels": strings(schema::LABELS),
            "deviceAssignmentModes": strings(schema::ASSIGNMENTS),
            "deviceMatchModes": strings(schema::MATCHES),
            "qrexecActions": strings(&["allow", "deny"]),
            "reservedQrexecServices": strings(schema::RESERVED_SERVICES),
        }
    })
}

/// Preserve static protocol enumeration order at the Nix export boundary.
fn strings(values: &[&str]) -> NixValue {
    NixValue::list(values.iter().map(|value| (*value).into()))
}

/// Install topology materialization and configuration transport using native application recipes.
pub(crate) fn module(
    source: &rusix::interop::NixPath,
    workspace: &airlock_build::Workspace,
) -> NixValue {
    use airlock_build::tools::Tool;
    use rusix::nixos::{ContextualModule, ModuleBody};

    ContextualModule::new(|context| {
        let cfg = context.config.clone().select("airlock.qubes");
        let access = cfg.clone().select("configAccessPolicy");
        let tools = Tool::packages(&context.pkgs, workspace, source);
        let configuration = tools.bind(|tools| {
            let packages = NixValue::concat_lists([
                NixValue::list(
                    [Tool::ClosureSmoke, Tool::Setup, Tool::StartProviders]
                        .map(|tool| tools.clone().select(tool.export_name())),
                ),
                NixValue::if_else(
                    access.clone().select("enable"),
                    NixValue::list(
                        [
                            Tool::Apply,
                            Tool::Archive,
                            Tool::BootstrapCurrent,
                            Tool::VerifyBoot,
                            Tool::Control,
                            Tool::ImportInputs,
                            Tool::Status,
                            Tool::ConfigSmoke,
                        ]
                        .map(|tool| tools.clone().select(tool.export_name())),
                    ),
                    NixValue::list([]),
                ),
            ]);
            let assertions = [
                ("configCurrentBootstrapOnBoot", "setupConfigOnBoot"),
                ("startNetworkProvidersOnBoot", "setupConfigOnBoot"),
                ("deferredSetupConfigOnBoot", "setupConfigOnBoot"),
                ("deferredSetupConfigOnBoot", "startNetworkProvidersOnBoot"),
                ("configCurrentBootstrapOnBoot", "configAccessPolicy"),
            ]
            .map(|(operation, prerequisite)| nix_record! {
                "assertion": cfg
                    .clone()
                    .select(operation)
                    .select("enable")
                    .equals(false)
                    .or(cfg.clone().select(prerequisite).select("enable")),
                "message": format!(
                    "airlock.qubes.{operation}.enable requires airlock.qubes.{prerequisite}.enable."
                ),
            });
            let base = nix_record! {
                "assertions": NixValue::list(assertions),
                "airlock": nix_record! {
                    "qubes": nix_record! {
                        "topology": normalize::topology(cfg.clone()),
                    },
                },
                "environment": nix_record! {
                    "systemPackages": packages,
                    "etc": nix_record! {
                        "airlock/qubes-topology.json": nix_record! {
                            "text": NixValue::concat_text([
                                NixValue::builtin("toJSON").call(cfg.clone().select("topology")),
                                "\n".into(),
                            ]),
                        },
                    },
                },
                "systemd": nix_record! {
                    "services": services::configuration(&context, &cfg, &tools),
                },
            };

            context.lib.clone().select("mkMerge").call(NixValue::list([
                base,
                context
                    .lib
                    .clone()
                    .select("mkIf")
                    .call(access.clone().select("enable"))
                    .call(access::configuration(&context, &access, &tools)),
            ]))
        });

        ModuleBody::new(
            context
                .lib
                .clone()
                .select("mkIf")
                .call(cfg.select("enable"))
                .call(configuration),
        )
        .key("airlock.topology")
        .import_value(options::declarations(&context.lib))
    })
    .into_expression()
}
