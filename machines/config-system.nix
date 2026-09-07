{
  lib,
  nixomeConfigSeedInputs ? null,
  nixomePrebuiltTemplatePackages ? { },
  nixomePrebuiltTemplateSources ? { },
  nixomeConfigSystemTemplateImport ? true,
  nixomeQubeKernel ? null,
  nixomeQubeKernelOpts ? null,
  nixomeTemplatePackages ? null,
  ...
}:
let
  configSystemTemplateSource =
    if nixomePrebuiltTemplateSources ? configSystem then
      nixomePrebuiltTemplateSources.configSystem
    else if nixomePrebuiltTemplateSources ? nixome-nixos-config-system then
      nixomePrebuiltTemplateSources.nixome-nixos-config-system
    else if nixomeTemplatePackages != null && nixomeTemplatePackages ? configSystem then
      "${nixomeTemplatePackages.configSystem}/nixome-nixos-config-system"
    else
      null;
  haveTemplates = !nixomeConfigSystemTemplateImport || configSystemTemplateSource != null;
  haveQubeKernel =
    nixomeQubeKernel != null && nixomeQubeKernelOpts != null && nixomeQubeKernelOpts ? configSystem;
  haveConfigSeedInputs = nixomeConfigSeedInputs != null;
in
{
  assertions = [
    {
      assertion = haveTemplates;
      message = "machines/config-system.nix requires a config-system template package/source when template import is enabled.";
    }
    {
      assertion = haveQubeKernel;
      message = "machines/config-system.nix requires nixomeQubeKernel and nixomeQubeKernelOpts specialArgs.";
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

  environment.systemPackages = lib.attrValues nixomePrebuiltTemplatePackages;

  nixome.qubes = {
    enable = true;
    setupConfigOnBoot.enable = true;

    prefs = {
      # Fresh Nixome dom0 images do not have a Fedora/Qubes VM kernel
      # installed. TemplateVM creation still needs this global property to be
      # defined, even when the imported template later chooses another boot
      # path.
      default_kernel = "";
    };

    templates = {
      nixome-nixos-config-system = {
        import = nixomeConfigSystemTemplateImport;
        source = if nixomeConfigSystemTemplateImport then configSystemTemplateSource else null;
        skipStart = true;
      };
    };

    kernels = {
      ${if haveQubeKernel then nixomeQubeKernel.name else "missing-qubes-vm-kernel"} = {
        source = if haveQubeKernel then "${nixomeQubeKernel.package}" else null;
      };
    };

    domains = {
      nixome-nixos-config-system = {
        class = "TemplateVM";
        label = "black";
        netvm = null;
        prefs = {
          default_user = "user";
          kernel = if haveQubeKernel then nixomeQubeKernel.name else "";
          kernelopts = if haveQubeKernel then nixomeQubeKernelOpts.configSystem else "";
          qrexec_timeout = 120;
          virt_mode = "pvh";
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "config-system-template";
          os = "Linux";
          qrexec = "1";
        };
      };

      config-vault = {
        class = "AppVM";
        template = "nixome-nixos-config-system";
        label = "black";
        netvm = null;
        prefs = {
          autostart = false;
          include_in_backups = true;
          memory = 1024;
          maxmem = 2048;
          vcpus = 1;
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "config-vault";
          "nixome-config-authority" = "1";
        };
        services = {
          "network-manager" = false;
        };
      };

      config-staging = {
        class = "AppVM";
        template = "nixome-nixos-config-system";
        label = "gray";
        netvm = null;
        prefs = {
          autostart = false;
          include_in_backups = true;
          memory = 2048;
          maxmem = 4096;
          vcpus = 2;
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "config-staging";
          "nixome-config-staging" = "1";
          "nixome-config-applied-message" = "initial";
        };
      };

      config-validator = {
        class = "AppVM";
        template = "nixome-nixos-config-system";
        label = "blue";
        netvm = null;
        prefs = {
          autostart = false;
          include_in_backups = true;
          memory = 2048;
          maxmem = 4096;
          vcpus = 2;
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "config-validator";
          "nixome-config-validator" = "1";
        };
      };
    };

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

  # This profile carries Nix-built template artifacts and may switch a staged
  # dom0 system generation at runtime. Keep the qcow2 capacity large; it is
  # sparse on the host but gives the nested dom0 store enough room to grow.
  virtualisation.diskSize = lib.mkDefault 131072;
}
