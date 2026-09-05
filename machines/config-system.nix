{
  lib,
  nixomeTemplatePackages ? null,
  ...
}:
let
  haveTemplates = nixomeTemplatePackages != null;
in
{
  assertions = [
    {
      assertion = haveTemplates;
      message = "machines/config-system.nix requires nixomeTemplatePackages specialArgs.";
    }
  ];

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

      nixome-nixos-agent = {
        import = true;
        source = if haveTemplates then "${nixomeTemplatePackages.agent}/nixome-nixos-agent" else null;
        skipStart = true;
      };
    };

    domains = {
      sys-config-vault = {
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

      sys-agent = {
        class = "AppVM";
        template = "nixome-nixos-agent";
        label = "gray";
        netvm = null;
        prefs = {
          autostart = false;
          include_in_backups = true;
          memory = 4096;
          maxmem = 8192;
          vcpus = 4;
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "agent";
          "nixome-config-reader" = "1";
        };
        services = {
          "meminfo-writer" = true;
        };
      };
    };

    configAccessPolicy = {
      enable = true;
      vault = "sys-config-vault";
      reader = "sys-agent";
      service = "nixome.Config.Export";
    };
  };

  # This profile carries Nix-built template artifacts in the dom0 store, then
  # imports them into the Qubes pool. It needs room, but not the full large
  # Fedora-template smoke-test disk.
  virtualisation.diskSize = lib.mkForce 49152;
}
