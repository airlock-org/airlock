//! Exercise the production uplink service graph with real udev events and harmless dummy interfaces.

use super::Build;
use airlock_build::Application;
use rusix::interop::raw::NixRepresentation;
use rusix::{interop::raw::NixValue, nix_record, nix_text};

/// Keep Qubes authority outside the test VM while preserving production service ordering and readiness.
pub(super) fn fixture(build: &Build) -> NixValue {
    let pkgs = NixValue::builtin("import")
        .call(build.inputs.clone().select("nixpkgs"))
        .call(nix_record! {
            "system": "x86_64-linux",
        });
    let probe = Application::UplinkTest
        .build(&pkgs, &build.workspace)
        .as_expression();
    let config = build.output("nixosConfigurations.airlock-domain-usr-assistant.config");
    let driver = build.output("nixosConfigurations.airlock-domain-sys-net.config");
    // Retain the production network driver's boot modules, excluding Xen-only
    // interfaces that this disposable KVM guest deliberately does not expose.
    let modules = pkgs
        .clone()
        .select("lib.filter")
        .call(NixValue::function(|name| {
            pkgs.clone()
                .select("lib.hasPrefix")
                .call("xen-")
                .call(name)
                .equals(false)
        }))
        .call(driver.clone().select("boot.kernelModules"));
    let uplink = config
        .clone()
        .select_segments(["systemd", "services", "qubes-network-uplink@"]);
    let rules = config.select("services.udev.extraRules");
    let replace = |from: &[&str], to: &[&str]| {
        NixValue::builtin("replaceStrings")
            .call(strings(from))
            .call(strings(to))
            .call(rules.clone())
    };
    let ordinary = replace(&[r#"DRIVERS=="vif""#], &[r#"KERNEL=="altest0""#]);
    let blocking = replace(
        &[r#"DRIVERS=="vif""#, "qubes-network-uplink@", "--no-block "],
        &[r#"KERNEL=="alblock0""#, "airlock-blocking-uplink@", ""],
    );
    let mut service = NixValue::record(
        ["after", "requires", "bindsTo", "before", "wants"]
            .map(|field| (field, uplink.clone().select(field))),
    );
    service = service.merge_attrs(nix_record! {
        "serviceConfig": uplink.select("serviceConfig").merge_attrs(nix_record! {
            "ExecStart": NixValue::list([
                nix_text!(
                    "{ip}/bin/ip address add 192.0.2.2/32 dev %i",
                    ip = pkgs.clone().select("iproute2"),
                )
                .into(),
                nix_text!(
                    "{ip}/bin/ip link set dev %i up",
                    ip = pkgs.clone().select("iproute2"),
                )
                .into(),
            ]),
            "ExecStop": nix_text!(
                "-{ip}/bin/ip address flush dev %i",
                ip = pkgs.clone().select("iproute2"),
            ),
        }),
    });
    let prerequisite = nix_record! {
        "serviceConfig": nix_record! {
            "Type": "oneshot",
            "RemainAfterExit": true,
            "ExecStart": nix_text!(
                "{coreutils}/bin/true",
                coreutils = pkgs.clone().select("coreutils"),
            ),
        },
    };

    pkgs.clone()
        .select("testers.runNixOSTest")
        .call(nix_record! {
            "name": "airlock-uplink-hotplug",
            "nodes": nix_record! {
                "machine": nix_record! {
                    "boot": nix_record! { "kernelModules": modules },
                    "networking": nix_record! {
                        "useDHCP": false,
                        "networkmanager": nix_record! {
                            "enable": driver.select("networking.networkmanager.enable"),
                        },
                    },
                    "services": nix_record! {
                        "udev": nix_record! {
                            "extraRules": NixValue::concat_text([ordinary, "\n".into(), blocking]),
                        },
                    },
                    "systemd": nix_record! {
                        "services": nix_record! {
                            "qubes-network-uplink@": service.clone(),
                            "airlock-blocking-uplink@": service.clone().merge_attrs(nix_record! {
                                "after": NixValue::concat_lists([
                                    service.select("after"),
                                    strings(&["sys-subsystem-net-devices-%i.device"]),
                                ]),
                                "bindsTo": strings(&["sys-subsystem-net-devices-%i.device"]),
                            }),
                            "qubes-db": prerequisite.clone(),
                            "qubes-sysinit": prerequisite,
                        },
                    },
                    "environment": nix_record! {
                        "systemPackages": NixValue::list([
                            pkgs.clone().select("iproute2"),
                            probe,
                        ]),
                        "etc": nix_record! {
                            "airlock-uplink-test": nix_record! { "text": "disposable systemd/udev fixture\n" },
                        },
                    },
                    "virtualisation": nix_record! {
                        "memorySize": 768_i64,
                    },
                },
            },
            "testScript": nix_text!(r#"
            from datetime import timedelta

            machine.start()
            machine.wait_for_unit("multi-user.target")
            try:
                machine.succeed(
                    "airlock-uplink-probe --inside-test-vm > /tmp/uplink-hotplug.json",
                    timeout=timedelta(seconds=90),
                )
            finally:
                machine.succeed("journalctl -b --no-pager > /tmp/uplink-journal.txt")
                machine.copy_from_machine("/tmp/uplink-journal.txt")
                machine.copy_from_machine("/tmp/uplink-hotplug.json")
        "#),
        })
}

/// Preserve exact udev match strings and systemd dependency identities.
fn strings(values: &[&str]) -> NixValue {
    NixValue::list(values.iter().map(|value| (*value).into()))
}
