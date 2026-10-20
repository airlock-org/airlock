//! Expose the development loopback proxy through the selected Qubes updates-proxy service.

use super::{Host, assertion, strings};
use rusix::{interop::raw::NixValue, nix_record, nix_text};

/// Keep host development traffic on qrexec and avoid installing a default route through DHCP.
pub(super) fn configuration(host: &Host<'_>) -> NixValue {
    let settings = host.setting("devModeNetworkProxy");
    let enabled = settings.clone().select("enable");
    let target = settings.clone().select("target");
    let listen: NixValue = nix_text!(
        "{address}:{port}",
        address = settings.clone().select("listenAddress"),
        port = settings.select("port"),
    )
    .into();
    let url: NixValue = nix_text!("http://{listen}", listen = listen.clone()).into();
    let mut environment = ["http_proxy", "https_proxy", "HTTP_PROXY", "HTTPS_PROXY"]
        .map(|name| (name, url.clone()))
        .to_vec();
    environment.extend([
        ("no_proxy", "127.0.0.1,localhost,::1".into()),
        ("NO_PROXY", "127.0.0.1,localhost,::1".into()),
    ]);
    let environment = NixValue::record(environment);
    let policy = host
        .package("writeTextDir")
        .call("etc/qubes/policy.d/35-airlock-dom0-network-proxy.policy")
        .call(nix_text!(
            r#"
            qubes.UpdatesProxy * dom0 {target} allow
        "#,
            target = target.clone(),
        ));
    let args = host
        .context
        .lib
        .clone()
        .select("escapeShellArgs")
        .call(NixValue::list([
            "--use-stdin-socket".into(),
            "-d".into(),
            target.clone(),
            "DEFAULT:QUBESRPC qubes.UpdatesProxy+ dom0".into(),
        ]));
    let readiness = strings(&[
        "airlock-start-network-providers.service",
        "qubes-qrexec-policy-daemon.service",
        "qubesd.service",
    ]);
    let proxy = nix_record! {
        "assertions": NixValue::list([
            assertion(
                target.clone().equals("").equals(false),
                "The development proxy requires a target VM.",
            ),
            assertion(
                NixValue::builtin("match")
                    .call("[A-Za-z0-9_.-]+")
                    .call(target)
                    .equals(NixValue::null())
                    .equals(false),
                "The development proxy target must be a simple Qubes VM name.",
            ),
        ]),
        "networking": nix_record! {
            "dhcpcd": nix_record! {
                "extraConfig": host.context.lib.clone().select("mkAfter").call(nix_text!(r#"
                nogateway
            "#)),
            },
        },
        "virtualisation": nix_record! {
            "qubes": nix_record! {
                "packages": NixValue::list([policy]),
            },
        },
        "environment": nix_record! {
            "sessionVariables": environment.clone(),
        },
        "systemd": nix_record! {
            "sockets": nix_record! {
                "airlock-dom0-updates-proxy": nix_record! {
                    "description": "Listen for development dom0 traffic to the updates proxy",
                    "wantedBy": strings(&["sockets.target"]),
                    "listenStreams": NixValue::list([listen]),
                    "socketConfig": nix_record! {
                        "Accept": true,
                        "BindToDevice": "lo",
                    },
                },
            },
            "services": nix_record! {
                "nix-daemon": nix_record! {
                    "environment": environment,
                },
                "airlock-dom0-updates-proxy@": nix_record! {
                    "description": "Forward a development dom0 proxy connection to the network VM",
                    "wants": readiness.clone(),
                    "after": readiness,
                    "serviceConfig": nix_record! {
                        "ExecStart": nix_text!(
                            "{qrexec}/bin/qrexec-client {args}",
                            qrexec = host.package("python3.pkgs.qubes-core-qrexec.dom0"),
                            args = args,
                        ),
                        "StandardInput": "socket",
                        "StandardOutput": "inherit",
                        "StandardError": "journal",
                    },
                },
            },
        },
    };

    host.context
        .lib
        .clone()
        .select("mkMerge")
        .call(NixValue::list([
            nix_record! {
                "networking": nix_record! {
                    "useDHCP": host.default(
                        host.context
                            .config
                            .clone()
                            .select("airlock.image.devMode.enable"),
                    ),
                },
            },
            host.when(enabled, proxy),
        ]))
}
