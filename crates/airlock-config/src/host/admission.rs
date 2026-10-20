//! Admit inactive VM store generations through a separately validated privileged receiver.

use airlock_build::{Application, Workspace};
use rusix::{
    interop::{
        NixExpression,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody, ModuleContext},
};

/// Install full and incremental pool admission for the configured controller and VM targets.
pub(super) fn module(workspace: &Workspace) -> NixValue {
    ContextualModule::new(|context| {
        let cfg = context.config.clone().select("airlock.poolAdmission");
        let store = Application::Dom0
            .build(&context.pkgs, workspace)
            .as_expression();
        let config = store.bind(|store| configuration(&context, &cfg, store));
        let domain = context
            .lib
            .clone()
            .select("types.strMatching")
            .call("[A-Za-z][A-Za-z0-9_.-]*");
        let option = |kind, default, description| {
            context.lib.clone().select("mkOption").call(nix_record! {
                "type": kind,
                "default": default,
                "description": description,
            })
        };
        let options = nix_record! {
            "options": nix_record! {
                "airlock": nix_record! {
                    "poolAdmission": nix_record! {
                        "enable": context
                            .lib
                            .clone()
                            .select("mkEnableOption")
                            .call("VM artifact admission into the shared pool"),
                        "caller": option(
                            domain.clone(),
                            "usr-assistant".into(),
                            "Qubes VM allowed to submit pool artifacts.",
                        ),
                        "targets": option(
                            context.lib.clone().select("types.listOf").call(domain),
                            NixValue::list([]),
                            "VMs for which the caller may submit pool artifacts.",
                        ),
                    },
                },
            },
        };

        ModuleBody::new(
            context
                .lib
                .clone()
                .select("mkIf")
                .call(cfg.select("enable"))
                .call(config),
        )
        .key("airlock.host.pool-admission")
        .import_value(options)
    })
    .into_expression()
}

/// Give sudo authority to exact receiver programs, never to an unrestricted Nix or service command.
fn configuration(context: &ModuleContext, cfg: &NixValue, store: NixValue) -> NixValue {
    let targets = cfg.clone().select("targets");
    let caller = cfg.clone().select("caller");
    let target_args = context
        .lib
        .clone()
        .select("concatMap")
        .call(NixValue::function(|target| {
            NixValue::list(["--target".into(), target])
        }))
        .call(targets.clone());
    let mut files = Vec::new();
    let mut commands = Vec::new();
    let mut policy = Vec::new();
    for (rpc, suffix) in [
        ("airlock.Pool.Admit", ""),
        ("airlock.Pool.AdmitIncremental", "-incremental"),
    ] {
        let program: NixValue =
            nix_text!("{store}/bin/airlock-dom0-service", store = store.clone()).into();
        let receiver_args = NixValue::concat_lists([
            NixValue::list([
                "admission-adapter".into(),
                "receive".into(),
                "--caller".into(),
                caller.clone(),
                "--service".into(),
                program.clone(),
            ]),
            target_args.clone(),
            NixValue::list(if suffix.is_empty() {
                vec![]
            } else {
                vec!["--incremental".into()]
            }),
            NixValue::list(["--".into()]),
        ]);
        let receiver = airlock_build::native::command(
            &context.pkgs,
            &format!("airlock-pool-admit-root{suffix}"),
            nix_record! {
                "program": program.clone(), "args": receiver_args, "argument_count": 2_i64,
            },
            false,
        );
        let handler = airlock_build::native::command(
            &context.pkgs,
            &format!("airlock-pool-admit-qrexec{suffix}"),
            nix_record! {
                "program": program,
                "args": NixValue::concat_lists([
                    NixValue::list(["admission-adapter".into(), "handler".into(), "--caller".into(), caller.clone(), "--receiver".into(), receiver.clone()]),
                    target_args.clone(),
                ]),
                "inherit_args": false,
            },
            false,
        );
        files.push((
            format!("qubes-rpc/{rpc}"),
            nix_record! {
                "source": handler,
                "mode": "0555",
            },
        ));
        commands.push(nix_record! {
            "command": receiver.to_text(),
            "options": NixValue::list(["NOPASSWD".into()]),
        });
        let rules = context
            .lib
            .clone()
            .select("concatMapStringsSep")
            .call("\n")
            .call(NixValue::function(|target| {
                nix_text!(
                    "{rpc} +{target} {caller} dom0 allow",
                    rpc = rpc,
                    target = target,
                    caller = cfg.clone().select("caller"),
                )
                .into()
            }))
            .call(targets.clone());
        policy.push(
            nix_text!(
                r#"
            {rules}
            {rpc} * @anyvm @anyvm deny
        "#,
                rules = rules,
                rpc = rpc,
            )
            .into(),
        );
    }
    let policy = context
        .pkgs
        .clone()
        .select("writeTextDir")
        .call("etc/qubes/policy.d/42-airlock-pool-admission.policy")
        .call(NixValue::concat_text(policy));

    nix_record! {
        "assertions": NixValue::list([nix_record! {
            "assertion": targets.equals(NixValue::list([])).equals(false),
            "message": "Pool admission requires at least one permitted VM target.",
        }]),
        "environment": nix_record! {
            "etc": NixValue::record(files),
        },
        "security": nix_record! {
            "sudo": nix_record! {
                "extraRules": NixValue::list([nix_record! {
                    "users": NixValue::list([context.config.clone().select("airlock.dom0.user")]),
                    "commands": NixValue::list(commands),
                }]),
            },
        },
        "virtualisation": nix_record! {
            "qubes": nix_record! {
                "packages": NixValue::list([policy]),
            },
        },
    }
}
