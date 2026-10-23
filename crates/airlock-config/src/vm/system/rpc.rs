//! Install Qubes RPC endpoints with explicit target users and shared application discovery.

use super::System;
use airlock_build::Application;
use rusix::{
    interop::{
        NixExpression,
        raw::{NixRepresentation, NixValue},
    },
    nix_record, nix_text,
};

/// Install role-specific archive RPCs and the upstream application and shell protocol adapters.
pub(super) fn configuration(system: &System<'_>, programs: &NixValue) -> NixValue {
    application_environment(system).bind(|environment| {
        let mut files = Vec::new();
        for (role, rpc, program) in [
            (
                "sys-config-staging",
                "airlock.Config.CandidateExport",
                "configCandidateExportService",
            ),
            (
                "sys-config-staging",
                "airlock.Config.ImportCurrent",
                "configCurrentImportService",
            ),
            (
                "sys-config-validator",
                "airlock.Config.ValidateCandidate",
                "configCandidateValidateService",
            ),
            (
                "sys-config-vault",
                "airlock.Config.ArchiveCandidate",
                "configArchiveService",
            ),
            (
                "sys-config-vault",
                "airlock.Config.ArchiveDeployment",
                "configDeploymentArchiveService",
            ),
            (
                "sys-config-vault",
                "airlock.Config.ExportCurrent",
                "configCurrentExportService",
            ),
        ] {
            add_rpc(
                &mut files,
                system,
                rpc,
                programs.clone().select(program),
                Some(system.role(role)),
                Some(force_user(system.setting("user"))),
            );
        }
        files.push((
            "airlock/guest-tools/airlock-config-staging-validate".into(),
            system.when(
                system.role("sys-config-staging"),
                nix_record! {
                    "source": programs.clone().select("configStagingValidate"),
                },
            ),
        ));
        add_rpc(
            &mut files,
            system,
            "airlock.Config.InputsImport",
            programs.clone().select("configInputsImportService"),
            Some(system.imports_inputs()),
            Some(force_user("root".into())),
        );
        add_rpc(
            &mut files,
            system,
            "airlock.NixStoreRuntimeCheck",
            script(
                system,
                "airlock-nix-store-runtime-check-qrexec",
                crate::commands::contract(
                    &system.context.pkgs,
                    "store-check",
                    [
                        ("service", system.native()),
                        (
                            "path",
                            system.bin_path(system.packages(&["coreutils", "util-linux"])),
                        ),
                    ],
                ),
            ),
            None,
            None,
        );
        add_rpc(
            &mut files,
            system,
            "airlock.PolicyProbe",
            policy_probe(system),
            Some(system.roles(&["sys-ctrl", "usr-assistant"])),
            Some(force_user(system.setting("user"))),
        );

        let shell = script(
            system,
            "airlock-qubes-vmshell",
            nix_text!(
                r#"
            export PATH=/run/current-system/sw/bin:/run/wrappers/bin:/bin:/usr/bin
            exec {shell}
        "#,
                shell = system.package("runtimeShell"),
            )
            .into(),
        );
        add_rpc(
            &mut files,
            system,
            "qubes.VMShell",
            shell,
            None,
            Some(NixValue::if_else(
                system.setting("allowQrexecUserSelection"),
                "",
                force_user(system.setting("user")),
            )),
        );

        let service = Application::VmService
            .build(&system.context.pkgs, system.workspace)
            .as_expression();
        let catalog = script(
            system,
            "airlock-qubes-get-appmenus",
            nix_text!(
                r#"
            . {environment}
            export AIRLOCK_VM_PERSISTENCE="$(
              {database}/bin/qubesdb-read /qubes-vm-persistence 2>/dev/null || printf 'full\n'
            )"
            exec {service}/bin/airlock-vm-service catalog
        "#,
                environment = environment.clone(),
                database = system.package("qubes-core-qubesdb"),
                service = service.clone(),
            )
            .into(),
        );
        let launch = script(
            system,
            "airlock-qubes-start-app",
            nix_text!(
                r#"
            . {environment}
            export PYTHONPATH="{agent}/{sitePackages}${{PYTHONPATH:+:$PYTHONPATH}}"
            exec {python}/bin/python {source}/qubes-rpc/qubes.StartApp "$@"
        "#,
                environment = environment.clone(),
                agent = system.package("qubes-core-agent-linux"),
                sitePackages = system.package("python3.sitePackages"),
                python = system.python(),
                source = system.package("qubes-core-agent-linux.src"),
            )
            .into(),
        );
        for (name, command) in [
            ("qubes.StartApp", launch),
            ("qubes.GetAppmenus", catalog),
            ("qubes.GetImageRGBA", image(system, &environment, service)),
        ] {
            add_rpc(
                &mut files,
                system,
                name,
                command,
                None,
                Some(force_user(system.setting("user"))),
            );
        }

        nix_record! {
            "environment": nix_record! {
                "etc": NixValue::record(files),
            },
        }
    })
}

/// Link an executable and its package-local resources without changing caller-user restrictions.
fn add_rpc(
    files: &mut Vec<(String, NixValue)>,
    system: &System<'_>,
    name: &str,
    executable: NixValue,
    condition: Option<NixValue>,
    user: Option<NixValue>,
) {
    let enabled = |value| match &condition {
        Some(condition) => system.when(condition.clone(), value),
        None => value,
    };
    files.push((
        format!("qubes-rpc/{name}"),
        enabled(nix_record! {
            "source": executable,
            "mode": "symlink",
        }),
    ));
    if let Some(user) = user {
        files.push((
            format!("qubes/rpc-config/{name}"),
            enabled(nix_record! {
                "text": user,
                "mode": "0444",
            }),
        ));
    }
}

/// Select the unprivileged target account in Qubes' RPC configuration syntax.
fn force_user(user: NixValue) -> NixValue {
    nix_text!(
        r#"
        force-user='{user}'
    "#,
        user = user,
    )
    .into()
}

/// Build a single executable file suitable for an /etc/qubes-rpc symlink.
fn script(system: &System<'_>, name: &str, text: NixValue) -> NixValue {
    airlock_build::native::writer(&system.context.pkgs, false)
        .call(name)
        .call(text)
}

/// Use the active user and system profiles consistently for discovery, launching and icons.
fn application_environment(system: &System<'_>) -> NixValue {
    system
        .package("writeText")
        .call("airlock-application-environment")
        .call(nix_text!(
            r#"
        if [ -r /etc/profile ]; then
          . /etc/profile
        fi
        export XDG_DATA_HOME="${{XDG_DATA_HOME:-$HOME/.local/share}}"
        export XDG_DATA_DIRS="$HOME/.nix-profile/share:\
        /etc/profiles/per-user/{user}/share:\
        /run/current-system/sw/share:\
        ${{XDG_DATA_DIRS:-/usr/local/share:/usr/share}}"
    "#,
            user = system.setting("user"),
        ))
}

/// Preserve upstream image-transfer framing while using native icon selection and SVG rendering.
fn image(system: &System<'_>, environment: &NixValue, service: NixValue) -> NixValue {
    let icon = script(
        system,
        "airlock-xdg-icon",
        nix_text!(
            r#"
        . {environment}
        exec {service}/bin/airlock-vm-service icon "$@"
    "#,
            environment = environment.clone(),
            service = service,
        )
        .into(),
    );
    let upstream = system
        .package("runCommand")
        .call("airlock-qubes-get-image-upstream")
        .call(nix_record! {
            "nativeBuildInputs": system.packages(&["patch"]),
        })
        .call(nix_text!(
            r#"
            cp {source}/qubes-rpc/qubes.GetImageRGBA "$out"
            chmod u+w "$out"
            patch "$out" < {patch}
            substituteInPlace "$out" --replace-fail /usr/lib/qubes/xdg-icon {icon}
        "#,
            source = system.package("qubes-core-agent-linux.src"),
            icon = icon,
            patch = system
                .workspace
                .file("patches/qubes-core-agent-linux/0001-render-svg-icons-with-librsvg.patch")
                .snapshot(),
        ));

    script(
        system,
        "airlock-qubes-get-image",
        nix_text!(
            r#"
        . {environment}
        export PATH="{path}:$PATH"
        exec {bash}/bin/bash {upstream}
    "#,
            environment = environment.clone(),
            path = system.bin_path(system.packages(&["coreutils", "graphicsmagick", "librsvg"])),
            bash = system.package("bash"),
            upstream = upstream,
        )
        .into(),
    )
}

/// Report identity through QubesDB without granting additional execution or policy authority.
fn policy_probe(system: &System<'_>) -> NixValue {
    script(
        system,
        "airlock-policy-probe-qrexec",
        nix_text!(
            r#"
        set -eu

        role={role}
        qubesdb_read="${{AIRLOCK_QUBESDB_READ:-{database}/bin/qubesdb-read}}"
        qdb_role="$("$qubesdb_read" /features/airlock-role 2>/dev/null || true)"
        vm_name="$("$qubesdb_read" /name 2>/dev/null || true)"

        case "$qdb_role" in
          sys-ctrl|usr-assistant) role="$qdb_role" ;;
          "") ;;
          *) role=base ;;
        esac

        case "$vm_name" in
          sys-ctrl) role=sys-ctrl ;;
          usr-assistant) role=usr-assistant ;;
        esac

        printf 'airlock-policy-probe role=%s user=%s\n' "$role" "$({coreutils}/bin/whoami)"
    "#,
            role = system.quote(system.setting("role")),
            database = system.package("qubes-core-qubesdb"),
            coreutils = system.package("coreutils"),
        )
        .into(),
    )
}
