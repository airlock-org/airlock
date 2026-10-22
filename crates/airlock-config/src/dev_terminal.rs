//! Keep the explicitly selected development terminal behind Qubes policy and normal dom0 sudo.
//! The terminal transports an unprivileged PTY; it grants no password-free administration.

use rusix::{
    interop::{NixExpression, raw::NixValue},
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody, ModuleContext},
};

/// Install the development terminal only on the host and VMs selected by the machine definition.
pub(crate) fn module() -> NixValue {
    ContextualModule::new(|context| {
        let cfg = context.config.clone().select("airlock.devDom0Terminal");
        let config = context.lib.clone().select("mkMerge").call(NixValue::list([
            context
                .lib
                .clone()
                .select("mkIf")
                .call(cfg.clone().select("host.enable"))
                .call(host(&context, &cfg)),
            context
                .lib
                .clone()
                .select("mkIf")
                .call(cfg.clone().select("guest.enable"))
                .call(client(&context)),
        ]));

        ModuleBody::new(config)
            .key("airlock.development.dom0-terminal")
            .import_value(options(&context))
    })
    .into_expression()
}

/// Declare explicit development-only placement and permitted caller identities.
fn options(context: &ModuleContext) -> NixValue {
    let enable = |description| {
        context
            .lib
            .clone()
            .select("mkEnableOption")
            .call(description)
    };
    let matching = |pattern| {
        context
            .lib
            .clone()
            .select("types.strMatching")
            .call(pattern)
    };
    let option = |kind, default, description| {
        context.lib.clone().select("mkOption").call(nix_record! {
            "type": kind,
            "default": default,
            "description": description,
        })
    };

    nix_record! {
        "options": nix_record! {
            "airlock": nix_record! {
                "devDom0Terminal": nix_record! {
                    "host": nix_record! {
                        "enable": enable("the development-only dom0 shell RPC"),
                    },
                    "guest": nix_record! {
                        "enable": enable("the development-only dom0 terminal launcher"),
                    },
                    "callers": option(
                        context
                            .lib
                            .clone()
                            .select("types.listOf")
                            .call(matching("[A-Za-z][A-Za-z0-9_.-]*")),
                        NixValue::list(["sys-gui".into()]),
                        "Qubes VMs permitted to open the development-only dom0 shell.",
                    ),
                    "user": option(
                        matching("[a-z_][a-z0-9_-]*"),
                        "airlock".into(),
                        "Unprivileged dom0 account; its normal sudo policy still applies.",
                    ),
                },
            },
        },
    }
}

/// Admit only configured callers and run the PTY as the existing ordinary dom0 user.
fn host(context: &ModuleContext, cfg: &NixValue) -> NixValue {
    let package = |name| context.pkgs.clone().select(name);
    let shell = airlock_build::native::writer(&context.pkgs, false)
        .call("airlock-dev-dom0-login")
        .call(crate::commands::contract(
            &context.pkgs,
            "shell",
            [
                ("utilLinux", package("util-linux")),
                ("ncurses", package("ncurses")),
                ("coreutils", package("coreutils")),
                ("bash", package("bashInteractive")),
            ],
        ));
    let handler = airlock_build::native::writer(&context.pkgs, false)
        .call("airlock-dev-dom0-shell")
        .call(crate::commands::contract(
            &context.pkgs,
            "handler",
            [
                ("utilLinux", package("util-linux")),
                ("coreutils", package("coreutils")),
                (
                    "callers",
                    context
                        .lib
                        .clone()
                        .select("concatStringsSep")
                        .call(" ")
                        .call(cfg.clone().select("callers")),
                ),
                ("user", cfg.clone().select("user")),
                ("shell", shell),
            ],
        ));
    let user = NixValue::builtin("getAttr")
        .call(cfg.clone().select("user"))
        .call(context.config.clone().select("users.users"));
    let rules = context
        .lib
        .clone()
        .select("concatMapStringsSep")
        .call("\n")
        .call(NixValue::function(|caller| {
            nix_text!(
                "airlock.DevDom0Shell + {caller} dom0 allow",
                caller = caller,
            )
            .into()
        }))
        .call(cfg.clone().select("callers"));
    let policy = package("runCommand")
        .call("airlock-dev-dom0-terminal-policy")
        .call(nix_record! {})
        .call(nix_text!(
            r#"
            install -d "$out/etc/qubes/policy.d"
            cat > "$out/etc/qubes/policy.d/40-airlock-dev-dom0-terminal.policy" <<'EOF'
            # Intentional development-only dom0 user shell.
            {rules}
            airlock.DevDom0Shell * @anyvm dom0 deny
            airlock.DevDom0Shell * @anyvm @anyvm deny
            EOF
        "#,
            rules = rules,
        ));

    nix_record! {
        "assertions": NixValue::list([nix_record! {
            "assertion": context
                .config
                .clone()
                .select("airlock.image.devMode.enable")
                .and(context.config.clone().select("airlock.dom0.enable"))
                .and(user.select("isNormalUser")),
            "message": "The dom0 terminal requires a development image and an ordinary dom0 user.",
        }]),
        "virtualisation": nix_record! {
            "qubes": nix_record! {
                "packages": NixValue::list([policy]),
            },
        },
        "environment": nix_record! {
            "etc": nix_record! {
                "qubes-rpc/airlock.DevDom0Shell": nix_record! {
                    "source": handler,
                    "mode": "symlink",
                },
            },
        },
    }
}

/// Expose a terminal launcher and direct command using the same bounded Qubes transport.
fn client(context: &ModuleContext) -> NixValue {
    let package = |name| context.pkgs.clone().select(name);
    let client = airlock_build::native::writer(&context.pkgs, false)
        .call("airlock-dev-dom0-terminal-client")
        .call(crate::commands::contract(
            &context.pkgs,
            "client",
            [
                ("coreutils", package("coreutils")),
                ("qrexec", package("python3.pkgs.qubes-core-qrexec.domU")),
            ],
        ));

    client.bind(|client| {
        let command = airlock_build::native::writer(&context.pkgs, true)
            .call("airlock-dom0-shell")
            .call(nix_text!(
                r#"
            exec {client} "$@"
        "#,
                client = client.clone(),
            ));
        let launcher = airlock_build::native::writer(&context.pkgs, true)
            .call("airlock-dom0-terminal")
            .call(nix_text!(
                r#"
            exec {xterm}/bin/xterm -hold -T 'dom0 terminal — DEVELOPMENT' \
              -bg '#1e1e2e' -fg '#cdd6f4' -cr '#f5e0dc' \
              -fa 'monospace' -fs 18 \
              -geometry 100x30 -e {client}
        "#,
                xterm = package("xterm"),
                client = client,
            ));
        let desktop = package("makeDesktopItem").call(nix_record! {
            "name": "airlock-dom0-terminal",
            "desktopName": "dom0 Terminal (Development)",
            "comment": "Administrative dom0 access; sudo requires the dom0 password",
            "exec": "airlock-dom0-terminal",
            "icon": "utilities-terminal",
            "categories": NixValue::list(["System".into(), "TerminalEmulator".into()]),
        });

        nix_record! {
            "environment": nix_record! {
                "systemPackages": NixValue::list([command, launcher, desktop]),
            },
        }
    })
}
