{
  appliedMessage,
  pkgs ? null,
  system,
  templateBuilders ? null,
  ...
}:
{
  # Primary edit surface for this Nixome runtime.
  #
  # Edit this file in config-staging, commit the change, validate it in
  # config-validator, then ask dom0 to apply it. config-vault records the
  # candidate and deployment history.

  # Declare additional traditional Qubes domains here. Nixome supports the
  # existing TemplateVM/AppVM model alongside Nix-native domains.
  #
  # Traditional Qubes-backed domain:
  #
  # domains.work-fedora = {
  #   backend.type = "qubes-template";
  #   template = "fedora-44-xfce";
  #   label = "green";
  #   netvm = "sys-net";
  # };
  #
  domains = { };

  # Declare Nix-native domains here. The materializer creates a Qubes
  # StandaloneVM from a small Nixome root image plus a separate read-only Nix
  # store closure image mounted at /nix/store during early boot.
  #
  # nixNative.domains.work-nix = {
  #   role = "base";
  #   label = "green";
  #   netvm = null;
  #   devices.pci = [
  #     {
  #       backend = "dom0";
  #       id = "00_14.0";
  #       match = "port";
  #       mode = "required";
  #       options.no-strict-reset = true;
  #     }
  #   ];
  #   packages = with pkgs; [
  #     git
  #     jq
  #   ];
  #   configuration = {
  #     environment.etc."nixome/domain-role".text = "work-nix\n";
  #   };
  # };
  nixNative.domains = {
    sys-net = {
      role = "sys-net";
      label = "red";
      netvm = null;
      rootImageSizeMiB = 512;
      prefs = {
        autostart = false;
        include_in_backups = true;
        memory = 768;
        maxmem = 1024;
        provides_network = true;
        vcpus = 2;
      };
      features = {
        "nixome-role" = "sys-net";
        "nixome-network-provider" = "sys-net";
      };
      services = {
        "network-manager" = true;
        "qubes-firewall" = true;
        "qubes-network" = true;
      };
      devices.pci = [
        # Fill this with the NIC PCI ID from dom0, for example:
        # { id = "00_14.0"; match = "port"; mode = "required"; options.no-strict-reset = true; }
      ];
      state.persistentSize = 2147483648;
    };

    dev = {
      role = "dev";
      label = "green";
      netvm = "sys-net";
      rootImageSizeMiB = 512;
      prefs = {
        autostart = false;
        include_in_backups = true;
        memory = 4096;
        maxmem = 8192;
        vcpus = 4;
      };
      features = {
        "nixome-role" = "dev";
        "nixome-dev" = "1";
      };
      state.persistentSize = 34359738368;
    };
  };

  # Add or override raw Qubes topology entries here. These become normalized
  # nixome.topology JSON in config-validator before dom0 applies them.
  # Prefer top-level domains for normal additions; extraDomains is a low-level
  # escape hatch that overrides generated domain entries.
  topology = {
    extraTemplates = { };
    extraKernels = { };
    extraPrefs = { };
    extraDomains = { };
    extraQrexecRules = [ ];
  };

  # Config authority VMs are fixed-name Nix-native domains. guests.<role>
  # customizes the exact NixOS system closure for each role. domains.<role>
  # customizes the Qubes runtime envelope around that role.
  configVms = {
    guests = {
      config-vault = { };
      config-staging = { };
      config-validator = { };
    };
    domains = { };
  };

  # Add dom0 host configuration here. These fields compile into normal NixOS
  # modules inside nixosConfigurations.nixome-dom0. Use overlays for dom0
  # package/version overrides.
  dom0.host = {
    # Automatic apply classification:
    # - packages, etc, and systemd fields default to an in-session switch
    # - overlays, configuration, modules, and dom0.extraModules default to the
    #   next boot generation because they can affect Xen/kernel/early services
    # Override only when you know the change class:
    # apply.mode = "switch"; # force live switch
    # apply.mode = "boot";   # force next boot generation
    apply.mode = "auto";

    overlays = [ ];
    packages = [ ];
    etc = { };
    systemd = { };

    # For ordinary NixOS options that do not have a high-level field here.
    configuration = { };

    # Advanced escape hatch: raw NixOS modules appended after the high-level
    # host fields.
    modules = [ ];
  };

  # Backward-compatible raw NixOS modules for dom0. Prefer dom0.host for new
  # config.
  dom0.extraModules = [ ];

  # Low-level escape hatch. These values override dom0.host.apply classification
  # and are mainly for tests or unusual hand-authored plans.
  system.args = { };
}
