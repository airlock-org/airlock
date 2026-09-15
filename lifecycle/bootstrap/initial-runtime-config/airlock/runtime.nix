{
  appliedMessage,
  airlockSupport ? "/etc/airlock/config-inputs/airlock-support",
  pkgs ? null,
  system,
  templateBuilders ? null,
  ...
}:
{
  # Primary edit surface for this Airlock runtime.
  #
  # Edit this file in sys-config-staging, commit the change, validate it in
  # sys-config-validator, then ask dom0 to apply it. sys-config-vault records the
  # candidate and deployment history.

  # Declare additional traditional Qubes domains here. Airlock supports the
  # existing TemplateVM/AppVM model alongside Nix-native domains.
  #
  # Traditional Qubes-backed domain:
  #
  # domains.work-fedora = {
  #   backend.type = "qubes-template";
  #   template = "fedora-44-xfce";
  #   label = "green";
  #   netvm = "sys-firewall";
  # };
  #
  domains = { };

  # Declare Nix-native domains here. The materializer creates a Qubes
  # StandaloneVM from a small Airlock root image plus a separate read-only Nix
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
  #     environment.etc."airlock/domain-role".text = "work-nix\n";
  #   };
  # };
  nixNative.domains = {
    usr-home = {
      role = "base";
      configuration.imports = [
        "${airlockSupport}/lifecycle/misc/modules/usr-home-guest.nix"
      ];
      label = "blue";
      netvm = null;
      rootImageSizeMiB = 1024;
      prefs = {
        autostart = true;
        include_in_backups = true;
        memory = 2048;
        maxmem = 4096;
        vcpus = 2;
      };
      features = {
        "airlock-role" = "usr-home";
        "menu-items" = "";
      };
      services."network-manager" = false;
      state.persistentSize = 17179869184;
    };
    usr-browser = {
      role = "base";
      configuration.imports = [
        "${airlockSupport}/lifecycle/misc/modules/usr-browser-guest.nix"
      ];
      label = "yellow";
      netvm = "sys-firewall";
      rootImageSizeMiB = 1024;
      prefs = {
        include_in_backups = true;
        memory = 2048;
        maxmem = 4096;
        vcpus = 2;
      };
      features = {
        "airlock-role" = "usr-browser";
        "menu-items" = "";
      };
      services."network-manager" = false;
      state.persistentSize = 8589934592;
    };
    usr-cmd = {
      role = "cmd";
      label = "yellow";
      netvm = "sys-firewall";
      rootImageSizeMiB = 1024;
      prefs = {
        include_in_backups = true;
        memory = 2048;
        maxmem = 4096;
        vcpus = 2;
      };
      features = {
        "airlock-role" = "cmd";
      };
      services."network-manager" = false;
      state.persistentSize = 8589934592;
    };
    sys-gui = {
      role = "sys-gui";
      # Initial development login. Replace with a deployment password file.
      configuration.users.users.user.initialPassword = "airlock";
      label = "black";
      netvm = null;
      rootImageSizeMiB = 512;
      prefs = {
        autostart = true;
        guivm = "dom0";
        include_in_backups = true;
        memory = 1024;
        maxmem = 2048;
        vcpus = 2;
      };
      features = {
        gui = "1";
        "gui-allow-fullscreen" = "1";
        "airlock-role" = "sys-gui";
        servicevm = "1";
      };
      services = {
        guivm = true;
        "guivm-gui-agent" = true;
        "network-manager" = false;
      };
    };
    sys-net = {
      role = "sys-net";
      label = "red";
      netvm = null;
      rootImageSizeMiB = 512;
      prefs = {
        autostart = true;
        include_in_backups = true;
        memory = 256;
        maxmem = 512;
        provides_network = true;
        vcpus = 2;
        virt_mode = "hvm";
      };
      features = {
        "airlock-role" = "sys-net";
        "airlock-network-provider" = "sys-net";
      };
      services = {
        "network-manager" = true;
        "qubes-firewall" = true;
        "qubes-network" = true;
        "qubes-updates-proxy" = true;
      };
      # Empty means setup-config discovers dom0 network-class PCI devices for
      # sys-net, matching the normal Qubes installer behavior. Add entries here
      # only when this machine needs explicit pinning.
      devices.pci = [
        # Explicit override example:
        # { id = "00_14.0"; match = "port"; mode = "required"; options.no-strict-reset = true; }
      ];
      state.persistentSize = 2147483648;
    };
    sys-firewall = {
      role = "sys-firewall";
      label = "green";
      netvm = "sys-net";
      rootImageSizeMiB = 512;
      prefs = {
        autostart = true;
        include_in_backups = true;
        memory = 256;
        maxmem = 512;
        provides_network = true;
        vcpus = 1;
      };
      features = {
        "airlock-role" = "sys-firewall";
        "airlock-network-provider" = "sys-firewall";
        servicevm = "1";
      };
      services = {
        "network-manager" = false;
        "qubes-firewall" = true;
        "qubes-network" = true;
        "qubes-updates-proxy" = true;
      };
    };
  };

  # Add or override raw Qubes topology entries here. These become normalized
  # airlock.topology JSON in sys-config-validator before dom0 applies them.
  # Prefer top-level domains for normal additions; extraDomains is a low-level
  # escape hatch that overrides generated domain entries.
  topology = {
    extraTemplates = { };
    extraKernels = { };
    extraPrefs = {
      default_guivm = "sys-gui";
      default_netvm = "sys-firewall";
    };
    extraDomains = { };
    extraQrexecRules = [
      {
        service = "airlock.OpenUrl";
        source = "sys-gui";
        target = "usr-browser";
        action = "allow";
      }
    ];
  };

  # Config authority VMs are fixed-name Nix-native domains. guests.<role>
  # customizes the exact NixOS system closure for each role. domains.<role>
  # customizes the Qubes runtime envelope around that role.
  configVms = {
    guests = {
      sys-config-vault = { };
      sys-config-staging = { };
      sys-config-validator = { };
    };
    domains = { };
  };

  # Add dom0 host configuration here. These fields compile into normal NixOS
  # modules inside nixosConfigurations.airlock-dom0. Use overlays for dom0
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
