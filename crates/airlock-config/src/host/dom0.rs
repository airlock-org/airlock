//! Assemble the Qubes control domain around upstream Xen, qrexec, QubesDB and qubesd.
//! Machine policy selects capabilities; native services implement storage and deployment operations.

mod options;

mod policy;

mod network;

use airlock_build::{Application, Workspace, adapters, tools::Tool, upstream};
use rusix::{
    interop::{
        NixExpression, NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
    nixos::{ContextualModule, ModuleBody, ModuleContext},
};

/// Compose host services with explicit application and machine asset sources.
pub(crate) fn module(source: &NixPath, workspace: &Workspace) -> NixValue {
    ContextualModule::new(|context| {
        let system = Host {
            context: &context,
            source,
            workspace,
        };
        let config = system.configuration();

        ModuleBody::new(system.when(system.setting("enable"), config))
            .key("airlock.host.dom0")
            .import_value(options::declarations(&context))
            .import_value(super::setup::module(source, workspace))
            .import_value(crate::dev_terminal::module())
            .import_value(super::admission::module(workspace))
            .import_value(crate::input::module(source, workspace))
            .import_value(super::vbios::module(source, workspace))
            .import_value(super::gpu::module(source, workspace))
            .import_value(super::boot::module(source, workspace))
    })
    .into_expression()
}

/// Sources and final NixOS context needed by the control-domain composition.
struct Host<'a> {
    /// Host package set, merged settings and upstream library.
    context: &'a ModuleContext,
    /// Machine-owned Qubes patches and shell integration assets.
    source: &'a NixPath,
    /// Native application workspace used by runtime package recipes.
    workspace: &'a Workspace,
}

impl Host<'_> {
    /// Read a dom0 capability or resource setting after machine overrides are merged.
    fn setting(&self, name: &str) -> NixValue {
        self.context
            .config
            .clone()
            .select("airlock.dom0")
            .select(name)
    }

    /// Select a host package while retaining the machine's overlays.
    fn package(&self, name: &str) -> NixValue {
        self.context.pkgs.clone().select(name)
    }

    /// Supply an ordinary machine-overridable value.
    fn default(&self, value: impl Into<NixValue>) -> NixValue {
        self.context
            .lib
            .clone()
            .select("mkDefault")
            .call(value.into())
    }

    /// Contribute definitions only when the selected capability is enabled.
    fn when(&self, enabled: NixValue, value: NixValue) -> NixValue {
        self.context
            .lib
            .clone()
            .select("mkIf")
            .call(enabled)
            .call(value)
    }

    /// Select optional packages or service dependencies lazily.
    fn optional(&self, enabled: NixValue, value: NixValue) -> NixValue {
        NixValue::if_else(enabled, value, NixValue::list([]))
    }

    /// Combine Qubes baseline, storage integration, GUI relay and development networking.
    fn configuration(&self) -> NixValue {
        let store = Application::Dom0
            .build(&self.context.pkgs, self.workspace)
            .as_expression();

        store.bind(|store| {
            self.context
                .lib
                .clone()
                .select("mkMerge")
                .call(NixValue::list([
                    self.base(),
                    self.storage(&store),
                    self.when(self.setting("gui.enable"), self.gui()),
                    network::configuration(self),
                ]))
        })
    }

    /// Keep Qubes in charge of lifecycle and preserve the selected Xen host memory allocation.
    fn base(&self) -> NixValue {
        let memory = self.setting("dom0Resources.memoryMiB");
        let maximum = self.setting("dom0Resources.maxMemoryMiB");
        let opt_outs = NixValue::concat_lists([
            strings(&["dedicated-sys-net", "dedicated-sys-usb"]),
            self.optional(
                self.setting("gui.enable").equals(false),
                strings(&["dom0-is-gui"]),
            ),
            self.setting("extraOptOutRecommendedConfiguration"),
        ]);
        let unfree = NixValue::function(|package| {
            let name = package.clone().attr_or(
                "pname",
                self.context.lib.clone().select("getName").call(package),
            );

            NixValue::builtin("elem")
                .call(name)
                .call(strings(&["qubes-artwork", "qubes-vmm-stubdom-linux"]))
        });
        let tools = [Tool::BootstrapAssets, Tool::GuiSmoke, Tool::QrexecSmoke].map(|tool| {
            tool.build(&self.context.pkgs, self.workspace, self.source)
                .as_expression()
        });
        let packages = NixValue::concat_lists([
            self.optional(
                self.setting("observability.enable"),
                NixValue::list([upstream::observation(&self.context.pkgs).as_expression()]),
            ),
            self.optional(self.setting("testingTools.enable"), NixValue::list(tools)),
        ]);

        nix_record! {
            "assertions": NixValue::list([
                assertion(
                    self.setting("user").equals("").equals(false),
                    "Dom0 requires a local Qubes control user.",
                ),
                assertion(
                    NixValue::builtin("lessThan")
                        .call(0_i64)
                        .call(memory.clone()),
                    "Dom0 memory must be greater than zero.",
                ),
                assertion(
                    NixValue::builtin("lessThan")
                        .call(maximum.clone())
                        .call(memory.clone())
                        .equals(false),
                    "Maximum dom0 memory must not be below its initial allocation.",
                ),
            ]),
            "nixpkgs": nix_record! {
                "config": nix_record! {
                    "allowUnfreePredicate": self.default(unfree),
                },
            },
            "boot": nix_record! {
                "initrd": nix_record! {
                    "systemd": nix_record! {
                        "enable": self.default(true),
                    },
                },
            },
            "time": nix_record! {
                "timeZone": self.default("UTC"),
            },
            "virtualisation": nix_record! {
                "qubes": nix_record! {
                    "dom0": nix_record! {
                        "enable": true,
                    },
                    "user": self.setting("user"),
                    // This POC still uses upstream's explicitly insecure control-domain mode.
                    "secure": false,
                    "optOutRecommendedConfiguration": self.context.lib.clone().select("mkBefore").call(opt_outs),
                },
                "xen": nix_record! {
                    "boot": nix_record! {
                        "builderVerbosity": self.default("info"),
                    },
                    "dom0Resources": nix_record! {
                        "memory": self.default(memory),
                        "maxMemory": self.default(maximum),
                    },
                },
                "libvirtd": nix_record! {
                    "firewallBackend": self
                        .context
                        .lib
                        .clone()
                        .select("mkForce")
                        .call(self.setting("libvirtFirewallBackend")),
                    // Libvirt must not resume saved VMs before Qubes reconciles the selected topology.
                    "onBoot": "ignore",
                    "onShutdown": "shutdown",
                },
            },
            "environment": nix_record! {
                "systemPackages": packages,
                "etc": nix_record! {
                    "airlock/dev-mode": self.when(
                        self.context
                            .config
                            .clone()
                            .select("airlock.image.devMode.enable"),
                        nix_record! {
                            "text": nix_text!(r#"
                            enabled
                        "#),
                        },
                    ),
                },
            },
        }
    }

    /// Initialize the Qubes database and install the store adapter before qubesd starts.
    fn storage(&self, store: &NixValue) -> NixValue {
        let adapter = adapters::qubes(&self.context.pkgs, self.workspace).as_expression();
        let python_path = NixValue::concat_text([
            nix_text!(
                "{adapter}/{site}",
                adapter = adapter,
                site = self.package("python3.sitePackages"),
            )
            .into(),
            NixValue::if_else(
                self.context
                    .config
                    .clone()
                    .select("airlock.amdVbios.enable"),
                nix_text!(
                    ":{vbios}/{site}",
                    vbios = self
                        .context
                        .config
                        .clone()
                        .select("airlock.amdVbios.package"),
                    site = self.package("python3.sitePackages"),
                ),
                "",
            ),
        ]);
        let menu = airlock_build::native::writer(&self.context.pkgs, false)
            .call("airlock-boot-menu")
            .call(nix_text!(
                r#"
            exec {store}/bin/airlock-dom0-service storage host boot-menu \
              --esp {esp} \
              --recovery-profile /nix/var/nix/profiles/system-profiles/airlock-recovery \
              --nix-env {nix}/bin/nix-env
        "#,
                store = store.clone(),
                esp = self.context.lib.clone().select("escapeShellArg").call(
                    self.context
                        .config
                        .clone()
                        .select("boot.loader.efi.efiSysMountPoint"),
                ),
                nix = self.context.config.clone().select("nix.package"),
            ));
        let labels = airlock_build::native::hook(
            &self.context.pkgs,
            "airlock-boot-labels",
            nix_record! {
                "program": nix_text!("{store}/bin/airlock-dom0-service", store = store.clone()),
                "args": NixValue::concat_lists([
                    strings(&["storage", "host", "boot-labels", "--esp"]),
                    NixValue::list([self.context.config.clone().select("boot.loader.efi.efiSysMountPoint")]),
                    NixValue::if_else(self.setting("bootMenu.enable"), NixValue::list(["--menu".into(), menu.to_text()]), NixValue::list([])),
                ]),
                "inherit_args": false,
            },
        );

        nix_record! {
            "environment": nix_record! {
                "etc": nix_record! {
                    "airlock/boot-source.json": self.when(
                        self.setting("bootMenu.source")
                            .equals(NixValue::null())
                            .equals(false),
                        nix_record! {
                            "text": NixValue::builtin("toJSON")
                                .call(self.setting("bootMenu.source")),
                        },
                    ),
                },
            },
            "boot": nix_record! {
                "loader": nix_record! {
                    "systemd-boot": nix_record! {
                        "extraInstallCommands": self.context.lib.clone().select("mkAfter").call(labels),
                    },
                },
            },
            "systemd": nix_record! {
                "tmpfiles": nix_record! {
                    "rules": strings(&[
                        "d /var/lib/airlock 0700 root root - -",
                        "d /var/lib/airlock/closure-store 0700 root root - -",
                    ]),
                },
                "services": nix_record! {
                    "airlock-qubes-store-init": nix_record! {
                        "description": "Initialize the Qubes XML store for Airlock dom0",
                        "after": strings(&["systemd-tmpfiles-setup.service"]),
                        "before": strings(&["qubesd.service"]),
                        "requiredBy": strings(&["qubesd.service"]),
                        "serviceConfig": nix_record! {
                            "Type": "oneshot",
                            "RemainAfterExit": true,
                        },
                        "script": nix_text!(
                            r#"
                            if [ ! -f /var/lib/qubes/qubes.xml ]; then
                              echo "setting up Qubes XML store..."
                              {admin}/bin/qubes-create --offline-mode
                            fi
                        "#,
                            admin = self.package("python3.pkgs.qubes-core-admin"),
                        ),
                    },
                    "qubesd": nix_record! {
                        "after": strings(&["airlock-qubes-store-init.service"]),
                        "requires": strings(&["airlock-qubes-store-init.service"]),
                        "environment": nix_record! {
                            "PYTHONPATH": python_path,
                        },
                        "path": NixValue::list(
                            ["erofs-utils", "lvm2", "sudo", "util-linux"]
                                .map(|name| self.package(name)),
                        ),
                    },
                },
            },
        }
    }

    /// Relay the desktop to sys-gui; authentication and the greeter remain inside that VM.
    fn gui(&self) -> NixValue {
        let xorg = airlock_build::native::writer(&self.context.pkgs, false)
            .call("X-qubes-shmoverride")
            .call(crate::commands::contract(
                &self.context.pkgs,
                "xorg",
                [
                    ("daemon", self.package("qubes-gui-daemon")),
                    ("xserver", self.package("xorg-server")),
                ],
            ));
        let poweroff = airlock_build::native::writer(&self.context.pkgs, false)
            .call("airlock-dom0-poweroff")
            .call(crate::commands::contract(
                &self.context.pkgs,
                "poweroff",
                [("systemd", self.package("systemd"))],
            ));
        let session = self
            .package("runCommand")
            .call("airlock-sys-gui-session")
            .call(nix_record! {
                "passthru": nix_record! {
                    "providedSessions": strings(&["sys-gui"]),
                },
            })
            .call(nix_text!(
                r#"
                install -d "$out/share/xsessions"
                cat > "$out/share/xsessions/sys-gui.desktop" <<EOF
                [Desktop Entry]
                Name=GUI Domain (sys-gui)
                Exec={admin}/bin/qubes-guivm-session sys-gui
                Type=Application
                EOF
            "#,
                admin = self.package("python3.pkgs.qubes-core-admin-client"),
            ));

        nix_record! {
            "services": nix_record! {
                "xserver": nix_record! {
                    "desktopManager": nix_record! {
                        "xfce": nix_record! {
                            "enable": false,
                        },
                        "xterm": nix_record! {
                            "enable": false,
                        },
                    },
                    "displayManager": nix_record! {
                        "xserverBin": self
                            .context
                            .lib
                            .clone()
                            .select("mkOverride")
                            .call(50_i64)
                            .call(xorg),
                        "lightdm": nix_record! {
                            "greeter": nix_record! {
                                "enable": false,
                            },
                            "greeters": nix_record! {
                                "gtk": nix_record! {
                                    "enable": false,
                                },
                            },
                            "autoLogin": nix_record! {
                                "timeout": 0_i64,
                            },
                        },
                    },
                },
                "displayManager": nix_record! {
                    "sessionPackages": NixValue::list([session]),
                    "defaultSession": self
                        .context
                        .lib
                        .clone()
                        .select("mkOverride")
                        .call(40_i64)
                        .call("sys-gui"),
                    "autoLogin": nix_record! {
                        "enable": true,
                        "user": self.setting("user"),
                    },
                },
            },
            "systemd": nix_record! {
                "services": nix_record! {
                    "display-manager": nix_record! {
                        "after": strings(&["airlock-setup-config.service"]),
                    },
                },
            },
            "airlock": nix_record! {
                "qubes": nix_record! {
                    "setupConfigOnBoot": nix_record! {
                        "statusDevice": self.default("/dev/tty1"),
                    },
                },
            },
            "virtualisation": nix_record! {
                "qubes": nix_record! {
                    "packages": NixValue::list([policy::gui(self.context)]),
                },
            },
            "environment": nix_record! {
                "etc": nix_record! {
                    "qubes-rpc/airlock.Dom0Poweroff": nix_record! {
                        "source": poweroff,
                        "mode": "0555",
                    },
                },
            },
            "security": nix_record! {
                "sudo": nix_record! {
                    "extraRules": NixValue::list([nix_record! {
                        "users": NixValue::list([self.setting("user")]),
                        "commands": NixValue::list([nix_record! {
                            "command": nix_text!(
                                "{systemd}/bin/systemctl poweroff",
                                systemd = self.package("systemd"),
                            ),
                            "options": strings(&["NOPASSWD"]),
                        }]),
                    }]),
                },
            },
        }
    }
}

/// Pair a deferred invariant with the NixOS evaluation error presented to the machine author.
fn assertion(condition: NixValue, message: &str) -> NixValue {
    nix_record! {
        "assertion": condition,
        "message": message,
    }
}

/// Preserve declared package, service and rule order.
fn strings(values: &[&str]) -> NixValue {
    NixValue::list(values.iter().map(|value| (*value).into()))
}
