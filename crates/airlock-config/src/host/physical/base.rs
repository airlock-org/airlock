//! Configure the physical host's accounts, USB filesystems, boot loader and diagnostics.

use super::{ModuleBody, ModuleContext, NixValue};
use rusix::nix_record;

/// Supply the console/bootstrap carrier shared by the normal and graphics boots.
pub(super) fn module(inputs: &ModuleContext) -> ModuleBody {
    let force = |value: NixValue| inputs.lib.clone().select("mkForce").call(value);
    let packages = [
        "curl",
        "jq",
        "gnumake",
        "python3",
        "cryptsetup",
        "rsync",
        "xdotool",
        "e2fsprogs",
        "openssl",
    ]
    .map(|name| inputs.pkgs.clone().select(name));
    let configuration = nix_record! {
        "airlock": nix_record! {
            "dom0": nix_record! {
                "enable": true,
                "user": "user",
                "gui": nix_record! {
                    "enable": true,
                },
                "bootMenu": nix_record! {
                    "enable": true,
                },
                "devModeNetworkProxy": nix_record! {
                    "enable": false,
                },
                "dom0Resources": nix_record! {
                    "memoryMiB": 8192,
                    "maxMemoryMiB": 8192,
                },
            },
            "devDom0Terminal": nix_record! {
                "host": nix_record! {
                    "enable": true,
                },
                "user": "user",
            },
        },
        "users": nix_record! {
            "mutableUsers": false,
            "users": nix_record! {
                "user": nix_record! {
                    "isNormalUser": true,
                    "uid": 1000,
                    "extraGroups": NixValue::list(["wheel", "qubes", "networkmanager"].map(Into::into)),
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
        "networking": nix_record! {
            "useDHCP": false,
            "networkmanager": nix_record! {
                "enable": true,
            },
            "firewall": nix_record! {
                "interfaces": nix_record! {
                    "tailscale0": nix_record! {
                        "allowedTCPPorts": NixValue::list([22.into()]),
                    },
                },
            },
        },
        "services": nix_record! {
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
                    "AuthorizedKeysFile": force(".ssh/authorized_keys".into()),
                },
            },
            "udisks2": nix_record! {
                "enable": force(false.into()),
            },
            "gvfs": nix_record! {
                "enable": force(false.into()),
            },
            "usbguard": nix_record! {
                "enable": force(false.into()),
            },
            "xserver": nix_record! {
                "enable": true,
                "displayManager": nix_record! {
                    "lightdm": nix_record! {
                        "enable": true,
                    },
                },
            },
            "libinput": nix_record! {
                "enable": true,
            },
            "journald": nix_record! {
                "settings": nix_record! {
                    "Journal": nix_record! {
                        "Storage": "persistent",
                        "SystemMaxUse": "1G",
                        "SyncIntervalSec": "1s",
                    },
                },
            },
        },
        "boot": nix_record! {
            "loader": nix_record! {
                "systemd-boot": nix_record! {
                    "enable": true,
                    "consoleMode": "1",
                    "configurationLimit": 5,
                },
                "timeout": 10,
                "efi": nix_record! {
                    "canTouchEfiVariables": false,
                },
            },
            "initrd": nix_record! {
                "systemd": nix_record! {
                    "enable": true,
                },
            },
            "kernelParams": NixValue::list(["console=tty0", "console=hvc0"].map(Into::into)),
            "growPartition": false,
        },
        "fileSystems": nix_record! {
            "/": nix_record! {
                "device": force("/dev/disk/by-label/airlock-usb".into()),
                "fsType": "ext4",
                "autoResize": force(false.into()),
            },
            "/boot": nix_record! {
                "device": force("/dev/disk/by-label/ESP".into()),
                "fsType": "vfat",
            },
        },
        "environment": nix_record! {
            "systemPackages": NixValue::list(packages),
        },
        "systemd": nix_record! {
            "tmpfiles": nix_record! {
                "rules": NixValue::list(
                    [
                        "d /var/lib/airlock/usb 0700 root root - -",
                        "d /home/user/dev 0755 user users - -",
                        "d /home/user/.airlock-logs 0700 user users - -",
                    ]
                    .map(Into::into),
                ),
            },
            "services": nix_record! {
                "qubes-vm@": nix_record! {
                    "enable": force(false.into()),
                },
            },
        },
        "system": nix_record! {
            "stateVersion": "26.05",
        },
    };

    ModuleBody::new(configuration)
        .import_value(crate::host::nested::gui_minimal())
        .import_value(nix_record! {
            "airlock": nix_record! {
                "image": nix_record! {
                    "devMode": nix_record! {
                        "enable": true,
                    },
                },
            },
        })
}
