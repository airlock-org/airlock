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
      localConfig = import ./nixome/machine.nix {
        inherit
          appliedMessage
          system
          templateBuilders
          templateSet
          ;
        pkgs = templateBuilders.pkgs;
      };
      localTopology = localConfig.topology or { };
      localDom0 = localConfig.dom0 or { };
      localDom0Host = localDom0.host or { };
      localDom0HostModule =
        { ... }:
        {
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
      localDom0HostModules =
        [ localDom0HostModule ]
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

      appliedMessage = "initial";
      config = configVms {
        inherit appliedMessage;
        extraTemplates = templateSet.topology.templates // (localTopology.extraTemplates or { });
        extraKernels = localTopology.extraKernels or { };
        extraPrefs = localTopology.extraPrefs or { };
        extraDomains = templateSet.topology.domains // (localTopology.extraDomains or { });
        extraQrexecRules = localTopology.extraQrexecRules or [ ];
      };
      topology = topologySchema.mkTopology config;
      machineSystemConfig = systemConfig (
        {
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
      configSystemGuest = nixpkgs.lib.nixosSystem {
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
            networking.hostName = "nixome-config-system-template";
            documentation.enable = nixpkgs.lib.mkDefault false;
            programs.command-not-found.enable = nixpkgs.lib.mkDefault false;
            system.nixos.versionSuffix = nixpkgs.lib.mkForce "";
          }
        ];
      };
      configSystemRootImage = templateBuilders.mkQubeRootImage {
        configuration = configSystemGuest;
        imageName = "nixome-nixos-config-system";
      };
      configSystemTemplate = templateBuilders.mkQubesTemplatePackage {
        templateName = "nixome-nixos-config-system";
        rootImageFile = "${configSystemRootImage}/root.img";
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
      packages.${system} = templateSet.packages // {
        nixome-dom0-profile = dom0Profile;
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
        system = machineSystem;
        inherit topology;
      };

      nixosConfigurations.nixome-dom0 = nixomeDom0System;
    };
}
