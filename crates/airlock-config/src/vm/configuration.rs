//! Assemble configuration vault, staging and validation services with explicit dependencies.
//! The archive and validation protocols remain separate from machine-authoring policy.

use airlock_build::{Application, Workspace};
use rusix::{
    interop::{NixExpression, NixPath, PackageFunction, raw::NixValue},
    nix_record, nix_text,
};

/// Values shared by the three configuration roles and their qrexec entry points.
struct Build {
    /// Target packages, including the Nix version used to validate candidates.
    pkgs: NixValue,
    /// NixOS library used to quote shell arguments and construct executable search paths.
    lib: NixValue,
    /// Configured user and persistent workspace locations for this VM.
    settings: NixValue,
    /// Wrapper that selects the VM's private Nix store.
    nix: NixValue,
    /// Machine-owned initial runtime source tree, copied by native role commands.
    seed: NixValue,
    /// Read-only mount exposing admitted configuration inputs to unprivileged callers.
    public_inputs: NixValue,
    /// Executable search path for initialization after replacing imported inputs.
    staging_path: NixValue,
}

/// Accept configuration dependencies without giving the services additional control authority.
pub(super) fn constructor() -> PackageFunction {
    PackageFunction::from_function_attrs(
        [
            "pkgs",
            "lib",
            "settings",
            "nix",
            "seed",
            "publicInputs",
            "stagingPath",
        ],
        |args| {
            let build = Build {
                pkgs: args.clone().select("pkgs"),
                lib: args.clone().select("lib"),
                settings: args.clone().select("settings"),
                nix: args.clone().select("nix"),
                seed: args.clone().select("seed"),
                public_inputs: args.clone().select("publicInputs"),
                staging_path: args.select("stagingPath"),
            };

            (Vec::<(&str, NixValue)>::new(), build.services())
        },
    )
}

impl Build {
    /// Select a package from the target system rather than the author's workstation.
    fn package(&self, name: &str) -> NixValue {
        self.pkgs.clone().select(name)
    }

    /// Read a VM workspace setting without forcing unrelated configuration fields.
    fn setting(&self, name: &str) -> NixValue {
        self.settings.clone().select(name)
    }

    /// Install an executable command into the package's bin directory.
    fn command(&self, name: &str, text: NixValue) -> NixValue {
        self.package("writeShellScriptBin").call(name).call(text)
    }

    /// Bind one native role command to immutable installed paths and existing upstream tools.
    fn native(&self, name: &str, action: &str) -> NixValue {
        let workspace = Workspace::new(NixPath::from_source_text(
            self.pkgs
                .clone()
                .select("airlockAppsSource")
                .to_text()
                .into_expr::<String>(),
        ));
        let service = Application::VmService.build(&self.pkgs, &workspace);
        let config = self
            .package("writeText")
            .call("airlock-configuration-runtime.json")
            .call(NixValue::builtin("toJSON").call(nix_record! {
                "user": self.setting("user"),
                "repo": self.setting("configRepoPath"),
                "archive": self.setting("configArchivePath"),
                "validation": self.setting("configValidationPath"),
                "inputs": self.setting("configInputsPath"),
                "store": self.setting("configNixStoreRoot"),
                "public_inputs": self.public_inputs.clone(),
                "seed": self.seed.clone().select("initialRuntimeConfigRepoSource"),
                "nix": nix_text!("{nix}/bin/airlock-config-nix", nix = self.nix.clone()),
                "role": self.setting("role"),
                "staging_path": self.staging_path.clone(),
            }));
        let path = self.lib.clone().select("makeBinPath").call(NixValue::list([
            self.package("coreutils"),
            self.package("gitMinimal"),
            self.package("gnutar"),
            self.package("nix"),
            self.package("util-linux"),
            self.package("gnumake"),
            self.package("jq"),
        ]));
        self.command(
            name,
            nix_text!(
                r#"
            export PATH={path}:{staging_path}:$PATH
            exec {service}/bin/airlock-vm-service configuration --config {config} {action} "$@"
            "#,
                path = path,
                staging_path = self.staging_path.clone(),
                service = service,
                config = config,
                action = action,
            )
            .into(),
        )
    }

    /// Adapt an installed command to a Qubes RPC executable file.
    fn rpc(&self, name: &str, package: NixValue, command: &str) -> NixValue {
        self.package("writeShellScript").call(name).call(nix_text!(
            r#"
            exec {package}/bin/{command}
            "#,
            package = package,
            command = command,
        ))
    }

    /// Bind shared programs once so exported RPCs and interactive commands use identical builds.
    fn services(&self) -> NixValue {
        self.validator().bind(|validator| {
            self.vault().bind(|vault| {
                self.staging().bind(|staging| {
                    validator
                        .clone()
                        .merge_attrs(vault)
                        .merge_attrs(staging.clone())
                        .merge_attrs(nix_record! {
                            "configInputsImportService": self.import_inputs(),
                        })
                })
            })
        })
    }

    /// Validate candidate archives in a separate workspace using the selected private Nix store.
    fn validator(&self) -> NixValue {
        let init = self.native("airlock-config-validator-init", "validator-init");
        let validate = self.native("airlock-config-validator-validate-candidate", "validate");
        nix_record! {
            "configValidatorInit": init,
            "configValidatorValidateCandidate": validate.clone(),
            "configCandidateValidateService": self.rpc("airlock-config-candidate-validate-qrexec", validate, "airlock-config-validator-validate-candidate"),
        }
    }

    /// Archive admitted candidates and deployment receipts without editing the staging checkout.
    fn vault(&self) -> NixValue {
        let init = self.native("airlock-config-vault-init", "vault-init");
        let export = self.native("airlock-config-vault-export-current", "vault-export");
        let candidate = self.native("airlock-config-vault-archive-candidate", "vault-candidate");
        let deployment = self.native(
            "airlock-config-vault-archive-deployment",
            "vault-deployment",
        );
        nix_record! {
            "configVaultInit": init,
            "configVaultExportCurrent": export.clone(),
            "configVaultArchiveCandidate": candidate.clone(),
            "configVaultArchiveDeployment": deployment.clone(),
            "configCurrentExportService": self.rpc("airlock-config-current-export-qrexec", export, "airlock-config-vault-export-current"),
            "configArchiveService": self.rpc("airlock-config-archive-candidate-qrexec", candidate, "airlock-config-vault-archive-candidate"),
            "configDeploymentArchiveService": self.rpc("airlock-config-archive-deployment-qrexec", deployment, "airlock-config-vault-archive-deployment"),
        }
    }

    /// Build the editable candidate workspace and its import, export and validation commands.
    fn staging(&self) -> NixValue {
        let init = self.native("airlock-config-staging-init", "staging-init");
        let import = self.native("airlock-config-staging-import-current", "staging-import");
        let export = self.native("airlock-config-staging-export-candidate", "staging-export");
        let validate = self.native("airlock-config-staging-validate", "staging-validate");
        nix_record! {
            "configStagingInit": init,
            "configStagingImportCurrent": import.clone(),
            "configStagingValidate": validate,
            "configStagingExportCandidate": export.clone(),
            "configCurrentImportService": self.rpc("airlock-config-current-import-qrexec", import, "airlock-config-staging-import-current"),
            "configCandidateExportService": self.rpc("airlock-config-candidate-export-qrexec", export, "airlock-config-staging-export-candidate"),
        }
    }

    /// Import admitted input archives and refresh staging locks as the configured unprivileged user.
    fn import_inputs(&self) -> NixValue {
        let command = self.native("airlock-config-inputs-import", "inputs-import");
        self.rpc(
            "airlock-config-inputs-import-qrexec",
            command,
            "airlock-config-inputs-import",
        )
    }
}
