//! Provide a physical dom0 console image for diagnosing Xen and generic USB boot.

use rusix::{
    interop::{
        NixPath,
        raw::{NixRepresentation, NixValue},
    },
    nix_record,
    nixos::{ContextualModule, ModuleBody},
};

/// Boot the Qubes control stack without starting VMs or claiming the physical display for a GUI VM.
pub(crate) fn module() -> NixValue {
    ContextualModule::with_arguments(["modulesPath"], |context| {
        let force = |value: NixValue| context.lib.clone().select("mkForce").call(value);
        let image = super::usb::disk_builder(&context)
            .call(super::usb::disk_arguments(&context, "airlock-console"));
        let settings = nix_record! {
            "airlock": nix_record! {
                "dom0": nix_record! {
                    "enable": true,
                    "user": "user",
                    "gui": nix_record! {
                        "enable": false,
                    },
                    "devModeNetworkProxy": nix_record! {
                        "enable": false,
                    },
                    "dom0Resources": nix_record! {
                        "memoryMiB": 4096_i64,
                        "maxMemoryMiB": 4096_i64,
                    },
                },
                "qubes": nix_record! {
                    "setupConfigOnBoot": nix_record! {
                        "enable": false,
                    },
                },
            },
            "systemd": nix_record! {
                "defaultUnit": "multi-user.target",
                "services": nix_record! {
                    "qubes-vm@": nix_record! {
                        "enable": force(false.into()),
                    },
                },
            },
            "services": nix_record! {
                // This diagnostic image must retain access to its own USB root disk.
                "usbguard": nix_record! {
                    "enable": force(false.into()),
                },
                "tailscale": nix_record! {
                    "enable": true,
                },
                "openssh": nix_record! {
                    "enable": true,
                    "openFirewall": false,
                    "settings": nix_record! {
                        "PasswordAuthentication": false,
                        "KbdInteractiveAuthentication": false,
                        "PermitRootLogin": "no",
                    },
                },
                "journald": nix_record! {
                    "settings": nix_record! {
                        "Journal": nix_record! {
                            "Storage": "persistent",
                            "SystemMaxUse": "256M",
                            "SyncIntervalSec": "1s",
                        },
                    },
                },
            },
            "users": nix_record! {
                "users": nix_record! {
                    "user": nix_record! {
                        "isNormalUser": true,
                        "uid": 1000_i64,
                        "extraGroups": strings(&["wheel", "qubes", "networkmanager"]),
                        "initialPassword": "password",
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
            "networking": nix_record! {
                "useDHCP": false,
                "networkmanager": nix_record! {
                    "enable": true,
                },
                "firewall": nix_record! {
                    "interfaces": nix_record! {
                        "tailscale0": nix_record! {
                            "allowedTCPPorts": NixValue::list([22_i64.into()]),
                        },
                    },
                },
            },
            "hardware": nix_record! {
                "enableRedistributableFirmware": true,
            },
            "boot": nix_record! {
                "initrd": nix_record! {
                    "availableKernelModules": strings(&[
                        "xhci_pci",
                        "ehci_pci",
                        "usb_storage",
                        "uas",
                        "sd_mod",
                        "virtio_pci",
                        "virtio_blk",
                        "virtio_scsi",
                    ]),
                    "systemd": nix_record! {
                        "enable": true,
                    },
                },
                "kernelParams": strings(&["console=hvc0", "console=tty0"]),
                "growPartition": false,
                "loader": nix_record! {
                    "systemd-boot": nix_record! {
                        "enable": true,
                    },
                    "timeout": 10_i64,
                    "efi": nix_record! {
                        "canTouchEfiVariables": false,
                    },
                },
            },
            "virtualisation": nix_record! {
                "xen": nix_record! {
                    "boot": nix_record! {
                        "params": strings(&["console=vga,com1", "com1=115200,8n1"]),
                    },
                },
                "diskSize": 12288_i64,
            },
            "fileSystems": nix_record! {
                "/": nix_record! {
                    "device": force("/dev/disk/by-label/airlock-console".into()),
                    "autoResize": force(false.into()),
                },
                "/boot": nix_record! {
                    "device": force("/dev/disk/by-label/ESP".into()),
                },
            },
            "image": nix_record! {
                "baseName": "airlock-usb-dom0-console",
                "format": "raw",
                "efiSupport": true,
            },
            "system": nix_record! {
                "build": nix_record! {
                    "image": force(image),
                },
                "stateVersion": "26.05",
            },
            "environment": nix_record! {
                "systemPackages": NixValue::list([
                    context.pkgs.clone().select("pciutils"),
                    context.pkgs.clone().select("usbutils"),
                ]),
                "etc": nix_record! {
                    "airlock/usb-dom0-console": nix_record! {
                        "text": "airlock-usb-dom0-console-v1\n",
                    },
                },
                "loginShellInit": r#"
                    if [ -t 1 ]; then
                      echo 'Airlock dom0 console baseline. Local login: user / password'
                      echo 'Check Xen: cat /proc/xen/capabilities; sudo xl list'
                    fi
                "#,
            },
        };
        let modules: NixPath = context.argument("modulesPath");

        ModuleBody::new(settings)
            .key("airlock.image.console")
            .import_value(
                modules
                    .join_relative("virtualisation/disk-image.nix")
                    .unwrap()
                    .as_expression(),
            )
    })
    .into_expression()
}

/// Preserve the order of boot driver, console and user group declarations.
fn strings(values: &[&str]) -> NixValue {
    NixValue::list(values.iter().map(|value| (*value).into()))
}
