//! Connect a GUI VM's completed screen captures to a personal VM.
//! One validated relationship supplies both runtime contracts and exact Qubes permissions.

use super::machine_source;
use crate::model::{Gui, Personal, VmHandle, VmSnapshot};
use airlock_machine_core::services::{
    CaptureConnection, CaptureKind, CaptureOperation, CaptureReceiverConfig, CaptureSenderConfig,
};
use rusix::{
    installation,
    interop::raw::{NixRepresentation, NixValue},
    nix_record, nix_text,
    nixos::{ModuleBody, ModuleContext, NixosModule},
};

/// Installed receiver contract checked against Qubes' authenticated caller identity.
const RECEIVER_CONFIG_PATH: &str = "/etc/airlock/capture-receiver.json";

/// Connects completed captures from a GNOME GUI VM to a personal VM.
/// Finalization checks both declarations, installs both endpoints and derives Qubes
/// policy. Reconnecting the same source replaces its destination for future snapshots.
/// No files, services or permissions are changed until the generated system is deployed.
pub fn connect(source: &VmHandle<Gui>, target: &VmHandle<Personal>) {
    source.capture_to(target);
}

/// Installs endpoint contributions after every connection has passed membership checks.
/// Receivers shared by several GUI VMs get one contract listing exactly those sources.
pub(crate) fn configure(vms: &mut [VmSnapshot], connections: &[CaptureConnection]) {
    for vm in vms {
        if let Some(connection) = connections
            .iter()
            .find(|connection| connection.source == vm.name)
        {
            vm.modules.push(sender(
                vm.name.to_string(),
                CaptureSenderConfig {
                    target: connection.target.clone(),
                },
            ));
        }

        let sources = connections
            .iter()
            .filter(|connection| connection.target == vm.name)
            .map(|connection| connection.source.clone())
            .collect::<Vec<_>>();
        if !sources.is_empty() {
            vm.modules.push(receiver(CaptureReceiverConfig { sources }));
        }
    }
}

/// Emits exact allow rules followed by denials for every other capture request.
/// Install before the GUI policy's broad denial; rules authorize only the supplied
/// connections, including when that list is empty. Qubes remains the policy authority.
pub fn policy(connections: &[CaptureConnection]) -> String {
    let mut text = String::new();
    for connection in connections {
        for operation in CaptureOperation::ALL {
            for kind in CaptureKind::ALL {
                text.push_str(&format!(
                    "{service} +{kind} {source} {target} allow\n",
                    service = operation.service(),
                    kind = kind.as_str(),
                    source = connection.source,
                    target = connection.target
                ));
            }
        }
    }

    for operation in CaptureOperation::ALL {
        text.push_str(&format!(
            "{service} * @anyvm @anyvm deny\n",
            service = operation.service(),
        ));
    }

    text
}

/// Installs the sender contract, package and GNOME completion adapter.
fn sender(scope: String, config: CaptureSenderConfig) -> NixosModule {
    NixosModule::with_arguments([machine_source::ARGUMENT], |inputs| {
        let service = airlock_sys_gui_service::authoring::deployment(
            &inputs.pkgs,
            &machine_source::service_sources(&inputs),
        );
        let expected = serde_json::to_string(&config).expect("capture identities serialize");

        let contract = ModuleBody::new(nix_record! {
            "assertions": NixValue::list([
                nix_record! {
                    "assertion": inputs
                        .config
                        .clone()
                        .select("services.desktopManager.gnome.enable"),
                    "message": "The screen-capture connection requires the GNOME completion adapter.",
                },
                contract_assertion(&inputs, "airlock/capture-sender.json", expected),
            ]),
        });
        let behavior = installation::compose(
            &scope,
            installation::CollisionChecks::Enabled,
            [service.capture_sender(&config)],
        )
        .unwrap_or_else(|error| panic!("{error}"));

        ModuleBody::empty().module(behavior).module(contract)
    })
}

/// Rejects downstream edits that would detach a service contract from the authored policy.
fn contract_assertion(inputs: &ModuleContext, file: &str, expected: String) -> NixValue {
    nix_record! {
        "assertion": inputs
            .config
            .clone()
            .select("environment.etc")
            .select_segments([file])
            .select("text")
            .equals(expected),
        "message": format!(
            "{file} is derived from the typed screen-capture connection; \
             change the connection in Rust."
        ),
    }
}

/// Installs the receiver contract and the two narrow Qubes service entry points.
fn receiver(config: CaptureReceiverConfig) -> NixosModule {
    NixosModule::with_arguments([machine_source::ARGUMENT], |inputs| {
        let workspace = airlock_build::Workspace::new(rusix::interop::NixPath::from_expression(
            inputs.pkgs.clone().select("airlockAppsSource"),
        ));
        let package = airlock_build::Application::VmService
            .build(&inputs.pkgs, &workspace)
            .as_expression();

        let json = serde_json::to_string(&config).expect("capture identities serialize");
        let user = nix_text!(
            r#"
                force-user='{user}'
            "#,
            user = inputs.config.clone().select("airlock.qubeGuest.user"),
        );

        let mut files = vec![(
            "airlock/capture-receiver.json".to_owned(),
            nix_record! {
                "text": json.clone(),
            },
        )];
        for operation in CaptureOperation::ALL {
            files.push((
                format!("qubes-rpc/{service}", service = operation.service()),
                nix_record! {
                    "source": receiver_script(&inputs, &package, operation),
                    "mode": "0555",
                },
            ));
            files.push((
                format!("qubes/rpc-config/{service}", service = operation.service()),
                nix_record! {
                    "text": user.clone(),
                },
            ));
        }

        ModuleBody::new(nix_record! {
            "assertions": NixValue::list([contract_assertion(
                &inputs,
                "airlock/capture-receiver.json",
                json,
            )]),
            "environment": nix_record! {
                "systemPackages": NixValue::list([package]),
                "etc": NixValue::record(files),
            },
        })
    })
}

/// Dispatches a Qubes request to Rust, which validates the media argument and configured caller.
fn receiver_script(
    inputs: &ModuleContext,
    vm_service: &NixValue,
    operation: CaptureOperation,
) -> NixValue {
    let executable = NixValue::concat_text([
        vm_service.clone().to_text(),
        "/bin/airlock-vm-service".into(),
    ]);
    let script = nix_text!(
        r#"
            set -eu
            exec {executable} {command} "${{QREXEC_SERVICE_ARGUMENT:-}}" --config {config}
        "#,
        executable = executable,
        command = operation.command(),
        config = RECEIVER_CONFIG_PATH,
    );

    inputs
        .pkgs
        .clone()
        .select("writeShellScript")
        .call(format!("airlock-{command}", command = operation.command()))
        .call(script)
}
