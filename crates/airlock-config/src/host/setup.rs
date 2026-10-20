//! Restrict first-login setup and boot-status operations to the GUI VM's Qubes identity.

use airlock_build::{Workspace, tools::Tool};
use rusix::{
    interop::{
        NixExpression, NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody, ModuleContext},
};

/// Install first-login setup state and its narrowly scoped GUI-to-dom0 operations.
pub(super) fn module(source: &NixPath, workspace: &Workspace) -> NixValue {
    ContextualModule::new(|context| {
        let service = Tool::Dom0
            .build(&context.pkgs, workspace, source)
            .as_expression();
        let configuration = service.bind(|service| {
            let rpcs = desktop_rpcs(
                &context,
                &service,
                "setup",
                "40-airlock-setup.policy",
                &[
                    ("airlock.SetupStatus", "status"),
                    ("airlock.SetupComplete", "complete"),
                ],
            );

            rpcs.merge_attrs(nix_record! {
                "systemd": nix_record! {
                    "tmpfiles": nix_record! {
                        "rules": NixValue::list(
                            ["d /var/lib/airlock/setup 0700 root root - -".into()],
                        ),
                    },
                },
            })
        });
        let enabled = context
            .config
            .clone()
            .select("airlock.dom0.enable")
            .and(context.config.clone().select("airlock.dom0.gui.enable"));

        ModuleBody::new(
            context
                .lib
                .clone()
                .select("mkIf")
                .call(enabled)
                .call(configuration),
        )
        .key("airlock.host.setup")
    })
    .into_expression()
}

/// Pair GUI-only RPC handlers with matching Qubes policy and exact password-free sudo commands.
/// Runtime command parsing and validation remain in the native dom0 service.
pub(super) fn desktop_rpcs(
    context: &ModuleContext,
    service: &NixValue,
    group: &str,
    policy_name: &str,
    operations: &[(&str, &str)],
) -> NixValue {
    let mut files = Vec::new();
    let mut commands = Vec::new();
    let mut rules = String::new();
    for (rpc, operation) in operations {
        let command: NixValue = nix_text!(
            "{service}/bin/airlock-dom0-service {group} {operation}",
            service = service.clone(),
            group = group,
            operation = *operation,
        )
        .into();
        let handler = context
            .pkgs
            .clone()
            .select("writeShellScript")
            .call(format!("airlock-{group}-{operation}-rpc"))
            .call(nix_text!(
                r#"
                set -eu
                [ "${{QREXEC_REMOTE_DOMAIN:-}}" = sys-gui ] || exit 1
                [ -z "${{QREXEC_SERVICE_ARGUMENT:-}}" ] || exit 1
                exec /run/wrappers/bin/sudo -n {command}
            "#,
                command = command.clone(),
            ));
        files.push((
            format!("qubes-rpc/{rpc}"),
            nix_record! {
                "source": handler,
                "mode": "0555",
            },
        ));
        commands.push(nix_record! {
            "command": command,
            "options": NixValue::list(["NOPASSWD".into()]),
        });
        rules.push_str(&format!(
            r#"{rpc} + sys-gui dom0 allow
{rpc} * @anyvm @anyvm deny
"#
        ));
    }
    let policy = context
        .pkgs
        .clone()
        .select("writeTextDir")
        .call(format!("etc/qubes/policy.d/{policy_name}"))
        .call(rules);

    nix_record! {
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
