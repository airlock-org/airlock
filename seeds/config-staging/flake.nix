{
  description = "Nixome machine configuration candidate";

  inputs.nixpkgs.url = "__NIXOME_NIXPKGS_URL__";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      topologySchema = import ./nixome/topology.nix;
      systemSchema = import ./nixome/system-schema.nix;
      configVms = import ./nixome/config-vms.nix;
      systemConfig = import ./nixome/system.nix;
      expectMachineAttrs =
        path: value:
        let
          prefix = "nixome.machine${if path == "" then "" else ".${path}"}";
        in
        if builtins.typeOf value == "set" then
          value
        else
          throw "${prefix}: expected an attribute set, got ${builtins.typeOf value}";
      checkMachineKeys =
        path: allowed: attrs:
        let
          unknown = builtins.filter (name: !(builtins.elem name allowed)) (builtins.attrNames attrs);
          prefix = "nixome.machine${if path == "" then "" else ".${path}"}";
        in
        if unknown == [ ] then
          attrs
        else
          throw "${prefix}: unknown option(s): ${nixpkgs.lib.concatStringsSep ", " unknown}";
      rawLocalConfig = import ./nixome/machine.nix {
        inherit
          appliedMessage
          system
          templateBuilders
          templateSet
          ;
        pkgs = templateBuilders.pkgs;
      };
      localConfig = checkMachineKeys "" [
        "configVms"
        "domains"
        "dom0"
        "nixNative"
        "system"
        "templates"
        "topology"
      ] (expectMachineAttrs "" rawLocalConfig);
      localTopology = localConfig.topology or { };
      localDomains = localConfig.domains or { };
      localConfigVms = localConfig.configVms or { };
      configVmRoles = [
        "config-vault"
        "config-staging"
        "config-validator"
      ];
      checkConfigKeys =
        path: allowed: attrs:
        let
          unknown = builtins.filter (name: !(builtins.elem name allowed)) (builtins.attrNames attrs);
          prefix = "nixome.configVms${if path == "" then "" else ".${path}"}";
        in
        if unknown == [ ] then
          attrs
        else
          throw "${prefix}: unknown option(s): ${nixpkgs.lib.concatStringsSep ", " unknown}";
      expectConfigAttrs =
        path: value:
        let
          prefix = "nixome.configVms${if path == "" then "" else ".${path}"}";
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
          "template"
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
          prefix = "nixome.nixNative${if path == "" then "" else ".${path}"}";
        in
        if unknown == [ ] then
          attrs
        else
          throw "${prefix}: unknown option(s): ${nixpkgs.lib.concatStringsSep ", " unknown}";
      expectNixNativeAttrs =
        path: value:
        let
          prefix = "nixome.nixNative${if path == "" then "" else ".${path}"}";
        in
        if builtins.typeOf value == "set" then
          value
        else
          throw "${prefix}: expected an attribute set, got ${builtins.typeOf value}";
      checkNixNativeDomain =
        path: value:
        checkNixNativeKeys path [
          "additionalRootSpace"
          "additionalStoreSpaceMiB"
          "configuration"
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
          "services"
          "systemd"
          "rootImageSizeMiB"
        ] (expectNixNativeAttrs path value);
      checkedNixNative =
        let
          checked = checkNixNativeKeys "" [ "domains" ] (expectNixNativeAttrs "" (localConfig.nixNative or { }));
        in
        checked
        // {
          domains = builtins.mapAttrs (
            name: value: checkNixNativeDomain "domains.${name}" value
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
          throw "nixome.configVms.${path}: unknown role(s): ${nixpkgs.lib.concatStringsSep ", " unknown}";
      checkedConfigVms =
        let
          checked = checkConfigKeys "" [
            "domains"
            "guests"
            "sharedGuest"
            "templateMode"
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
          sharedGuest = checkConfigGuest "sharedGuest" (checked.sharedGuest or { });
        };
      expectDom0Attrs =
        path: value:
        let
          prefix = "nixome.dom0${if path == "" then "" else ".${path}"}";
        in
        if builtins.typeOf value == "set" then
          value
        else
          throw "${prefix}: expected an attribute set, got ${builtins.typeOf value}";
      checkDom0Keys =
        path: allowed: attrs:
        let
          unknown = builtins.filter (name: !(builtins.elem name allowed)) (builtins.attrNames attrs);
          prefix = "nixome.dom0${if path == "" then "" else ".${path}"}";
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
          throw "nixome.dom0.host.apply.mode: expected one of auto, switch, boot"
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
                    throw "nixome.dom0.host.apply.reason: expected a string or path, got ${builtins.typeOf apply.reason}";
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
      prebuiltTemplateSources = import ./nixome/prebuilt-template-sources.nix;
      nixomeSupport = "__NIXOME_SUPPORT_PATH__";
      nixosQubesSource = "__NIXOME_NIXOS_QUBES_PATH__";
      nixpkgsLibvirt105Source = "__NIXPKGS_LIBVIRT_10_5_PATH__";
      templateBuilders = import "${nixomeSupport}/lib/nixome/qube-template.nix" {
        inherit nixpkgs system;
        inherit
          nixomeSupport
          nixosQubesSource
          nixpkgsLibvirt105Source
          ;
      };
      templateSet = import ./nixome/templates.nix {
        extraTemplateDefinitions = localConfig.templates.definitions or { };
        inherit
          prebuiltTemplateSources
          system
          templateBuilders
          ;
      };
      nixNativeDomainEntries = builtins.mapAttrs (
        name: definition:
        templateBuilders.mkNixNativeDomain {
          inherit name;
          modules = [
            (mkGuestContentModule name definition)
          ]
          ++ guestExtraModules definition;
          additionalRootSpace = definition.additionalRootSpace or null;
          additionalStoreSpaceMiB = definition.additionalStoreSpaceMiB or 256;
          rootImageSizeMiB = definition.rootImageSizeMiB or 512;
          kernelName = definition.kernel or nixomeQubeKernelName;
          label = definition.label or "green";
          netvm = definition.netvm or null;
          prefs = definition.prefs or { };
          features = definition.features or { };
          services = definition.services or { };
        }
      ) checkedNixNative.domains;
      nixNativeTopologyDomains = builtins.mapAttrs (_name: entry: entry.domain) nixNativeDomainEntries;
      nixNativePackages = builtins.listToAttrs (
        nixpkgs.lib.concatLists (
          map (name: [
            {
              name = nixNativeDomainEntries.${name}.rootImageName;
              value = nixNativeDomainEntries.${name}.rootImage;
            }
            {
              name = nixNativeDomainEntries.${name}.storeImageName;
              value = nixNativeDomainEntries.${name}.storeImage;
            }
          ]) (builtins.attrNames nixNativeDomainEntries)
        )
      );
      nixNativeConfigurations = builtins.listToAttrs (
        map (name: {
          name = nixNativeDomainEntries.${name}.configurationName;
          value = nixNativeDomainEntries.${name}.configuration;
        }) (builtins.attrNames nixNativeDomainEntries)
      );

      appliedMessage = "initial";
      config = configVms {
        inherit appliedMessage system;
        configVms = checkedConfigVms;
        configRoleKernelOpts = {
          config-vault = templateBuilders.mkQubeKernelOpts configVaultGuest;
          config-staging = templateBuilders.mkQubeKernelOpts configStagingGuest;
          config-validator = templateBuilders.mkQubeKernelOpts configValidatorGuest;
        };
        extraTemplates = templateSet.topology.templates // (localTopology.extraTemplates or { });
        extraKernels = localTopology.extraKernels or { };
        extraPrefs = localTopology.extraPrefs or { };
        extraDomains =
          templateSet.topology.domains
          // nixNativeTopologyDomains
          // localDomains
          // (localTopology.extraDomains or { });
        extraQrexecRules = localTopology.extraQrexecRules or [ ];
      };
      topology = topologySchema.mkTopology config;
      machineSystemConfig = systemConfig (
        automaticSystemArgs
        // {
          inherit appliedMessage system;
        }
        // (localConfig.system.args or { })
      );
      machineSystem = systemSchema.mkSystem machineSystemConfig;
      pkgs = templateBuilders.pkgs;
      allowUnfreePredicate =
        pkg:
        builtins.elem (pkg.pname or (nixpkgs.lib.getName pkg)) [
          "qubes-artwork"
          "qubes-vmm-stubdom-linux"
        ];
      nixomeQubeKernelName = "6.12.64-1.fc41";
      nixomeQubeKernel = pkgs.repackRpmKernel {
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
      nixomeConfigSeedInputs = {
        nixpkgs = "${nixpkgs}";
        nixpkgsLibvirt105 = nixpkgsLibvirt105Source;
        nixosQubes = nixosQubesSource;
        inherit nixomeSupport;
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
      mkConfigGuest =
        role: defaultHostName:
        let
          definition = (checkedConfigVms.guests or { }).${role} or { };
        in
        nixpkgs.lib.nixosSystem {
          inherit system;
          specialArgs = {
            inherit
              nixomeConfigSeedInputs
              ;
            nixomePrebuiltTemplateSources = prebuiltTemplateSources;
          };
          modules = [
            "${nixpkgs}/nixos/modules/profiles/minimal.nix"
            "${nixosQubesSource}/nixos/modules/virtualisation/qubes"
            "${nixomeSupport}/modules/qube-guest.nix"
            {
              nixpkgs.overlays = templateBuilders.qubesOverlays;
              nixpkgs.config.allowUnfreePredicate = allowUnfreePredicate;
              nixome.qubeGuest.enable = true;
              nixome.qubeGuest.role = role;
              documentation.enable = nixpkgs.lib.mkDefault false;
              programs.command-not-found.enable = nixpkgs.lib.mkDefault false;
              system.nixos.versionSuffix = nixpkgs.lib.mkForce "";
            }
            (mkGuestContentModule defaultHostName definition)
          ]
          ++ guestExtraModules definition;
        };
      configSystemGuest =
        let
          definition = checkedConfigVms.sharedGuest or { };
        in
        nixpkgs.lib.nixosSystem {
          inherit system;
          specialArgs = {
            inherit
              nixomeConfigSeedInputs
              ;
            nixomePrebuiltTemplateSources = prebuiltTemplateSources;
            nixomePrebuiltTemplatePackages = {
              work = templateSet.packages.nixome-nixos-work-template;
            };
          };
          modules = [
            "${nixpkgs}/nixos/modules/profiles/minimal.nix"
            "${nixosQubesSource}/nixos/modules/virtualisation/qubes"
            "${nixomeSupport}/modules/qube-guest.nix"
            {
              nixpkgs.overlays = templateBuilders.qubesOverlays;
              nixpkgs.config.allowUnfreePredicate = allowUnfreePredicate;
              nixome.qubeGuest.enable = true;
              nixome.qubeGuest.role = "config-system";
              documentation.enable = nixpkgs.lib.mkDefault false;
              programs.command-not-found.enable = nixpkgs.lib.mkDefault false;
              system.nixos.versionSuffix = nixpkgs.lib.mkForce "";
            }
            (mkGuestContentModule "nixome-config-system-template" definition)
          ]
          ++ guestExtraModules definition;
        };
      configVaultGuest = mkConfigGuest "config-vault" "nixome-config-vault-template";
      configStagingGuest = mkConfigGuest "config-staging" "nixome-config-staging-template";
      configValidatorGuest = mkConfigGuest "config-validator" "nixome-config-validator-template";
      configSystemRootImage = templateBuilders.mkQubeRootImage {
        configuration = configSystemGuest;
        imageName = "nixome-nixos-config-system";
      };
      configVaultRootImage = templateBuilders.mkQubeRootImage {
        configuration = configVaultGuest;
        imageName = "nixome-nixos-config-vault";
      };
      configStagingRootImage = templateBuilders.mkQubeRootImage {
        configuration = configStagingGuest;
        imageName = "nixome-nixos-config-staging";
      };
      configValidatorRootImage = templateBuilders.mkQubeRootImage {
        configuration = configValidatorGuest;
        imageName = "nixome-nixos-config-validator";
      };
      configSystemTemplate = templateBuilders.mkQubesTemplatePackage {
        templateName = "nixome-nixos-config-system";
        rootImageFile = "${configSystemRootImage}/root.img";
      };
      configVaultTemplate = templateBuilders.mkQubesTemplatePackage {
        templateName = "nixome-nixos-config-vault";
        rootImageFile = "${configVaultRootImage}/root.img";
      };
      configStagingTemplate = templateBuilders.mkQubesTemplatePackage {
        templateName = "nixome-nixos-config-staging";
        rootImageFile = "${configStagingRootImage}/root.img";
      };
      configValidatorTemplate = templateBuilders.mkQubesTemplatePackage {
        templateName = "nixome-nixos-config-validator";
        rootImageFile = "${configValidatorRootImage}/root.img";
      };
      nixomeDom0System = nixpkgs.lib.nixosSystem {
        inherit system;
        specialArgs = {
          inherit
            appliedMessage
            nixomeConfigSeedInputs
            ;
          nixomeQubeKernel = {
            name = nixomeQubeKernelName;
            package = nixomeQubeKernel;
          };
          nixomeQubeKernelOpts = {
            configSystem = templateBuilders.mkQubeKernelOpts configSystemGuest;
          };
          nixomeTemplatePackages = {
          };
          nixomePrebuiltTemplatePackages = { };
          nixomePrebuiltTemplateSources = prebuiltTemplateSources;
          nixomeConfigSystemTemplateImport = false;
        };
        modules = [
          "${nixosQubesSource}/nixos/modules/virtualisation/qubes"
          "${nixomeSupport}/modules/dom0.nix"
          "${nixomeSupport}/modules/qubes-topology.nix"
          "${nixomeSupport}/machines/vm-gui.nix"
          "${nixomeSupport}/machines/config-system.nix"
          {
            nixpkgs.overlays = templateBuilders.qubesOverlays;
          }
          ./nixome/dom0-host.nix
        ]
        ++ localDom0HostModules;
      };
      dom0Profile =
        let
          pkgs = templateBuilders.pkgs;
        in
        pkgs.runCommand "nixome-dom0-profile"
          {
            nativeBuildInputs = [ pkgs.jq ];
            preferLocalBuild = true;
            allowSubstitutes = false;
            appliedMessage = machineSystem.dom0.profile.message;
          }
          ''
            mkdir -p "$out/bin" "$out/share/nixome"
            jq -c -n \
              --arg schema "nixome.dom0-profile.v1" \
              --arg message "$appliedMessage" \
              --arg built_by "config-staging" \
              '{
                schema: $schema,
                message: $message,
                built_by: $built_by
              }' > "$out/share/nixome/profile.json"
            cat > "$out/bin/nixome-dom0-profile-activate" <<EOF
            #!${pkgs.runtimeShell}
            set -eu

            state_dir="''${NIXOME_SYSTEM_STATE_DIR:-/var/lib/nixome/system}"
            etc_dir="''${NIXOME_SYSTEM_ETC_DIR:-/etc/nixome}"

            ${pkgs.coreutils}/bin/install -d -m 0755 "\$state_dir" "\$etc_dir"
            ${pkgs.coreutils}/bin/cp "$out/share/nixome/profile.json" "\$state_dir/current-profile.json"
            ${pkgs.coreutils}/bin/cp "$out/share/nixome/profile.json" "\$etc_dir/system-profile.json"

            if [ -n "''${NIXOME_SYSTEM_JSON:-}" ]; then
              ${pkgs.coreutils}/bin/cp "\$NIXOME_SYSTEM_JSON" "\$state_dir/current-system.json"
            fi
            if [ -n "''${NIXOME_SYSTEM_PLAN_JSON:-}" ]; then
              ${pkgs.coreutils}/bin/cp "\$NIXOME_SYSTEM_PLAN_JSON" "\$state_dir/current-plan.json"
            fi
            if [ -n "''${NIXOME_SYSTEM_PROFILE:-}" ]; then
              printf '%s\n' "\$NIXOME_SYSTEM_PROFILE" > "\$state_dir/current-profile-link"
            fi
            EOF
            chmod 0555 "$out/bin/nixome-dom0-profile-activate"
          '';
    in
    {
      packages.${system} = templateSet.packages // nixNativePackages // {
        nixome-dom0-profile = dom0Profile;
        nixome-nixos-config-vault-template = configVaultTemplate;
        nixome-nixos-config-staging-template = configStagingTemplate;
        nixome-nixos-config-validator-template = configValidatorTemplate;
      };

      checks.${system} = {
        topology =
          templateBuilders.pkgs.runCommand "nixome-config-topology-check"
            {
              topology = builtins.toJSON topology;
            }
            ''
              printf '%s\n' "$topology" > "$out"
            '';
        system =
          templateBuilders.pkgs.runCommand "nixome-config-system-check"
            {
              system = builtins.toJSON machineSystem;
            }
            ''
              printf '%s\n' "$system" > "$out"
            '';
      };

      nixome = {
        metadata = {
          schema = "nixome.config-repo.v1";
          purpose = "candidate";
          topology_schema = topologySchema.schema;
          system_schema = systemSchema.schema;
        };

        inherit config;
        configRoleTemplateConfigurations = {
          config-vault = configVaultGuest;
          config-staging = configStagingGuest;
          config-validator = configValidatorGuest;
        };
        configSystemTemplateConfiguration = configSystemGuest;
        templateConfigurations = templateSet.configurations;
        nixNativeConfigurations = nixNativeConfigurations;
        system = machineSystem;
        inherit topology;
      };

      nixosConfigurations = nixNativeConfigurations // {
        nixome-dom0 = nixomeDom0System;
      };
    };
}
