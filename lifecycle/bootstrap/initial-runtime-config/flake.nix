{
  description = "Airlock runtime configuration candidate";

  inputs.nixpkgs.url = "__AIRLOCK_NIXPKGS_URL__";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      topologySchema = import ./airlock/topology.nix;
      systemSchema = import ./airlock/system-schema.nix;
      configVms = import ./airlock/config-vms.nix;
      systemConfig = import ./airlock/system.nix;
      expectRuntimeAttrs =
        path: value:
        let
          prefix = "airlock.runtime${if path == "" then "" else ".${path}"}";
        in
        if builtins.typeOf value == "set" then
          value
        else
          throw "${prefix}: expected an attribute set, got ${builtins.typeOf value}";
      checkRuntimeKeys =
        path: allowed: attrs:
        let
          unknown = builtins.filter (name: !(builtins.elem name allowed)) (builtins.attrNames attrs);
          prefix = "airlock.runtime${if path == "" then "" else ".${path}"}";
        in
        if unknown == [ ] then
          attrs
        else
          throw "${prefix}: unknown option(s): ${nixpkgs.lib.concatStringsSep ", " unknown}";
      rawLocalConfig = import ./airlock/runtime.nix {
        inherit
          appliedMessage
          airlockSupport
          system
          ;
        pkgs = templateBuilders.pkgs;
      };
      localConfig = checkRuntimeKeys "" [
        "configVms"
        "domains"
        "dom0"
        "nixNative"
        "system"
        "topology"
      ] (expectRuntimeAttrs "" rawLocalConfig);
      localTopology = localConfig.topology or { };
      localDomains = localConfig.domains or { };
      localConfigVms = localConfig.configVms or { };
      configVmRoles = [
        "sys-config-vault"
        "sys-config-staging"
        "sys-config-validator"
      ];
      checkConfigKeys =
        path: allowed: attrs:
        let
          unknown = builtins.filter (name: !(builtins.elem name allowed)) (builtins.attrNames attrs);
          prefix = "airlock.configVms${if path == "" then "" else ".${path}"}";
        in
        if unknown == [ ] then
          attrs
        else
          throw "${prefix}: unknown option(s): ${nixpkgs.lib.concatStringsSep ", " unknown}";
      expectConfigAttrs =
        path: value:
        let
          prefix = "airlock.configVms${if path == "" then "" else ".${path}"}";
        in
        if builtins.typeOf value == "set" then
          value
        else
          throw "${prefix}: expected an attribute set, got ${builtins.typeOf value}";
      checkConfigDomainOverride =
        path: value:
        checkConfigKeys path [
          "features"
          "label"
          "netvm"
          "prefs"
          "services"
          "state"
        ] (expectConfigAttrs path value);
      checkConfigGuest =
        path: value:
        checkConfigKeys path [
          "configuration"
          "etc"
          "hostName"
          "hostname"
          "modules"
          "packages"
          "systemd"
        ] (expectConfigAttrs path value);
      checkNixNativeKeys =
        path: allowed: attrs:
        let
          unknown = builtins.filter (name: !(builtins.elem name allowed)) (builtins.attrNames attrs);
          prefix = "airlock.nixNative${if path == "" then "" else ".${path}"}";
        in
        if unknown == [ ] then
          attrs
        else
          throw "${prefix}: unknown option(s): ${nixpkgs.lib.concatStringsSep ", " unknown}";
      expectNixNativeAttrs =
        path: value:
        let
          prefix = "airlock.nixNative${if path == "" then "" else ".${path}"}";
        in
        if builtins.typeOf value == "set" then
          value
        else
          throw "${prefix}: expected an attribute set, got ${builtins.typeOf value}";
      checkNixNativeDomain =
        path: value:
        checkNixNativeKeys path [
          "additionalRootSpace"
          "configuration"
          "devices"
          "etc"
          "features"
          "hostName"
          "hostname"
          "kernel"
          "label"
          "modules"
          "netvm"
          "packages"
          "prefs"
          "role"
          "services"
          "state"
          "systemd"
          "rootImageSizeMiB"
        ] (expectNixNativeAttrs path value);
      checkNixNativeDomainRole =
        path: definition:
        let
          role = definition.role or "base";
        in
        if
          builtins.elem role [
            "base"
            "cmd"
            "sys-firewall"
            "sys-gui"
            "sys-net"
          ]
        then
          role
        else
          throw "airlock.nixNative.${path}.role: expected one of base, cmd, sys-firewall, sys-gui, sys-net";
      checkedNixNative =
        let
          checked = checkNixNativeKeys "" [ "domains" ] (
            expectNixNativeAttrs "" (localConfig.nixNative or { })
          );
        in
        checked
        // {
          domains = builtins.mapAttrs (
            name: value:
            let
              checkedDomain = checkNixNativeDomain "domains.${name}" value;
            in
            checkedDomain
            // {
              role = checkNixNativeDomainRole "domains.${name}" checkedDomain;
            }
          ) (checked.domains or { });
        };
      checkConfigRoleAttrs =
        path: attrs:
        let
          unknown = builtins.filter (role: !(builtins.elem role configVmRoles)) (builtins.attrNames attrs);
        in
        if unknown == [ ] then
          attrs
        else
          throw "airlock.configVms.${path}: unknown role(s): ${nixpkgs.lib.concatStringsSep ", " unknown}";
      checkedConfigVms =
        let
          checked = checkConfigKeys "" [
            "domains"
            "guests"
          ] (expectConfigAttrs "" localConfigVms);
          rawDomains = checkConfigRoleAttrs "domains" (checked.domains or { });
          rawGuests = checkConfigRoleAttrs "guests" (checked.guests or { });
          checkedDomains = builtins.mapAttrs (
            role: value: checkConfigDomainOverride "domains.${role}" value
          ) rawDomains;
          checkedGuests = builtins.mapAttrs (role: value: checkConfigGuest "guests.${role}" value) rawGuests;
        in
        checked
        // {
          domains = checkedDomains;
          guests = checkedGuests;
        };
      expectDom0Attrs =
        path: value:
        let
          prefix = "airlock.dom0${if path == "" then "" else ".${path}"}";
        in
        if builtins.typeOf value == "set" then
          value
        else
          throw "${prefix}: expected an attribute set, got ${builtins.typeOf value}";
      checkDom0Keys =
        path: allowed: attrs:
        let
          unknown = builtins.filter (name: !(builtins.elem name allowed)) (builtins.attrNames attrs);
          prefix = "airlock.dom0${if path == "" then "" else ".${path}"}";
        in
        if unknown == [ ] then
          attrs
        else
          throw "${prefix}: unknown option(s): ${nixpkgs.lib.concatStringsSep ", " unknown}";
      checkDom0Host =
        value:
        checkDom0Keys "host" [
          "apply"
          "configuration"
          "etc"
          "modules"
          "overlays"
          "packages"
          "systemd"
        ] (expectDom0Attrs "host" value);
      checkDom0HostApply =
        value:
        let
          apply = checkDom0Keys "host.apply" [
            "mode"
            "reason"
          ] (expectDom0Attrs "host.apply" value);
          mode = apply.mode or "auto";
        in
        if
          !(builtins.elem mode [
            "auto"
            "switch"
            "boot"
          ])
        then
          throw "airlock.dom0.host.apply.mode: expected one of auto, switch, boot"
        else
          {
            inherit mode;
          }
          // (
            if apply ? reason then
              {
                reason =
                  if builtins.typeOf apply.reason == "string" || builtins.typeOf apply.reason == "path" then
                    apply.reason
                  else
                    throw "airlock.dom0.host.apply.reason: expected a string or path, got ${builtins.typeOf apply.reason}";
              }
            else
              { }
          );
      localDom0 = checkDom0Keys "" [
        "extraModules"
        "host"
      ] (expectDom0Attrs "" (localConfig.dom0 or { }));
      localDom0Host = checkDom0Host (localDom0.host or { });
      localDom0HostModule =
        { ... }:
        {
          nixpkgs.overlays = localDom0Host.overlays or [ ];
          environment.systemPackages = localDom0Host.packages or [ ];
          environment.etc = localDom0Host.etc or { };
        }
        // (
          if localDom0Host ? systemd then
            {
              systemd = localDom0Host.systemd;
            }
          else
            { }
        );
      hasNonEmptyAttrs = value: builtins.typeOf value == "set" && builtins.attrNames value != [ ];
      hasNonEmptyList = value: builtins.typeOf value == "list" && value != [ ];
      localDom0HostApply = checkDom0HostApply (localDom0Host.apply or { });
      localDom0HostHasUnknownLiveSafety =
        hasNonEmptyList (localDom0Host.overlays or [ ])
        || hasNonEmptyAttrs (localDom0Host.configuration or { })
        || hasNonEmptyList (localDom0Host.modules or [ ])
        || hasNonEmptyList (localDom0.extraModules or [ ]);
      autoDom0SystemSwitchAction =
        if localDom0HostApply.mode == "boot" then
          "boot"
        else if localDom0HostApply.mode == "switch" then
          "switch"
        else if localDom0HostHasUnknownLiveSafety then
          "boot"
        else
          "switch";
      autoDom0SystemReason =
        if localDom0HostApply ? reason then
          localDom0HostApply.reason
        else if autoDom0SystemSwitchAction == "boot" then
          "dom0 host config includes arbitrary NixOS modules or overlays"
        else
          "dom0 host userland config can be applied live";
      automaticSystemArgs = {
        dom0SystemSwitchAction = autoDom0SystemSwitchAction;
        dom0SystemReason = autoDom0SystemReason;
      };
      localDom0HostModules = [
        localDom0HostModule
      ]
      ++ (if localDom0Host ? configuration then [ localDom0Host.configuration ] else [ ])
      ++ (localDom0Host.modules or [ ])
      ++ (localDom0.extraModules or [ ]);
      prebuiltTemplateSources = import ./airlock/prebuilt-template-sources.nix;
      airlockSupport = "__AIRLOCK_SUPPORT_PATH__";
      nixosQubesSource = "__AIRLOCK_NIXOS_QUBES_PATH__";
      nixpkgsLibvirt105Source = "__NIXPKGS_LIBVIRT_10_5_PATH__";
      templateBuilders = import "${airlockSupport}/lifecycle/misc/lib/qube-template.nix" {
        inherit nixpkgs system;
        inherit
          airlockSupport
          nixosQubesSource
          nixpkgsLibvirt105Source
          ;
      };
      mergeDomainAttrs =
        base: override:
        base
        // override
        // {
          prefs = (base.prefs or { }) // (override.prefs or { });
          features = (base.features or { }) // (override.features or { });
          services = (base.services or { }) // (override.services or { });
          state = (base.state or { }) // (override.state or { });
        };
      mkNixNativeEntry =
        name: definition:
        templateBuilders.mkNixNativeDomain {
          inherit name;
          guestRole = definition.role;
          modules = [
            (mkGuestContentModule name definition)
          ]
          ++ guestExtraModules definition;
          additionalRootSpace = definition.additionalRootSpace or null;
          rootImageSizeMiB = definition.rootImageSizeMiB or 512;
          kernelName = definition.kernel or airlockQubeKernelName;
          label = definition.label or "green";
          netvm = definition.netvm or null;
          prefs = definition.prefs or { };
          features = definition.features or { };
          services = definition.services or { };
          devices = definition.devices or { };
          state = definition.state or { };
        };
      userNixNativeDomainEntries = builtins.mapAttrs mkNixNativeEntry checkedNixNative.domains;
      configNixPrivateSizeBytes = 8589934592;
      configRoleDefaults = {
        sys-config-vault = {
          guestHostName = "sys-config-vault";
          label = "black";
          netvm = null;
          rootImageSizeMiB = 128;
          prefs = {
            autostart = false;
            include_in_backups = true;
            memory = 256;
            maxmem = 512;
            vcpus = 1;
          };
          features = {
            "airlock-managed" = "1";
            "airlock-role" = "sys-config-vault";
            "airlock-config-authority" = "1";
          };
          services = {
            "network-manager" = false;
          };
        };
        sys-config-staging = {
          guestHostName = "sys-config-staging";
          label = "gray";
          netvm = null;
          rootImageSizeMiB = 128;
          state.persistentSize = configNixPrivateSizeBytes;
          prefs = {
            autostart = false;
            include_in_backups = true;
            memory = 512;
            maxmem = 1024;
            vcpus = 2;
          };
          features = {
            "airlock-managed" = "1";
            "airlock-role" = "sys-config-staging";
            "airlock-config-staging" = "1";
            "airlock-config-applied-message" = appliedMessage;
          };
          services = {
            "network-manager" = false;
          };
        };
        sys-config-validator = {
          guestHostName = "sys-config-validator";
          label = "blue";
          netvm = null;
          rootImageSizeMiB = 128;
          state.persistentSize = configNixPrivateSizeBytes;
          prefs = {
            autostart = false;
            include_in_backups = true;
            # Keep enough room for full candidate NixOS evaluation after apply.
            memory = 2048;
            maxmem = 4096;
            vcpus = 2;
          };
          features = {
            "airlock-managed" = "1";
            "airlock-role" = "sys-config-validator";
            "airlock-config-validator" = "1";
          };
          services = {
            "network-manager" = false;
          };
        };
      };
      mkConfigRoleEntry =
        role:
        let
          defaults = configRoleDefaults.${role};
          guestDefinition = checkedConfigVms.guests.${role} or { };
          runtime = mergeDomainAttrs {
            inherit (defaults)
              features
              label
              netvm
              prefs
              services
              ;
            state = defaults.state or { };
          } (checkedConfigVms.domains.${role} or { });
        in
        templateBuilders.mkNixNativeDomain {
          name = role;
          guestRole = role;
          inherit
            airlockConfigInputs
            ;
          airlockPrebuiltTemplateSources = prebuiltTemplateSources;
          modules = [
            (mkGuestContentModule defaults.guestHostName guestDefinition)
          ]
          ++ guestExtraModules guestDefinition;
          rootImageSizeMiB = defaults.rootImageSizeMiB;
          kernelName = airlockQubeKernelName;
          label = runtime.label;
          netvm = runtime.netvm;
          prefs = runtime.prefs;
          features = runtime.features;
          services = runtime.services;
          state = runtime.state;
        };
      configRoleNameConflicts = builtins.filter (
        role:
        builtins.hasAttr role checkedNixNative.domains
        || builtins.hasAttr role localDomains
        || builtins.hasAttr role (localTopology.extraDomains or { })
      ) configVmRoles;
      configDomainEntries = builtins.listToAttrs (
        map (role: {
          name = role;
          value = mkConfigRoleEntry role;
        }) configVmRoles
      );
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
      configNixTopologyDomains = builtins.mapAttrs (_name: entry: entry.domain) configDomainEntries;
      materializedConfigDomains = builtins.mapAttrs (
        _name: entry: materializeNixClosureDomain entry
      ) configDomainEntries;
      allNixNativeDomainEntries = configDomainEntries // userNixNativeDomainEntries;
      nixNativeTopologyDomains = builtins.mapAttrs (
        _name: entry: entry.domain
      ) userNixNativeDomainEntries;
      nixNativePackages = builtins.listToAttrs (
        nixpkgs.lib.concatLists (
          map (name: [
            {
              name = allNixNativeDomainEntries.${name}.rootImageName;
              value = allNixNativeDomainEntries.${name}.rootImage;
            }
            {
              name = allNixNativeDomainEntries.${name}.storeArtifactName;
              value = allNixNativeDomainEntries.${name}.storeArtifact;
            }
          ]) (builtins.attrNames allNixNativeDomainEntries)
        )
      );
      nixNativeConfigurations = builtins.listToAttrs (
        map (name: {
          name = allNixNativeDomainEntries.${name}.configurationName;
          value = allNixNativeDomainEntries.${name}.configuration;
        }) (builtins.attrNames allNixNativeDomainEntries)
      );

      appliedMessage = "initial";
      config =
        if configRoleNameConflicts != [ ] then
          throw "config roles are built-in nix-closure domains and cannot be redeclared elsewhere: ${nixpkgs.lib.concatStringsSep ", " configRoleNameConflicts}"
        else
          configVms {
            inherit system;
            configDomains = configNixTopologyDomains;
            extraTemplates = localTopology.extraTemplates or { };
            extraKernels = localTopology.extraKernels or { };
            extraPrefs = localTopology.extraPrefs or { };
            extraDomains = nixNativeTopologyDomains // localDomains // (localTopology.extraDomains or { });
            extraQrexecRules = localTopology.extraQrexecRules or [ ];
          };
      topology = topologySchema.mkTopology config;
      runtimeSystemConfig = systemConfig (
        automaticSystemArgs
        // {
          inherit appliedMessage system;
        }
        // (localConfig.system.args or { })
      );
      runtimeSystem = systemSchema.mkSystem runtimeSystemConfig;
      pkgs = templateBuilders.pkgs;
      allowUnfreePredicate =
        pkg:
        builtins.elem (pkg.pname or (nixpkgs.lib.getName pkg)) [
          "qubes-artwork"
          "qubes-vmm-stubdom-linux"
        ];
      airlockQubeKernelName = "6.12.64-1.fc41";
      airlockQubeKernel = pkgs.repackRpmKernel {
        kernelSrc = pkgs.fetchurl {
          url = "https://ftp.qubes-os.org/repo/yum/r4.3/current/dom0/fc41/rpm/kernel-qubes-vm-6.12.64-1.qubes.fc41.x86_64.rpm";
          hash = "sha256-nK0KbnT8hUwIZPhUgW8nSRNAbZDgjexHWfeeYDiFxsU=";
        };
        modulesSrc = pkgs.fetchurl {
          url = "https://ftp.qubes-os.org/repo/yum/r4.3/current/dom0/fc41/rpm/kernel-modules-6.12.64-1.qubes.fc41.x86_64.rpm";
          hash = "sha256-JBPQM27u/nQwKnfGGDLpQ35BhRwSpy+vcZLUlI06ELo=";
        };
        kernelVersion = "6.12.64-1";
        fedoraVersion = "fc41";
      };
      airlockConfigInputs = {
        nixpkgs = "${nixpkgs}";
        nixpkgsLibvirt105 = nixpkgsLibvirt105Source;
        nixosQubes = nixosQubesSource;
        inherit airlockSupport;
      };
      mkGuestContentModule =
        defaultHostName: definition:
        let
          hasCamelHostName = definition ? hostName;
          hasLowerHostName = definition ? hostname;
          hostName =
            if hasCamelHostName then
              definition.hostName
            else if hasLowerHostName then
              definition.hostname
            else
              defaultHostName;
        in
        { ... }:
        {
          networking.hostName = nixpkgs.lib.mkOverride 900 hostName;
          environment.systemPackages = definition.packages or [ ];
          environment.etc = definition.etc or { };
        }
        // (
          if definition ? systemd then
            {
              systemd = definition.systemd;
            }
          else
            { }
        );
      guestExtraModules =
        definition:
        (if definition ? configuration then [ definition.configuration ] else [ ])
        ++ (definition.modules or [ ]);
      airlockDom0System = nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = {
          inherit
            appliedMessage
            airlockConfigInputs
            ;
          airlockQubeKernel = {
            name = airlockQubeKernelName;
            package = airlockQubeKernel;
          };
          airlockConfigDomains = materializedConfigDomains;
          airlockPrebuiltTemplateSources = prebuiltTemplateSources;
        };
        modules = [
          "${nixosQubesSource}/nixos/modules/virtualisation/qubes"
          "${airlockSupport}/lifecycle/misc/modules/dom0.nix"
          "${airlockSupport}/lifecycle/misc/modules/qubes-topology.nix"
          "${airlockSupport}/lifecycle/image/profiles/vm-gui.nix"
          "${airlockSupport}/lifecycle/image/profiles/dev-mode.nix"
          "${airlockSupport}/lifecycle/bootstrap/config-system.nix"
          {
            nixpkgs.overlays = templateBuilders.qubesOverlays;
          }
          ./airlock/dom0-host.nix
        ]
        ++ localDom0HostModules;
      };
      dom0Profile =
        let
          pkgs = templateBuilders.pkgs;
        in
        pkgs.runCommand "airlock-dom0-profile"
          {
            nativeBuildInputs = [ pkgs.jq ];
            preferLocalBuild = true;
            allowSubstitutes = false;
            appliedMessage = runtimeSystem.dom0.profile.message;
          }
          ''
            mkdir -p "$out/bin" "$out/share/airlock"
            jq -c -n \
              --arg schema "airlock.dom0-profile.v1" \
              --arg message "$appliedMessage" \
              --arg built_by "sys-config-staging" \
              '{
                schema: $schema,
                message: $message,
                built_by: $built_by
              }' > "$out/share/airlock/profile.json"
            cat > "$out/bin/airlock-dom0-profile-activate" <<EOF
            #!${pkgs.runtimeShell}
            set -eu

            state_dir="''${AIRLOCK_SYSTEM_STATE_DIR:-/var/lib/airlock/system}"
            etc_dir="''${AIRLOCK_SYSTEM_ETC_DIR:-/etc/airlock}"

            ${pkgs.coreutils}/bin/install -d -m 0755 "\$state_dir" "\$etc_dir"
            ${pkgs.coreutils}/bin/cp "$out/share/airlock/profile.json" "\$state_dir/current-profile.json"
            ${pkgs.coreutils}/bin/cp "$out/share/airlock/profile.json" "\$etc_dir/system-profile.json"

            if [ -n "''${AIRLOCK_SYSTEM_JSON:-}" ]; then
              ${pkgs.coreutils}/bin/cp "\$AIRLOCK_SYSTEM_JSON" "\$state_dir/current-system.json"
            fi
            if [ -n "''${AIRLOCK_SYSTEM_PLAN_JSON:-}" ]; then
              ${pkgs.coreutils}/bin/cp "\$AIRLOCK_SYSTEM_PLAN_JSON" "\$state_dir/current-plan.json"
            fi
            if [ -n "''${AIRLOCK_SYSTEM_PROFILE:-}" ]; then
              printf '%s\n' "\$AIRLOCK_SYSTEM_PROFILE" > "\$state_dir/current-profile-link"
            fi
            EOF
            chmod 0555 "$out/bin/airlock-dom0-profile-activate"
          '';
    in
    {
      packages.${system} = nixNativePackages // {
        airlock-dom0-profile = dom0Profile;
      };

      checks.${system} = {
        topology =
          templateBuilders.pkgs.runCommand "airlock-config-topology-check"
            {
              topology = builtins.toJSON topology;
            }
            ''
              printf '%s\n' "$topology" > "$out"
            '';
        system =
          templateBuilders.pkgs.runCommand "airlock-config-system-check"
            {
              system = builtins.toJSON runtimeSystem;
            }
            ''
              printf '%s\n' "$system" > "$out"
            '';
      };

      airlock = {
        metadata = {
          schema = "airlock.config-repo.v1";
          purpose = "candidate";
          topology_schema = topologySchema.schema;
          system_schema = systemSchema.schema;
        };

        inherit config;
        configRoleConfigurations = {
          sys-config-vault = configDomainEntries.sys-config-vault.configuration;
          sys-config-staging = configDomainEntries.sys-config-staging.configuration;
          sys-config-validator = configDomainEntries.sys-config-validator.configuration;
        };
        nixNativeConfigurations = nixNativeConfigurations;
        system = runtimeSystem;
        inherit topology;
      };

      nixosConfigurations = nixNativeConfigurations // {
        airlock-dom0 = airlockDom0System;
      };
    };
}
