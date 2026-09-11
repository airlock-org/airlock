{
  lib,
  mkNixomeQubeBuilders,
  nixomeConfigInputs,
  nixomeQubesVmKernelName,
  networkDebug ? false,
}:
let
  configVmRoles = [
    "sys-config-vault"
    "sys-config-staging"
    "sys-config-validator"
  ];

  configNixPrivateSizeBytes = 8589934592;
  sysNetPrivateSizeBytes = 2147483648;

  configRoleDefaults = {
    sys-config-vault = {
      guestHostName = "sys-config-vault";
      label = "black";
      rootImageSizeMiB = 128;
      prefs = {
        autostart = false;
        guivm = null;
        include_in_backups = true;
        memory = 1024;
        maxmem = 2048;
        vcpus = 1;
      };
      features = {
        "nixome-managed" = "1";
        "nixome-role" = "sys-config-vault";
        "nixome-config-authority" = "1";
      };
      services = {
        "network-manager" = false;
      };
    };
    sys-config-staging = {
      guestHostName = "sys-config-staging";
      label = "gray";
      rootImageSizeMiB = 128;
      state.persistentSize = configNixPrivateSizeBytes;
      prefs = {
        autostart = false;
        guivm = null;
        include_in_backups = true;
        memory = 2048;
        maxmem = 4096;
        vcpus = 2;
      };
      features = {
        "nixome-managed" = "1";
        "nixome-role" = "sys-config-staging";
        "nixome-config-staging" = "1";
        "nixome-config-applied-message" = "initial";
      };
      services = {
        "network-manager" = false;
      };
    };
    sys-config-validator = {
      guestHostName = "sys-config-validator";
      label = "blue";
      rootImageSizeMiB = 128;
      state.persistentSize = configNixPrivateSizeBytes;
      prefs = {
        autostart = false;
        guivm = null;
        include_in_backups = true;
        memory = 2048;
        maxmem = 4096;
        vcpus = 2;
      };
      features = {
        "nixome-managed" = "1";
        "nixome-role" = "sys-config-validator";
        "nixome-config-validator" = "1";
      };
      services = {
        "network-manager" = false;
      };
    };
  };

  sysNetRoleDefaults = {
    guestHostName = "sys-net";
    label = "red";
    rootImageSizeMiB = 512;
    netvm = null;
    state.persistentSize = sysNetPrivateSizeBytes;
    prefs = {
      autostart = true;
      guivm = null;
      include_in_backups = true;
      memory = 768;
      maxmem = 1024;
      provides_network = true;
      vcpus = 2;
      virt_mode = "hvm";
    };
    features = {
      "nixome-managed" = "1";
      "nixome-role" = "sys-net";
      "nixome-network-provider" = "sys-net";
    };
    services = {
      "network-manager" = true;
      "qubes-firewall" = true;
      "qubes-network" = true;
      "qubes-updates-proxy" = true;
    };
    devices.pci = [ ];
  };

  sysFirewallRoleDefaults = {
    guestHostName = "sys-firewall";
    label = "green";
    rootImageSizeMiB = 512;
    netvm = "sys-net";
    prefs = {
      autostart = !networkDebug;
      guivm = null;
      include_in_backups = true;
      memory = 500;
      maxmem = 1024;
      provides_network = true;
      vcpus = 1;
    };
    features = {
      "nixome-managed" = "1";
      "nixome-role" = "sys-firewall";
      "nixome-network-provider" = "sys-firewall";
      servicevm = "1";
    };
    services = {
      "network-manager" = false;
      "qubes-firewall" = true;
      "qubes-network" = true;
      "qubes-updates-proxy" = true;
    };
  };

  sysGuiRoleDefaults = {
    guestHostName = "sys-gui";
    label = "black";
    rootImageSizeMiB = 512;
    netvm = null;
    prefs = {
      autostart = true;
      guivm = "dom0";
      include_in_backups = true;
      memory = 2048;
      maxmem = 4096;
      vcpus = 2;
    };
    features = {
      gui = "1";
      "gui-allow-fullscreen" = "1";
      "nixome-managed" = "1";
      "nixome-role" = "sys-gui";
      servicevm = "1";
    };
    services = {
      guivm = true;
      "guivm-gui-agent" = true;
      "network-manager" = false;
    };
  };

  mkConfigDomainEntries =
    system:
    let
      qubeBuilders = mkNixomeQubeBuilders system;
      mkGuestModule =
        role:
        let
          defaults = configRoleDefaults.${role};
        in
        { ... }:
        {
          networking.hostName = defaults.guestHostName;
        };
      mkEntry =
        role:
        let
          defaults = configRoleDefaults.${role};
        in
        qubeBuilders.mkNixNativeDomain {
          name = role;
          guestRole = role;
          inherit nixomeConfigInputs;
          nixomePrebuiltTemplateSources = { };
          modules = [ (mkGuestModule role) ];
          rootImageSizeMiB = defaults.rootImageSizeMiB;
          kernelName = nixomeQubesVmKernelName;
          label = defaults.label;
          netvm = null;
          prefs = defaults.prefs;
          features = defaults.features;
          services = defaults.services;
          state = defaults.state or { };
        };
    in
    lib.listToAttrs (
      builtins.map (role: {
        name = role;
        value = mkEntry role;
      }) configVmRoles
    );

  mkSysNetDomainEntry =
    system:
    let
      qubeBuilders = mkNixomeQubeBuilders system;
    in
    qubeBuilders.mkNixNativeDomain {
      name = "sys-net";
      guestRole = "sys-net";
      modules = [
        (
          { ... }:
          {
            networking.hostName = sysNetRoleDefaults.guestHostName;
          }
          // lib.optionalAttrs networkDebug {
            nixome.qubeGuest.allowQrexecUserSelection = true;
            systemd.services.NetworkManager.wantedBy = lib.mkForce [ ];
          }
        )
      ];
      rootImageSizeMiB = sysNetRoleDefaults.rootImageSizeMiB;
      kernelName = nixomeQubesVmKernelName;
      label = sysNetRoleDefaults.label;
      netvm = sysNetRoleDefaults.netvm;
      prefs = sysNetRoleDefaults.prefs;
      features = sysNetRoleDefaults.features;
      services = sysNetRoleDefaults.services;
      devices = sysNetRoleDefaults.devices;
      state = sysNetRoleDefaults.state;
    };

  mkSysFirewallDomainEntry =
    system:
    let
      qubeBuilders = mkNixomeQubeBuilders system;
    in
    qubeBuilders.mkNixNativeDomain {
      name = "sys-firewall";
      guestRole = "sys-firewall";
      modules = [
        ({ ... }: {
          networking.hostName = sysFirewallRoleDefaults.guestHostName;
        })
      ];
      rootImageSizeMiB = sysFirewallRoleDefaults.rootImageSizeMiB;
      kernelName = nixomeQubesVmKernelName;
      label = sysFirewallRoleDefaults.label;
      netvm = sysFirewallRoleDefaults.netvm;
      prefs = sysFirewallRoleDefaults.prefs;
      features = sysFirewallRoleDefaults.features;
      services = sysFirewallRoleDefaults.services;
    };

  mkSysGuiDomainEntry =
    system:
    let
      qubeBuilders = mkNixomeQubeBuilders system;
    in
    qubeBuilders.mkNixNativeDomain {
      name = "sys-gui";
      guestRole = "sys-gui";
      modules = [
        ({ ... }: {
          networking.hostName = sysGuiRoleDefaults.guestHostName;
        })
      ];
      rootImageSizeMiB = sysGuiRoleDefaults.rootImageSizeMiB;
      kernelName = nixomeQubesVmKernelName;
      label = sysGuiRoleDefaults.label;
      netvm = sysGuiRoleDefaults.netvm;
      prefs = sysGuiRoleDefaults.prefs;
      features = sysGuiRoleDefaults.features;
      services = sysGuiRoleDefaults.services;
    };

  materializeNixClosureDomain =
    entry:
    entry.domain
    // {
      prefs = entry.domain.prefs // {
        kernelopts = entry.kernelopts;
      };
      backend = entry.domain.backend // {
        system = entry.domain.backend.system // {
          source = "${entry.configuration.config.system.build.toplevel}";
          sourceRef = null;
        };
        root = entry.domain.backend.root // {
          source = "${entry.rootImage}/root.img";
          sourceRef = null;
        };
        store = entry.domain.backend.store // {
          source = "${entry.storeArtifact}";
          sourceRef = null;
        };
      };
    };
in
{
  inherit
    configRoleDefaults
    configVmRoles
    materializeNixClosureDomain
    mkConfigDomainEntries
    mkSysFirewallDomainEntry
    mkSysGuiDomainEntry
    mkSysNetDomainEntry
    sysFirewallRoleDefaults
    sysGuiRoleDefaults
    sysNetRoleDefaults
    ;
}
