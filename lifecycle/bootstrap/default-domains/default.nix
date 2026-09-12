{
  lib,
  mkAirlockQubeBuilders,
  airlockConfigInputs,
  airlockQubesVmKernelName,
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
  cmdVmRoleDefaults = import ./usr-cmd.nix;
  homeVmRoleDefaults = import ./usr-home.nix;
  browserVmRoleDefaults = import ./usr-browser.nix;

  mkConfigDomainEntries =
    system:
    let
      qubeBuilders = mkAirlockQubeBuilders system;
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
          inherit airlockConfigInputs;
          airlockPrebuiltTemplateSources = { };
          modules = [ (mkGuestModule role) ];
          rootImageSizeMiB = defaults.rootImageSizeMiB;
          kernelName = airlockQubesVmKernelName;
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
      qubeBuilders = mkAirlockQubeBuilders system;
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
            airlock.qubeGuest.allowQrexecUserSelection = true;
            systemd.services.NetworkManager.wantedBy = lib.mkForce [ ];
          }
        )
      ];
      rootImageSizeMiB = sysNetRoleDefaults.rootImageSizeMiB;
      kernelName = airlockQubesVmKernelName;
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
      qubeBuilders = mkAirlockQubeBuilders system;
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
      kernelName = airlockQubesVmKernelName;
      label = sysFirewallRoleDefaults.label;
      netvm = sysFirewallRoleDefaults.netvm;
      prefs = sysFirewallRoleDefaults.prefs;
      features = sysFirewallRoleDefaults.features;
      services = sysFirewallRoleDefaults.services;
    };

  mkSysGuiDomainEntryWith =
    system:
    {
      personalLaunchers ? true,
      configurationName ? "airlock-domain-sys-gui",
    }:
    let
      qubeBuilders = mkAirlockQubeBuilders system;
    in
    qubeBuilders.mkNixNativeDomain {
      name = "sys-gui";
      guestRole = "sys-gui";
      inherit configurationName;
      modules = [
        ({ ... }: {
          networking.hostName = sysGuiRoleDefaults.guestHostName;
          airlock.sysGuiGuest.personalLaunchers.enable = personalLaunchers;
          # Development/bootstrap login, matching the nested image password.
          # Real deployments supply their own NixOS password configuration.
          users.users.user.initialPassword = "airlock";
        })
      ];
      rootImageSizeMiB = sysGuiRoleDefaults.rootImageSizeMiB;
      kernelName = airlockQubesVmKernelName;
      label = sysGuiRoleDefaults.label;
      netvm = sysGuiRoleDefaults.netvm;
      prefs = sysGuiRoleDefaults.prefs;
      features = sysGuiRoleDefaults.features;
      services = sysGuiRoleDefaults.services;
    };

  mkSysGuiDomainEntry = system: mkSysGuiDomainEntryWith system { };

  mkCmdVmDomainEntry =
    system:
    let
      qubeBuilders = mkAirlockQubeBuilders system;
    in
    qubeBuilders.mkNixNativeDomain {
      name = "usr-cmd";
      guestRole = "cmd";
      modules = [
        ({ ... }: {
          networking.hostName = cmdVmRoleDefaults.guestHostName;
        })
      ];
      rootImageSizeMiB = cmdVmRoleDefaults.rootImageSizeMiB;
      kernelName = airlockQubesVmKernelName;
      label = cmdVmRoleDefaults.label;
      netvm = cmdVmRoleDefaults.netvm;
      prefs = cmdVmRoleDefaults.prefs;
      features = cmdVmRoleDefaults.features;
      services = cmdVmRoleDefaults.services;
      state = cmdVmRoleDefaults.state;
    };

  mkPersonalVmDomainEntry =
    system: name: defaults: guestModule:
    let
      qubeBuilders = mkAirlockQubeBuilders system;
    in
    qubeBuilders.mkNixNativeDomain {
      inherit name;
      guestRole = "base";
      modules = [
        ({ ... }: { networking.hostName = defaults.guestHostName; })
        guestModule
      ];
      rootImageSizeMiB = defaults.rootImageSizeMiB;
      kernelName = airlockQubesVmKernelName;
      label = defaults.label;
      netvm = defaults.netvm;
      prefs = defaults.prefs;
      features = defaults.features;
      services = defaults.services;
      state = defaults.state;
    };

  mkHomeVmDomainEntry = system:
    mkPersonalVmDomainEntry system "usr-home" homeVmRoleDefaults ../../misc/modules/usr-home-guest.nix;

  mkBrowserVmDomainEntry = system:
    mkPersonalVmDomainEntry system "usr-browser" browserVmRoleDefaults ../../misc/modules/usr-browser-guest.nix;

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
    mkCmdVmDomainEntry
    mkHomeVmDomainEntry
    mkBrowserVmDomainEntry
    mkSysFirewallDomainEntry
    mkSysGuiDomainEntry
    mkSysGuiDomainEntryWith
    mkSysNetDomainEntry
    sysFirewallRoleDefaults
    sysGuiRoleDefaults
    sysNetRoleDefaults
    ;
}
