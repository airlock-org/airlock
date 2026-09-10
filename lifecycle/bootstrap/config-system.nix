{
  lib,
  nixomeConfigDomains ? { },
  nixomeConfigInputs ? null,
  nixomeQubeKernel ? null,
  ...
}:
let
  configRoles = [
    "config-vault"
    "config-staging"
    "config-validator"
  ];
  missingConfigDomains = builtins.filter (
    role: !(builtins.hasAttr role nixomeConfigDomains)
  ) configRoles;
  haveConfigDomains = missingConfigDomains == [ ];
  haveQubeKernel = nixomeQubeKernel != null;
  haveConfigInputs = nixomeConfigInputs != null;
in
{
  assertions = [
    {
      assertion = haveConfigDomains;
      message = "lifecycle/bootstrap/config-system.nix requires generated nix-closure config domains: ${builtins.concatStringsSep ", " missingConfigDomains}";
    }
    {
      assertion = haveQubeKernel;
      message = "lifecycle/bootstrap/config-system.nix requires nixomeQubeKernel specialArg.";
    }
    {
      assertion = haveConfigInputs;
      message = "lifecycle/bootstrap/config-system.nix requires nixomeConfigInputs specialArgs.";
    }
  ];

  environment.etc = {
    "nixome/config-inputs/nixpkgs".source =
      if haveConfigInputs then nixomeConfigInputs.nixpkgs else null;
    "nixome/config-inputs/nixpkgs-libvirt_10_5".source =
      if haveConfigInputs && nixomeConfigInputs ? nixpkgsLibvirt105 then
        nixomeConfigInputs.nixpkgsLibvirt105
      else
        null;
    "nixome/config-inputs/nixos-qubes".source =
      if haveConfigInputs then nixomeConfigInputs.nixosQubes else null;
    "nixome/config-inputs/nixome-support".source =
      if haveConfigInputs then nixomeConfigInputs.nixomeSupport else null;
  };

  nixome.qubes = {
    enable = true;
    setupConfigOnBoot.enable = true;
    configCurrentBootstrapOnBoot.enable = true;

    prefs = {
      # Fresh Nixome dom0 images do not have a Fedora/Qubes VM kernel
      # installed. Nix-native domains set their own kernel explicitly.
      default_kernel = "";
    };

    templates = { };

    kernels = {
      ${if haveQubeKernel then nixomeQubeKernel.name else "missing-qubes-vm-kernel"} = {
        source = if haveQubeKernel then "${nixomeQubeKernel.package}" else null;
      };
    };

    domains = nixomeConfigDomains;

    configAccessPolicy = {
      enable = true;
      vault = "config-vault";
      staging = "config-staging";
      validator = "config-validator";
      candidateExportService = "nixome.Config.CandidateExport";
      candidateValidateService = "nixome.Config.ValidateCandidate";
      archiveService = "nixome.Config.ArchiveCandidate";
      archiveDeploymentService = "nixome.Config.ArchiveDeployment";
      currentExportService = "nixome.Config.ExportCurrent";
      currentImportService = "nixome.Config.ImportCurrent";
      configInputsImportService = "nixome.Config.InputsImport";
    };
  };

  # This profile may switch a staged dom0 system generation at runtime. Keep the
  # qcow2 capacity large; it is sparse on the host but gives nested dom0 enough
  # room for closure-backed domain artifacts.
  virtualisation.diskSize = lib.mkDefault 131072;
}
