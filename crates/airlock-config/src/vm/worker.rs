//! Install private-store build commands and the controller's review RPC endpoints.

use super::WorkerRole;
use airlock_build::{Application, Workspace};
use rusix::{
    interop::{
        NixExpression, NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody},
};

/// Mount the worker's admitted pool view and private writable layer before qrexec starts.
pub(super) fn store(role: WorkerRole) -> NixValue {
    ContextualModule::new(|context| {
        let cfg = context.config.clone().select("airlock.qubeGuest");
        let pkgs = &context.pkgs;
        let workspace = Workspace::new(NixPath::from_source_text(
            pkgs.clone().select("airlockAppsSource").to_text().into_expr::<String>(),
        ));
        let quote = |value: NixValue| context.lib.clone().select("escapeShellArg").call(value);
        let tool = airlock_build::native::writer(pkgs, false)
            .call("usr-assistant-build-store")
            .call(nix_text!(
                r#"
                exec {cli}/bin/airlock builder-store \
                  "$@" --domain {role} --user {user} --nix-store {nix}/bin/nix-store
            "#,
                cli = Application::Cli.build(pkgs, &workspace),
                role = quote(role.identity().into()),
                user = quote(cfg.clone().select("user")),
                nix = pkgs.clone().select("nix"),
            ));
        let prerequisites = NixValue::list([
            "airlock-qubes-rw-init.service".into(),
            "dev-xvdg.device".into(),
            "dev-xvdh.device".into(),
        ]);
        let service = nix_record! {
            "description": format!(
                "Mount {role}'s pinned pool base and persistent private build layer",
                role = role.identity(),
            ),
            "wantedBy": NixValue::list(["multi-user.target".into()]),
            "requires": prerequisites.clone(),
            "after": prerequisites,
            "before": NixValue::list(["qubes-qrexec-agent.service".into()]),
            "path": NixValue::list([pkgs.clone().select("coreutils"), pkgs.clone().select("util-linux")]),
            "serviceConfig": nix_record! {
                "Type": "oneshot",
                "RemainAfterExit": true,
                "ExecStart": nix_text!("{tool} start", tool = tool.clone()),
                "ExecStop": nix_text!("{tool} stop", tool = tool),
                "TimeoutStartSec": 120_i64,
                // Build workers must see the mounts created by this service.
                "PrivateMounts": false,
            },
        };

        ModuleBody::new(nix_record! {
            "airlock": nix_record! {
                "qubeGuest": nix_record! {
                    "configNixStoreRoot": context.lib.select("mkDefault").call("/rw/airlock-builder/merged"),
                },
            },
            "assertions": NixValue::list([nix_record! {
                "assertion": cfg.select("configNixStoreRoot").equals("/rw/airlock-builder/merged"),
                "message": "the builder's persistent pool-backed store must use /rw/airlock-builder/merged",
            }]),
            "boot": nix_record! {
                "supportedFilesystems": NixValue::list(["erofs".into(), "overlay".into()]),
            },
            "nix": nix_record! {
                "settings": nix_record! {
                    "experimental-features": NixValue::list([
                        "local-overlay-store".into(),
                        "read-only-local-store".into(),
                    ]),
                },
            },
            "systemd": nix_record! {
                "services": NixValue::record([(
                    format!("airlock-{role}-build-store", role = role.identity()),
                    service,
                )]),
            },
        })
        .key(format!("airlock.vm.{role}.store", role = role.identity()))
    })
    .into_expression()
}

/// Seed an absent checkout from its admitted archive and retain offline input dependencies.
pub(super) fn seed() -> NixValue {
    ContextualModule::with_arguments(["specialArgs"], |context| {
        let arguments: NixValue = context.argument("specialArgs");
        let archive = arguments
            .clone()
            .attr_or("airlockMachineSeedArchive", NixValue::null());
        let offline = arguments.attr_or("airlockMachineOfflineInputs", NixValue::null());
        let cfg = context.config.select("airlock.qubeGuest");
        let native_workspace = Workspace::new(NixPath::from_source_text(context.pkgs.clone().select("airlockAppsSource").to_text().into_expr::<String>()));
        let native = Application::VmService.build(&context.pkgs, &native_workspace).as_expression();
        let dependencies =
            NixValue::builtin("map")
                .call(NixValue::builtin("storePath"))
                .call(
                    context.lib.clone().select("unique").call(
                        NixValue::builtin("attrValues").call(offline.clone().select("inputs")),
                    ),
                );
        let service = nix_record! {
            "description": "Seed the pinned shallow airlockos-machine checkout",
            "wantedBy": NixValue::list(["multi-user.target".into()]),
            "wants": NixValue::list(["airlock-qubes-rw-init.service".into()]),
            "requires": NixValue::list(["airlock-qubes-rw-init.service".into()]),
            "after": NixValue::list(["airlock-qubes-rw-init.service".into()]),
            "before": NixValue::list(["qubes-qrexec-agent.service".into()]),
            "path": NixValue::list(
                [
                    "coreutils",
                    "gitMinimal",
                    "gnugrep",
                    "gnutar",
                    "gzip",
                    "gawk",
                    "jq",
                ]
                .map(|name| context.pkgs.clone().select(name)),
            ),
            "script": airlock_build::native::hook(&context.pkgs, "airlock-machine-seed", crate::commands::contract(&context.pkgs, "machine-seed", [
                    ("service", native),
                    ("repo", cfg.clone().select("configRepoPath")),
                    ("user", cfg.clone().select("user")),
                    ("state", nix_text!("/home/{user}/.local/share/airlock/machine-build", user=cfg.clone().select("user")).into()),
                    ("cache", nix_text!("/home/{user}/.cache/airlock-machine-source", user=cfg.clone().select("user")).into()),
                    ("storeRoot", cfg.select("configNixStoreRoot")),
                    ("archive", archive.clone()),
                ])),
            "serviceConfig": nix_record! {
                "Type": "oneshot",
            },
        };

        ModuleBody::new(nix_record! {
            "system": nix_record! {
                "extraDependencies": NixValue::if_else(
                    offline.equals(NixValue::null()),
                    NixValue::list([]),
                    dependencies,
                ),
            },
            "systemd": nix_record! {
                "services": nix_record! {
                    "airlock-machine-source-seed": context
                        .lib
                        .select("mkIf")
                        .call(archive.equals(NixValue::null()).equals(false))
                        .call(service),
                },
            },
        })
        .key("airlock.vm.machine-source-seed")
    })
    .into_expression()
}

/// Configure build tooling for one declared worker role, including controller-only authority.
pub(super) fn module(role: WorkerRole) -> NixValue {
    ContextualModule::new(|context| {
        let cfg = context.config.clone().select("airlock.qubeGuest");
        let pkgs = &context.pkgs;
        let package = |name| pkgs.clone().select(name);
        let quote = |value: NixValue| context.lib.clone().select("escapeShellArg").call(value);
        let repo = cfg.clone().select("configRepoPath");
        let user = cfg.clone().select("user");
        let state: NixValue = nix_text!(
            "/home/{user}/.local/share/airlock/machine-build",
            user = user.clone(),
        )
        .into();
        let cache: NixValue = nix_text!(
            "/home/{user}/.cache/airlock-machine-source",
            user = user.clone(),
        )
        .into();
        let store = cfg.select("configNixStoreRoot");
        let store_directory: NixValue =
            nix_text!("{store}/nix/store", store = store.clone()).into();
        let binary = |name, script| {
            airlock_build::native::writer(&context.pkgs, true)
                .call(name)
                .call(script)
        };
        let native_workspace = Workspace::new(NixPath::from_source_text(
            package("airlockAppsSource").to_text().into_expr::<String>(),
        ));
        let native = Application::VmService
            .build(pkgs, &native_workspace)
            .as_expression();
        let runtime_path = context
            .lib
            .clone()
            .select("makeBinPath")
            .call(NixValue::list(
                ["coreutils", "gitMinimal", "util-linux", "gnumake"].map(package),
            ));
        let source_tool = binary(
            "airlock-usr-assistant-machine-source",
            crate::commands::contract(
                &context.pkgs,
                "machine-source",
                [
                    ("service", native.clone()),
                    ("repo", repo.clone()),
                    ("state", state.clone()),
                    ("cache", cache.clone()),
                    ("storeRoot", store.clone()),
                    ("path", runtime_path.clone()),
                ],
            ),
        );
        let build_tool = binary(
            "airlock-usr-assistant-machine-build",
            crate::commands::contract(
                &context.pkgs,
                "machine-build",
                [
                    ("service", native.clone()),
                    ("repo", repo),
                    ("state", state),
                    ("cache", cache),
                    ("storeRoot", store.clone()),
                    ("path", runtime_path),
                ],
            ),
        );
        let nix_store = binary(
            "airlock-config-nix-store",
            crate::commands::contract(
                &context.pkgs,
                "config-nix-store",
                [
                    ("service", native),
                    ("storeRoot", store),
                    ("nix", package("nix")),
                ],
            ),
        );

        let tools = nix_record! {
            "source": source_tool,
            "build": build_tool,
            "store": nix_store,
        };
        let configuration = tools.bind(|tools| {
            let workspace = Workspace::new(NixPath::from_source_text(
                package("airlockAppsSource").to_text().into_expr::<String>(),
            ));
            let mut runtime = vec![
                package("gitMinimal"),
                package("python3.pkgs.qubes-core-qrexec.domU"),
            ];
            let (name, application) = match role {
                WorkerRole::Development => ("airlock", Application::Cli),
                WorkerRole::Controller => {
                    runtime.extend([
                        tools.clone().select("source"),
                        tools.clone().select("build"),
                        tools.clone().select("store"),
                        Application::Cli.build(pkgs, &workspace).as_expression(),
                        package("coreutils"),
                        package("gnutar"),
                        package("gzip"),
                        package("gawk"),
                        package("jq"),
                        package("python3"),
                        package("systemd"),
                    ]);

                    ("airlock-sys-ctrl-service", Application::ControllerService)
                }
            };
            let executable = context
                .lib
                .clone()
                .select("getExe")
                .call(application.build(pkgs, &workspace));
            let command = package("writeShellApplication").call(nix_record! {
                "name": name,
                "runtimeInputs": NixValue::list(runtime),
                "text": nix_text!(
                    r#"
                export AIRLOCK_ARTIFACT_PHYSICAL_STORE={store}
                export AIRLOCK_ARTIFACT_NIX=airlock-config-nix
                exec {executable} "$@"
            "#,
                    store = quote(store_directory),
                    executable = executable,
                ),
            });

            command.bind(|command| {
                let extra = match role {
                    WorkerRole::Development => nix_record! {},
                    WorkerRole::Controller => controller(pkgs, command.clone(), user),
                };
                let environment = extra
                    .clone()
                    .attr_or("environment", nix_record! {})
                    .merge_attrs(nix_record! {
                        "systemPackages": NixValue::list([
                            tools.clone().select("source"),
                            tools.clone().select("build"),
                            tools.select("store"),
                            command,
                            package("gitMinimal"),
                            package("gnutar"),
                        ]),
                    });

                extra.merge_attrs(nix_record! {
                    "environment": environment,
                })
            })
        });

        ModuleBody::new(configuration)
            .key(format!("airlock.vm.{role}.build", role = role.identity()))
    })
    .into_expression()
}

/// Restrict review endpoints and background observation to the controller VM.
fn controller(pkgs: &NixValue, command: NixValue, user: NixValue) -> NixValue {
    let wrapper = |name, script| {
        airlock_build::native::writer(pkgs, false)
            .call(name)
            .call(script)
    };
    let endpoint = |script| {
        nix_record! {
            "source": script,
            "mode": "0555",
        }
    };
    let rpc_config = nix_record! {
        "text": nix_text!("force-user='{user}'\n", user = user.clone()),
    };

    nix_record! {
        "systemd": nix_record! {
            "services": nix_record! {
                "airlock-controller-observe": nix_record! {
                    "description": "Record authoritative dom0 deployment outcomes",
                    "after": NixValue::list(["qubes-qrexec-agent.service".into()]),
                    "serviceConfig": nix_record! {
                        "Type": "oneshot",
                        "User": user,
                        "ExecStart": nix_text!(
                            "{command}/bin/airlock-sys-ctrl-service observe",
                            command = command.clone(),
                        ),
                        "TimeoutStartSec": 30_i64,
                    },
                },
            },
            "timers": nix_record! {
                "airlock-controller-observe": nix_record! {
                    "wantedBy": NixValue::list(["timers.target".into()]),
                    "timerConfig": nix_record! {
                        "OnBootSec": "1min",
                        "OnUnitInactiveSec": "1min",
                    },
                },
            },
        },
        "environment": nix_record! {
            "etc": nix_record! {
                "qubes-rpc/airlock.Ctrl.GuestDeploy": endpoint(wrapper(
                    "airlock-ctrl-guest-deploy-qrexec",
                    nix_text!(
                        r#"
                [ "${{QREXEC_REMOTE_DOMAIN:-}}" = usr-assistant ] || exit 1
                exec {command}/bin/airlock-sys-ctrl-service serve
            "#,
                        command = command.clone(),
                    ),
                )),
                "airlock/controller-review-required": nix_record! {
                    "text": "v1\n",
                },
                "qubes/rpc-config/airlock.Ctrl.GuestDeploy": rpc_config.clone(),
                "qubes-rpc/airlock.Ctrl.ReviewStatus": endpoint(wrapper(
                    "airlock-review-status-qrexec",
                    nix_text!(
                        r#"
                exec {command}/bin/airlock-sys-ctrl-service review-status-serve
            "#,
                        command = command,
                    ),
                )),
                "qubes/rpc-config/airlock.Ctrl.ReviewStatus": rpc_config,
            },
        },
    }
}
