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
    sys-config-vault = import ./sys-config-vault.nix;
    sys-config-staging = import ./sys-config-staging.nix { inherit configNixPrivateSizeBytes; };
    sys-config-validator = import ./sys-config-validator.nix { inherit configNixPrivateSizeBytes; };
  };

  sysNetRoleDefaults = import ./sys-net.nix { inherit sysNetPrivateSizeBytes; };
  sysFirewallRoleDefaults = import ./sys-firewall.nix { inherit networkDebug; };
  sysGuiRoleDefaults = import ./sys-gui.nix;

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
          # Development/bootstrap login, matching the nested image password.
          # Real deployments supply their own NixOS password configuration.
          users.users.user.initialPassword = "nixome";
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
