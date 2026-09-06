{
  lib,
  nixomeQubeKernel ? null,
  nixomeQubeKernelOpts ? null,
  nixomeTemplatePackages ? null,
  ...
}:
let
  haveTemplates = nixomeTemplatePackages != null;
  haveQubeKernel = nixomeQubeKernel != null && nixomeQubeKernelOpts != null;
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
  ];

  nixome.qubes = {
    enable = true;
    reconcileOnBoot = true;

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

      nixome-nixos-config-mod = {
        import = true;
        source =
          if haveTemplates then "${nixomeTemplatePackages.configMod}/nixome-nixos-config-mod" else null;
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
        services = {
          "meminfo-writer" = true;
        };
      };

      nixome-nixos-config-mod = {
        class = "TemplateVM";
        label = "black";
        netvm = null;
        prefs = {
          default_user = "user";
          kernel = if haveQubeKernel then nixomeQubeKernel.name else "";
          kernelopts = if haveQubeKernel then nixomeQubeKernelOpts.configMod else "";
          qrexec_timeout = 120;
          virt_mode = "pvh";
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "config-mod-template";
          os = "Linux";
          qrexec = "1";
        };
        services = {
          "meminfo-writer" = true;
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
          "meminfo-writer" = true;
        };
      };

      config-mod = {
        class = "AppVM";
        template = "nixome-nixos-config-mod";
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
          "nixome-role" = "config-mod";
          "nixome-config-modifier" = "1";
        };
        services = {
          "meminfo-writer" = true;
        };
      };
    };

    configAccessPolicy = {
      enable = true;
      vault = "config-vault";
      modifier = "config-mod";
      candidateExportService = "nixome.Config.CandidateExport";
      archiveService = "nixome.Config.ArchiveCandidate";
    };
  };

  # This profile carries Nix-built template artifacts in the dom0 store, then
  # imports them into the Qubes pool. It needs room, but not the full large
  # Fedora-template smoke-test disk.
  virtualisation.diskSize = lib.mkForce 32768;
}
