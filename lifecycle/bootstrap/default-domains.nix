{
  lib,
  mkNixomeQubeBuilders,
  nixomeConfigInputs,
  nixomeQubesVmKernelName,
}:
let
  configVmRoles = [
    "config-vault"
    "config-staging"
    "config-validator"
  ];

  configNixPrivateSizeBytes = 8589934592;
  devPrivateSizeBytes = 34359738368;
  sysNetPrivateSizeBytes = 2147483648;

  configRoleDefaults = {
    config-vault = {
      guestHostName = "config-vault";
      label = "black";
      rootImageSizeMiB = 128;
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
      guestHostName = "config-staging";
      label = "gray";
      rootImageSizeMiB = 128;
      state.persistentSize = configNixPrivateSizeBytes;
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
      services = {
        "network-manager" = false;
      };
    };
    config-validator = {
      guestHostName = "config-validator";
      label = "blue";
      rootImageSizeMiB = 128;
      state.persistentSize = configNixPrivateSizeBytes;
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
      services = {
        "network-manager" = false;
      };
    };
  };

  devRoleDefaults = {
    guestHostName = "dev";
    label = "green";
    rootImageSizeMiB = 512;
    state.persistentSize = devPrivateSizeBytes;
    prefs = {
      autostart = false;
      include_in_backups = true;
      memory = 4096;
      maxmem = 8192;
      vcpus = 4;
    };
    features = {
      "nixome-managed" = "1";
      "nixome-role" = "dev";
      "nixome-dev" = "1";
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
      include_in_backups = true;
      memory = 768;
      maxmem = 1024;
      provides_network = true;
      vcpus = 2;
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
    };
    devices.pci = [ ];
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

  mkDevDomainEntry =
    system:
    let
      qubeBuilders = mkNixomeQubeBuilders system;
    in
    qubeBuilders.mkNixNativeDomain {
      name = "dev";
      guestRole = "dev";
      modules = [
        ({ ... }: {
          networking.hostName = devRoleDefaults.guestHostName;
        })
      ];
      rootImageSizeMiB = devRoleDefaults.rootImageSizeMiB;
      kernelName = nixomeQubesVmKernelName;
      label = devRoleDefaults.label;
      netvm = "sys-net";
      prefs = devRoleDefaults.prefs;
      features = devRoleDefaults.features;
      services = { };
      state = devRoleDefaults.state;
    };

  mkSysNetDomainEntry =
    system:
    let
      qubeBuilders = mkNixomeQubeBuilders system;
    in
    qubeBuilders.mkNixNativeDomain {
      name = "sys-net";
      guestRole = "sys-net";
      modules = [
        ({ ... }: {
          networking.hostName = sysNetRoleDefaults.guestHostName;
        })
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
    devRoleDefaults
    materializeNixClosureDomain
    mkConfigDomainEntries
    mkDevDomainEntry
    mkSysNetDomainEntry
    sysNetRoleDefaults
    ;
}
