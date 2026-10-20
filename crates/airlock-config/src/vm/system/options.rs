//! Declare the settings shared by VM services that participate in NixOS module merging.

use rusix::{interop::raw::NixValue, nix_record, nix_text};

/// Keep workspace defaults relative to the selected user and validate them in the VM module.
pub(super) fn module() -> NixValue {
    NixValue::function_attrs_open(["config", "lib"], |args| {
        let lib = args.clone().select("lib");
        let user = args.select("config.airlock.qubeGuest.user");
        let string = lib.clone().select("types.str");
        let option = |kind, default, description: &str| {
            lib.clone().select("mkOption").call(nix_record! {
                "type": kind,
                "default": default,
                "description": description,
            })
        };
        let mut options = vec![
            (
                "enable",
                lib.clone()
                    .select("mkEnableOption")
                    .call("Airlock Qubes VM userspace"),
            ),
            (
                "role",
                option(
                    lib.clone().select("types.enum").call(NixValue::list(
                        [
                            "base",
                            "sys-config-vault",
                            "sys-config-staging",
                            "sys-config-validator",
                            "cmd",
                            "sys-firewall",
                            "sys-gui",
                            "sys-net",
                            "usr-assistant",
                            "sys-ctrl",
                        ]
                        .map(Into::into),
                    )),
                    "base".into(),
                    "Userspace capability set installed in this VM.",
                ),
            ),
            (
                "user",
                option(
                    string.clone(),
                    "user".into(),
                    "Primary unprivileged VM user.",
                ),
            ),
            (
                "allowQrexecUserSelection",
                option(
                    lib.clone().select("types.bool"),
                    false.into(),
                    "Allow qubes.VMShell callers to select the target user.",
                ),
            ),
            (
                "configNixPackage",
                option(
                    lib.clone()
                        .select("types.nullOr")
                        .call(lib.clone().select("types.package")),
                    NixValue::null(),
                    "CLI or controller service package used by configuration build services.",
                ),
            ),
            (
                "cmdPersistentPaths",
                option(
                    lib.clone().select("types.listOf").call(string.clone()),
                    NixValue::list([nix_text!("/home/{user}/dev", user = user.clone()).into()]),
                    "Persistent user directories initialized for command workspaces.",
                ),
            ),
        ];
        for (name, root, suffix, description) in [
            (
                "configRepoPath",
                "/home",
                "airlock-config",
                "Editable configuration repository.",
            ),
            (
                "configValidationPath",
                "/home",
                "airlock-config-validation",
                "Candidate validation results.",
            ),
            (
                "configArchivePath",
                "/home",
                "airlock-config-vault",
                "Archived configuration candidates.",
            ),
            (
                "configNixStoreRoot",
                "/rw/home",
                "airlock-config-nix-root",
                "Private writable Nix store root.",
            ),
            (
                "configInputsPath",
                "/rw/home",
                "airlock-config-inputs",
                "Persistent imported offline build inputs.",
            ),
            (
                "codexHome",
                "/home",
                ".codex",
                "Persistent Codex CLI state exposed through CODEX_HOME.",
            ),
        ] {
            options.push((
                name,
                option(
                    string.clone(),
                    nix_text!(
                        "{root}/{user}/{suffix}",
                        root = root,
                        user = user.clone(),
                        suffix = suffix,
                    )
                    .into(),
                    description,
                ),
            ));
        }

        (
            Vec::<(&str, NixValue)>::new(),
            nix_record! {
                "options": nix_record! {
                    "airlock": nix_record! {
                        "qubeGuest": NixValue::record(options),
                    },
                },
            },
        )
    })
}
