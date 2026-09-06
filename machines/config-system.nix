{
  lib,
  nixomeConfigSeedInputs ? null,
  nixomeQubeKernel ? null,
  nixomeQubeKernelOpts ? null,
  nixomeTemplatePackages ? null,
  ...
}:
let
  haveTemplates = nixomeTemplatePackages != null;
  haveQubeKernel = nixomeQubeKernel != null && nixomeQubeKernelOpts != null;
  haveConfigSeedInputs = nixomeConfigSeedInputs != null;
in
{
  assertions = [
    {
      assertion = haveTemplates;
      message = "machines/config-system.nix requires nixomeTemplatePackages specialArgs.";
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
    "nixome/config-seed-inputs/nixos-qubes".source =
      if haveConfigSeedInputs then nixomeConfigSeedInputs.nixosQubes else null;
    "nixome/config-seed-inputs/nixome-support".source =
      if haveConfigSeedInputs then nixomeConfigSeedInputs.nixomeSupport else null;
  };

  nixome.qubes = {
    enable = true;

    prefs = {
      # Fresh Nixome dom0 images do not have a Fedora/Qubes VM kernel
      # installed. TemplateVM creation still needs this global property to be
      # defined, even when the imported template later chooses another boot
      # path.
      default_kernel = "";
    };

    templates = {
      nixome-nixos-config-vault = {
        import = true;
        source =
          if haveTemplates then "${nixomeTemplatePackages.configVault}/nixome-nixos-config-vault" else null;
        skipStart = true;
      };

      nixome-nixos-config-staging = {
        import = true;
        source =
          if haveTemplates then
            "${nixomeTemplatePackages.configStaging}/nixome-nixos-config-staging"
          else
            null;
        skipStart = true;
      };

      nixome-nixos-config-validator = {
        import = true;
        source =
          if haveTemplates then
            "${nixomeTemplatePackages.configValidator}/nixome-nixos-config-validator"
          else
            null;
        skipStart = true;
      };
    };

    kernels = {
      ${if haveQubeKernel then nixomeQubeKernel.name else "missing-qubes-vm-kernel"} = {
        source = if haveQubeKernel then "${nixomeQubeKernel.package}" else null;
      };
    };

    domains = {
      nixome-nixos-config-vault = {
        class = "TemplateVM";
        label = "black";
        netvm = null;
        prefs = {
          default_user = "user";
          kernel = if haveQubeKernel then nixomeQubeKernel.name else "";
          kernelopts = if haveQubeKernel then nixomeQubeKernelOpts.configVault else "";
          qrexec_timeout = 120;
          virt_mode = "pvh";
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "config-vault-template";
          os = "Linux";
          qrexec = "1";
        };
      };

      nixome-nixos-config-staging = {
        class = "TemplateVM";
        label = "black";
        netvm = null;
        prefs = {
          default_user = "user";
          kernel = if haveQubeKernel then nixomeQubeKernel.name else "";
          kernelopts = if haveQubeKernel then nixomeQubeKernelOpts.configStaging else "";
          qrexec_timeout = 120;
          virt_mode = "pvh";
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "config-staging-template";
          os = "Linux";
          qrexec = "1";
        };
      };

      nixome-nixos-config-validator = {
        class = "TemplateVM";
        label = "black";
        netvm = null;
        prefs = {
          default_user = "user";
          kernel = if haveQubeKernel then nixomeQubeKernel.name else "";
          kernelopts = if haveQubeKernel then nixomeQubeKernelOpts.configValidator else "";
          qrexec_timeout = 120;
          virt_mode = "pvh";
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "config-validator-template";
          os = "Linux";
          qrexec = "1";
        };
      };

      config-vault = {
        class = "AppVM";
        template = "nixome-nixos-config-vault";
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
        template = "nixome-nixos-config-staging";
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
        template = "nixome-nixos-config-validator";
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

  # This profile carries Nix-built template artifacts in the dom0 store, then
  # imports them into the Qubes pool. It needs room, but not the full large
  # Fedora-template smoke-test disk.
  virtualisation.diskSize = lib.mkForce 32768;
}
