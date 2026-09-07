{
  lib,
  nixomeConfigDomains ? { },
  nixomeConfigSeedInputs ? null,
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
  haveConfigSeedInputs = nixomeConfigSeedInputs != null;
in
{
  assertions = [
    {
      assertion = haveConfigDomains;
      message = "machines/config-system.nix requires generated nix-closure config domains: ${builtins.concatStringsSep ", " missingConfigDomains}";
    }
    {
      assertion = haveQubeKernel;
      message = "machines/config-system.nix requires nixomeQubeKernel specialArg.";
    }
    {
      assertion = haveConfigSeedInputs;
      message = "machines/config-system.nix requires nixomeConfigSeedInputs specialArgs.";
    }
  ];

  environment.etc = {
    "nixome/config-seed-inputs/nixpkgs".source =
      if haveConfigSeedInputs then nixomeConfigSeedInputs.nixpkgs else null;
    "nixome/config-seed-inputs/nixpkgs-libvirt_10_5".source =
      if haveConfigSeedInputs && nixomeConfigSeedInputs ? nixpkgsLibvirt105 then
        nixomeConfigSeedInputs.nixpkgsLibvirt105
      else
        null;
    "nixome/config-seed-inputs/nixos-qubes".source =
      if haveConfigSeedInputs then nixomeConfigSeedInputs.nixosQubes else null;
    "nixome/config-seed-inputs/nixome-support".source =
      if haveConfigSeedInputs then nixomeConfigSeedInputs.nixomeSupport else null;
  };

  nixome.qubes = {
    enable = true;
    setupConfigOnBoot.enable = true;

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
    };
  };

  # This profile may switch a staged dom0 system generation at runtime. Keep the
  # qcow2 capacity large; it is sparse on the host but gives nested dom0 enough
  # room for closure-backed domain artifacts.
  virtualisation.diskSize = lib.mkDefault 131072;
}
