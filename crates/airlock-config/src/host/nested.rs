//! Define disposable nested Xen hosts and their minimal graphics validation profile.

use airlock_build::Workspace;
use rusix::{
    interop::{
        NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record,
    nixos::{ContextualModule, ModuleBody},
};

/// Export host profiles for image assembly using the caller's machine and application sources.
pub(crate) fn profiles(source: &NixPath, workspace: &Workspace) -> NixValue {
    nix_record! {
        "test": test(),
        "vm": vm(false),
        "vmGui": vm(true),
        "development": nix_record! {
            "airlock": nix_record! {
                "image": nix_record! {
                    "devMode": nix_record! {
                        "enable": true,
                    },
                },
            },
        },
        "configSystem": config_system(source, workspace),
        "guiMinimal": gui_minimal(),
    }
}

/// Provide a small bootable host for the upstream Qubes module contract.
fn test() -> NixValue {
    ContextualModule::new(|context| {
        ModuleBody::new(nix_record! {
            "airlock": nix_record! {
                "dom0": nix_record! {
                    "enable": true,
                    "user": "airlock",
                    "gui": nix_record! {
                        "enable": false,
                    },
                },
            },
            "users": nix_record! {
                "users": nix_record! {
                    "airlock": nix_record! {
                        "isNormalUser": true,
                        "description": "Airlock test operator",
                        "extraGroups": strings(&["wheel"]),
                    },
                },
            },
            "boot": boot(&context.lib),
            "fileSystems": nix_record! {
                "/": nix_record! {
                    "device": "/dev/disk/by-label/airlock-root",
                    "fsType": "ext4",
                },
            },
            "networking": nix_record! {
                "hostName": context.lib.select("mkDefault").call("dom0"),
            },
            "system": nix_record! {
                "stateVersion": "26.05",
            },
        })
        .key("airlock.host.test")
    })
    .into_expression()
}

/// Configure a nested QCOW2 host, optionally forwarding its display through the GUI VM.
pub(crate) fn vm(gui: bool) -> NixValue {
    ContextualModule::with_arguments(["modulesPath"], |context| {
        let default = |value: NixValue| context.lib.clone().select("mkDefault").call(value);
        let boot = boot(&context.lib).merge_attrs(nix_record! {
            "initrd": nix_record! {
                "systemd": nix_record! {
                    "enable": true,
                },
                "availableKernelModules": strings(&["virtio_blk", "virtio_pci", "virtio_scsi"]),
            },
            "kernelParams": strings(&["console=tty0", "console=hvc0"]),
        });
        let base = nix_record! {
            "airlock": nix_record! {
                "dom0": nix_record! {
                    "enable": true,
                    "user": "airlock",
                    "gui": nix_record! {
                        "enable": default(false.into()),
                    },
                    "dom0Resources": nix_record! {
                        "memoryMiB": 4096_i64,
                        "maxMemoryMiB": 4096_i64,
                    },
                },
            },
            "users": nix_record! {
                "users": nix_record! {
                    "airlock": nix_record! {
                        "isNormalUser": true,
                        "description": "Airlock nested-VM test operator",
                        "initialPassword": "airlock",
                        "extraGroups": strings(&["wheel", "qubes"]),
                    },
                },
            },
            // Disposable nested hosts need unattended sudo and loopback-forwarded SSH for lifecycle checks.
            "security": nix_record! {
                "sudo": nix_record! {
                    "wheelNeedsPassword": false,
                },
            },
            "services": nix_record! {
                "openssh": nix_record! {
                    "enable": true,
                    "settings": nix_record! {
                        "PasswordAuthentication": true,
                        "PermitRootLogin": "no",
                    },
                },
            },
            "environment": nix_record! {
                "systemPackages": NixValue::list([
                    context.pkgs.clone().select("curl"),
                    context.pkgs.clone().select("jq"),
                ]),
            },
            "boot": boot,
            "virtualisation": nix_record! {
                "xen": nix_record! {
                    "boot": nix_record! {
                        "params": strings(&["console=com1,vga", "com1=115200,8n1"]),
                    },
                },
                "diskSize": 131072_i64,
            },
            "image": nix_record! {
                "baseName": default("airlock-dom0-vm".into()),
                "format": "qcow2",
                "efiSupport": true,
            },
            "networking": nix_record! {
                "hostName": default("dom0".into()),
            },
            "system": nix_record! {
                "stateVersion": "26.05",
            },
        };
        let mut definitions = vec![base];
        if gui {
            definitions.push(nix_record! {
                "airlock": nix_record! {
                    "dom0": nix_record! {
                        "gui": nix_record! {
                            "enable": true,
                        },
                    },
                },
                "services": nix_record! {
                    "xserver": nix_record! {
                        "enable": true,
                        "displayManager": nix_record! {
                            "lightdm": nix_record! {
                                "enable": true,
                            },
                        },
                    },
                    "displayManager": nix_record! {
                        "defaultSession": "sys-gui",
                        "autoLogin": nix_record! {
                            "enable": true,
                            "user": "airlock",
                        },
                    },
                    "libinput": nix_record! {
                        "enable": true,
                    },
                },
                "environment": nix_record! {
                    "systemPackages": NixValue::list([context.pkgs.clone().select("xdotool")]),
                },
                "image": nix_record! {
                    "baseName": "airlock-dom0-vm-gui",
                },
            });
        }
        let modules: NixPath = context.argument("modulesPath");

        ModuleBody::new(
            context
                .lib
                .select("mkMerge")
                .call(NixValue::list(definitions)),
        )
        .key("airlock.host.nested")
        .import_value(
            modules
                .join_relative("virtualisation/disk-image.nix")
                .unwrap()
                .as_expression(),
        )
    })
    .into_expression()
}

/// Keep EFI and initrd settings identical between the nested module contract and disk image.
fn boot(lib: &NixValue) -> NixValue {
    nix_record! {
        "loader": nix_record! {
            "systemd-boot": nix_record! {
                "enable": true,
            },
            "efi": nix_record! {
                "canTouchEfiVariables": lib.clone().select("mkDefault").call(false),
            },
        },
        "initrd": nix_record! {
            "systemd": nix_record! {
                "enable": true,
            },
        },
    }
}

/// Configure the controller workflow after the assembly has materialized its VM descriptors.
fn config_system(source: &NixPath, workspace: &Workspace) -> NixValue {
    ContextualModule::with_arguments(["airlockConfigDomains", "airlockQubeKernel"], |context| {
        let domains: NixValue = context.argument("airlockConfigDomains");
        let kernel: NixValue = context.argument("airlockQubeKernel");
        let default = |value: NixValue| context.lib.clone().select("mkDefault").call(value);

        ModuleBody::new(nix_record! {
            "assertions": NixValue::list([nix_record! {
                "assertion": domains.clone().has_attr("sys-ctrl"),
                "message": "Controller bootstrap requires a sys-ctrl VM.",
            }]),
            "airlock": nix_record! {
                "machineDeployment": nix_record! {
                    "enable": true,
                    "profile": "controller-lab",
                },
                "poolAdmission": nix_record! {
                    "enable": default(true.into()),
                    "caller": default("sys-ctrl".into()),
                    "targets": default(NixValue::builtin("attrNames").call(domains.clone())),
                },
                "qubes": nix_record! {
                    "enable": true,
                    "setupConfigOnBoot": nix_record! {
                        "enable": true,
                    },
                    "startNetworkProvidersOnBoot": nix_record! {
                        "enable": true,
                    },
                    "deferredSetupConfigOnBoot": nix_record! {
                        "enable": true,
                    },
                    "configCurrentBootstrapOnBoot": nix_record! {
                        "enable": false,
                    },
                    "configAccessPolicy": nix_record! {
                        "enable": false,
                    },
                    "prefs": nix_record! {
                        "default_kernel": "",
                        "default_guivm": "sys-gui",
                        "default_netvm": "sys-firewall",
                    },
                    "templates": nix_record! {},
                    "kernels": kernels(kernel),
                    "domains": domains,
                    "qrexecPolicy": nix_record! {
                        "rules": NixValue::list([]),
                    },
                },
            },
            "virtualisation": nix_record! {
                "diskSize": default(131072_i64.into()),
            },
        })
        .key("airlock.host.controller-bootstrap")
        .import_value(super::deployment::module(source, workspace))
    })
    .into_expression()
}

/// Configure a display-only topology without choosing host hardware or image settings.
pub(crate) fn gui_minimal() -> NixValue {
    ContextualModule::with_arguments(["airlockGuiDomain", "airlockQubeKernel"], |context| {
        ModuleBody::new(nix_record! {
            "airlock": nix_record! {
                "qubes": nix_record! {
                    "enable": true,
                    "setupConfigOnBoot": nix_record! {
                        "enable": true,
                        "scope": "all",
                    },
                    "startNetworkProvidersOnBoot": nix_record! {
                        "enable": false,
                    },
                    "deferredSetupConfigOnBoot": nix_record! {
                        "enable": false,
                    },
                    "configCurrentBootstrapOnBoot": nix_record! {
                        "enable": false,
                    },
                    "configAccessPolicy": nix_record! {
                        "enable": false,
                    },
                    "prefs": nix_record! {
                        "default_kernel": "",
                        "default_guivm": "sys-gui",
                        "default_netvm": NixValue::null(),
                    },
                    "kernels": kernels(context.argument("airlockQubeKernel")),
                    "domains": nix_record! {
                        "sys-gui": context.argument::<NixValue>("airlockGuiDomain"),
                    },
                },
            },
        })
        .key("airlock.host.gui-minimal")
    })
    .into_expression()
}

/// Index a materialized upstream kernel by the identity stored in Qubes VM descriptors.
fn kernels(kernel: NixValue) -> NixValue {
    NixValue::builtin("listToAttrs").call(NixValue::list([nix_record! {
        "name": kernel.clone().select("name"),
        "value": nix_record! {
            "source": kernel.select("package").to_text(),
        },
    }]))
}

/// Collect fixed systemd, boot and user group identifiers.
fn strings(values: &[&str]) -> NixValue {
    NixValue::list(values.iter().map(|value| (*value).into()))
}
