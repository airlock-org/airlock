//! Add host startup and configuration-transport controls to the shared topology schema.

use super::schema::Schema;
use rusix::{interop::raw::NixValue, nix_record};

/// Declare configuration authority separately from the VM and artifact schema.
pub(super) fn declarations(lib: &NixValue) -> NixValue {
    let schema = Schema { lib };
    let boolean = schema.kind("bool");
    let string = schema.kind("str");
    let mut startup = Vec::new();
    for (name, description) in [
        (
            "setupConfigOnBoot",
            "Materialize the selected Qubes topology during boot.",
        ),
        (
            "configCurrentBootstrapOnBoot",
            "Initialize staging from the vault's current baseline.",
        ),
        (
            "startNetworkProvidersOnBoot",
            "Start declared root network providers after setup.",
        ),
        (
            "deferredSetupConfigOnBoot",
            "Materialize remaining VMs after initial boot setup.",
        ),
    ] {
        let mut options = nix_record! {
            "enable": schema.option(boolean.clone(), Some(false.into()), description),
            "logDir": schema.option(
                string.clone(),
                Some("/var/log/airlock".into()),
                "JSON results and command logs for the boot operation.",
            ),
        };
        if name == "setupConfigOnBoot" {
            options = options.merge_attrs(nix_record! {
                "scope": schema.option(
                    schema.enumeration(&["boot-critical", "all"]),
                    Some("boot-critical".into()),
                    "Materialize boot-critical VMs first or the complete topology.",
                ),
                "statusDevice": schema.option(
                    schema.nullable(string.clone()),
                    Some(NixValue::null()),
                    "Optional console device for concise bootstrap progress.",
                ),
            });
        }
        startup.push((name, options));
    }

    let mut access = vec![
        (
            "enable",
            schema.option(
                boolean,
                Some(false.into()),
                "Install dom0-mediated configuration transport policy.",
            ),
        ),
        (
            "allowedSubmitters",
            schema.option(
                schema.list(string.clone()),
                Some(NixValue::list([])),
                "VMs allowed to request status, validation, application and rollback through dom0.",
            ),
        ),
    ];
    for (name, default, purpose) in [
        (
            "currentExportService",
            "airlock.Config.ExportCurrent",
            "Fetch the current vault baseline.",
        ),
        (
            "currentImportService",
            "airlock.Config.ImportCurrent",
            "Import the baseline into staging.",
        ),
        (
            "candidateExportService",
            "airlock.Config.CandidateExport",
            "Fetch a staged candidate.",
        ),
        (
            "candidateValidateService",
            "airlock.Config.ValidateCandidate",
            "Validate a candidate in the validator VM.",
        ),
        (
            "archiveService",
            "airlock.Config.ArchiveCandidate",
            "Archive a candidate in the vault.",
        ),
        (
            "archiveDeploymentService",
            "airlock.Config.ArchiveDeployment",
            "Archive deployment records.",
        ),
        (
            "configInputsImportService",
            "airlock.Config.InputsImport",
            "Import admitted offline build inputs.",
        ),
        (
            "statusService",
            "airlock.Config.Status",
            "Report configuration status through dom0.",
        ),
        (
            "checkService",
            "airlock.Config.Check",
            "Request validation without activation.",
        ),
        (
            "applyService",
            "airlock.Config.Apply",
            "Request validation, archival and activation.",
        ),
        (
            "rollbackService",
            "airlock.Config.Rollback",
            "Request a previously validated generation.",
        ),
        (
            "vault",
            "sys-config-vault",
            "VM retaining archived configuration history.",
        ),
        (
            "staging",
            "sys-config-staging",
            "VM owning the editable candidate checkout.",
        ),
        (
            "validator",
            "sys-config-validator",
            "VM validating candidates independently.",
        ),
    ] {
        access.push((
            name,
            schema.option(string.clone(), Some(default.into()), purpose),
        ));
    }
    let fields = schema
        .fields()
        .merge_attrs(NixValue::record(startup))
        .merge_attrs(nix_record! {
            "enable": lib
                .clone()
                .select("mkEnableOption")
                .call("declarative Qubes topology setup"),
            "topology": lib.clone().select("mkOption").call(nix_record! {
                "type": schema.kind("attrs"),
                "readOnly": true,
                "description": "Normalized Qubes topology consumed by runtime services.",
            }),
            "configAccessPolicy": NixValue::record(access),
        });

    nix_record! {
        "options": nix_record! {
            "airlock": nix_record! {
                "qubes": fields,
            },
        },
    }
}
