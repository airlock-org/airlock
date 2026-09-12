{
  lib,
  airlockConfigDomains ? { },
  airlockConfigInputs ? null,
  airlockQubeKernel ? null,
  ...
}:
let
  configRoles = [
    "sys-config-vault"
    "sys-config-staging"
    "sys-config-validator"
  ];
  missingConfigDomains = builtins.filter (
    role: !(builtins.hasAttr role airlockConfigDomains)
  ) configRoles;
  autostartConfigDomains =
    builtins.filter (role: airlockConfigDomains.${role}.prefs.autostart or false)
      (builtins.filter (role: lib.hasPrefix "sys-config-" role) (builtins.attrNames airlockConfigDomains));
  haveConfigDomains = missingConfigDomains == [ ];
  haveQubeKernel = airlockQubeKernel != null;
  haveConfigInputs = airlockConfigInputs != null;
in
{
  assertions = [
    {
      assertion = haveConfigDomains;
      message = "lifecycle/bootstrap/config-system.nix requires generated nix-closure config domains: ${builtins.concatStringsSep ", " missingConfigDomains}";
    }
    {
      assertion = autostartConfigDomains == [ ];
      message = "sys-config domains must not autostart: ${builtins.concatStringsSep ", " autostartConfigDomains}";
    }
    {
      assertion = haveQubeKernel;
      message = "lifecycle/bootstrap/config-system.nix requires airlockQubeKernel specialArg.";
    }
    {
      assertion = haveConfigInputs;
      message = "lifecycle/bootstrap/config-system.nix requires airlockConfigInputs specialArgs.";
    }
  ];

  environment.etc = {
    "airlock/config-inputs/nixpkgs".source =
      if haveConfigInputs then airlockConfigInputs.nixpkgs else null;
    "airlock/config-inputs/nixpkgs-libvirt_10_5".source =
      if haveConfigInputs && airlockConfigInputs ? nixpkgsLibvirt105 then
        airlockConfigInputs.nixpkgsLibvirt105
      else
        null;
    "airlock/config-inputs/nixos-qubes".source =
      if haveConfigInputs then airlockConfigInputs.nixosQubes else null;
    "airlock/config-inputs/airlock-support".source =
      if haveConfigInputs then airlockConfigInputs.airlockSupport else null;
  };

  airlock.qubes = {
    enable = true;
    setupConfigOnBoot.enable = true;
    startNetworkProvidersOnBoot.enable = true;
    deferredSetupConfigOnBoot.enable = true;
    # Config VMs are created at boot but started only by an explicit config
    # operation. The airlock-config-bootstrap-current command remains installed.
    configCurrentBootstrapOnBoot.enable = false;

    prefs = {
      # Fresh Airlock dom0 images do not have a Fedora/Qubes VM kernel
      # installed. Nix-native domains set their own kernel explicitly.
      default_kernel = "";
      default_guivm = "sys-gui";
      default_netvm = "sys-firewall";
    };

    templates = { };

    kernels = {
      ${if haveQubeKernel then airlockQubeKernel.name else "missing-qubes-vm-kernel"} = {
        source = if haveQubeKernel then "${airlockQubeKernel.package}" else null;
      };
    };

    domains = airlockConfigDomains;

    qrexecPolicy.rules = [
      {
        service = "airlock.OpenUrl";
        source = "sys-gui";
        target = "usr-browser";
        action = "allow";
      }
    ];

    configAccessPolicy = {
      enable = true;
      vault = "sys-config-vault";
      staging = "sys-config-staging";
      validator = "sys-config-validator";
      candidateExportService = "airlock.Config.CandidateExport";
      candidateValidateService = "airlock.Config.ValidateCandidate";
      archiveService = "airlock.Config.ArchiveCandidate";
      archiveDeploymentService = "airlock.Config.ArchiveDeployment";
      currentExportService = "airlock.Config.ExportCurrent";
      currentImportService = "airlock.Config.ImportCurrent";
      configInputsImportService = "airlock.Config.InputsImport";
      allowedSubmitters = [ "usr-cmd" ];
    };
  };

  # This profile may switch a staged dom0 system generation at runtime. Keep the
  # qcow2 capacity large; it is sparse on the host but gives nested dom0 enough
  # room for closure-backed domain artifacts.
  virtualisation.diskSize = lib.mkDefault 131072;
}
