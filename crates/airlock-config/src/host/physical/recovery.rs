//! Configure an independent recovery system that does not mount the normal machine's pool at boot.

use super::{ModuleContext, NixValue, PhysicalBuild};
use rusix::{nix_record, nix_text};

/// Use the parent kernel and Xen with a separate userspace, read-only store and no Nix daemon.
/// Packages come from the recovery system's own module arguments, not the parent's package set.
pub(super) fn module(inputs: &ModuleContext, build: &PhysicalBuild, source: NixValue) -> NixValue {
    let diagnostics = super::super::diagnostics::module(&build.workspace);
    let kernel = inputs.config.clone().select("boot.kernelPackages");
    let xen = inputs.config.clone().select("virtualisation.xen.package");

    NixValue::function_attrs_open(["pkgs"], |arguments| {
        let pkgs = arguments.clone().select("pkgs");
        let storage = airlock_build::Application::Dom0.build(&pkgs, &build.workspace);

        let mut packages = vec![rusix::interop::raw::NixRepresentation::as_expression(
            &storage,
        )];
        packages.extend(["e2fsprogs", "util-linux", "lvm2"].map(|name| pkgs.clone().select(name)));

        let recover = "sudo airlock-dom0-service storage host recover SYSTEM";

        (
            Vec::<(&str, NixValue)>::new(),
            nix_record! {
                "imports": NixValue::list([diagnostics]),
                "virtualisation": nix_record! {
                    "xen": nix_record! {
                        "enable": true,
                        "package": xen,
                    },
                },
                "boot": nix_record! {
                    "kernelPackages": kernel,
                    "initrd": nix_record! {
                        "systemd": nix_record! {
                            "enable": true,
                        },
                        "availableKernelModules": NixValue::list(
                            ["nvme", "xhci_pci", "usb_storage", "uas", "sd_mod", "ahci"]
                                .map(Into::into),
                        ),
                    },
                    "loader": nix_record! {
                        "systemd-boot": nix_record! {
                            "enable": true,
                        },
                        "efi": nix_record! {
                            "canTouchEfiVariables": false,
                        },
                    },
                    "kernelParams": NixValue::list(["console=tty0", "console=hvc0"].map(Into::into)),
                },
                "fileSystems": nix_record! {
                    "/": nix_record! {
                        "device": "/dev/disk/by-label/airlock-usb",
                        "fsType": "ext4",
                    },
                    "/boot": nix_record! {
                        "device": "/dev/disk/by-label/ESP",
                        "fsType": "vfat",
                    },
                    "/nix/store": nix_record! {
                        "device": "/var/lib/airlock/recovery-store",
                        "fsType": "none",
                        "options": NixValue::list(["bind", "ro"].map(Into::into)),
                        "neededForBoot": true,
                    },
                },
                "environment": nix_record! {
                    "systemPackages": NixValue::list(packages),
                    "etc": nix_record! {
                        "airlock/boot-role": nix_record! {
                            "text": "recovery\n",
                        },
                        "airlock/boot-source.json": nix_record! {
                            "text": NixValue::builtin("toJSON").call(source),
                        },
                        "airlock/pool-recovery": nix_record! {
                            "text": nix_text!(
                                r#"
                        Independent pool recovery. Networking and VMs are disabled.
                        Select a retained boot entry with: {recover}
                    "#,
                                recover = recover,
                            ),
                        },
                    },
                },
                "networking": nix_record! {
                    "useDHCP": false,
                    "wireless": nix_record! {
                        "enable": false,
                    },
                },
                "users": nix_record! {
                    "mutableUsers": false,
                    "users": nix_record! {
                        "user": nix_record! {
                            "isNormalUser": true,
                            "uid": 1000,
                            "extraGroups": NixValue::list(["wheel".into()]),
                            "hashedPasswordFile": "/var/lib/airlock/usb/local-password.hash",
                        },
                        "root": nix_record! {
                            "hashedPassword": "!",
                        },
                    },
                },
                "security": nix_record! {
                    "sudo": nix_record! {
                        "wheelNeedsPassword": true,
                    },
                },
                "nix": nix_record! {
                    "enable": false,
                },
                "system": nix_record! {
                    "stateVersion": "26.05",
                },
            },
        )
    })
}
