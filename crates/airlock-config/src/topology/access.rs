//! Limit configuration transport to explicit vault, staging, validation and submitter identities.

use rusix::{interop::raw::NixValue, nix_record, nix_text, nixos::ModuleContext};

/// Install the configuration controller's exact RPC handlers and bootstrap-owned policy.
pub(super) fn configuration(
    context: &ModuleContext,
    access: &NixValue,
    tools: &NixValue,
) -> NixValue {
    let controller = tools.clone().select("airlock-config-control");
    let mut handlers = Vec::new();
    let mut rules = Vec::new();
    for (service, targets) in [
        ("currentExportService", &["vault"][..]),
        ("currentImportService", &["staging"][..]),
        ("candidateExportService", &["staging"][..]),
        ("candidateValidateService", &["validator"][..]),
        ("configInputsImportService", &["staging", "validator"][..]),
        ("archiveService", &["vault"][..]),
        ("archiveDeploymentService", &["vault"][..]),
    ] {
        let service = access.clone().select(service);
        for target in targets {
            rules.push(
                nix_text!(
                    r#"
                {service} * dom0 {target} allow
            "#,
                    service = service.clone(),
                    target = access.clone().select(target),
                )
                .into(),
            );
        }
        rules.push(deny(service));
    }
    let submitters = NixValue::builtin("map")
        .call(NixValue::function(|submitter| {
            NixValue::concat_text(
                [
                    "statusService",
                    "checkService",
                    "applyService",
                    "rollbackService",
                ]
                .map(|service| {
                    nix_text!(
                        r#"
                {service} * {submitter} dom0 allow
            "#,
                        service = access.clone().select(service),
                        submitter = submitter.clone(),
                    )
                    .into()
                }),
            )
        }))
        .call(access.clone().select("allowedSubmitters"));
    rules.push(
        NixValue::builtin("concatStringsSep")
            .call("")
            .call(submitters),
    );
    for (setting, operation) in [
        ("statusService", "status"),
        ("checkService", "check"),
        ("applyService", "apply"),
        ("rollbackService", "rollback"),
    ] {
        let service = access.clone().select(setting);
        let handler = context
            .pkgs
            .clone()
            .select("writeShellScript")
            .call(format!("airlock-config-control-{operation}-qrexec"))
            .call(nix_text!(
                r#"
                exec {controller}/bin/airlock-config-control {operation}
            "#,
                controller = controller.clone(),
                operation = operation,
            ));
        handlers.push(nix_record! {
            "name": nix_text!("qubes-rpc/{service}", service = service.clone()),
            "value": nix_record! {
                "source": handler,
                "mode": "0555",
            },
        });
        rules.push(deny(service));
    }
    let policy = context
        .pkgs
        .clone()
        .select("writeTextDir")
        .call("etc/qubes/policy.d/30-airlock-config.policy")
        .call(NixValue::concat_text(rules));

    nix_record! {
        "environment": nix_record! {
            "etc": NixValue::builtin("listToAttrs").call(NixValue::list(handlers)),
        },
        "virtualisation": nix_record! {
            "qubes": nix_record! {
                "packages": NixValue::list([policy]),
            },
        },
    }
}

/// Finish each bootstrap service's explicit permissions with a catch-all denial.
fn deny(service: NixValue) -> NixValue {
    nix_text!(
        r#"
        {service} * @anyvm @anyvm deny
    "#,
        service = service,
    )
    .into()
}
