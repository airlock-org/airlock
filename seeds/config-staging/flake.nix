{
  description = "Nixome machine configuration candidate";

  inputs = {
    nixpkgs.url = "__NIXOME_NIXPKGS_URL__";

    nixos-qubes = {
      url = "path:__NIXOME_NIXOS_QUBES_PATH__";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    nixome-support = {
      url = "path:__NIXOME_SUPPORT_PATH__";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.nixos-qubes.follows = "nixos-qubes";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      nixos-qubes,
      nixome-support,
    }:
    let
      system = "x86_64-linux";
      topologySchema = import ./nixome/topology.nix;
      systemSchema = import ./nixome/system-schema.nix;
      configVms = import ./nixome/config-vms.nix;
      systemConfig = import ./nixome/system.nix;
      localConfig = import ./nixome/local.nix {
        inherit
          appliedMessage
          system
          templateSet
          ;
      };
      localTopology = localConfig.topology or { };
      prebuiltTemplateSources = import ./nixome/prebuilt-template-sources.nix;
      nixomeSupport = "${nixome-support}";
      nixosQubesSource = "${nixos-qubes}";
      templateBuilders = import "${nixomeSupport}/lib/nixome/qube-template.nix" {
        inherit nixpkgs system;
        inherit nixomeSupport nixosQubesSource;
      };
      templateSet = import ./nixome/templates.nix {
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
      nixomeQubeKernelName = "6.12.64-1.fc41";
      nixomeConfigSeedInputs = {
        nixpkgs = "${nixpkgs}";
        nixosQubes = "${nixos-qubes}";
        nixomeSupport = "${nixome-support}";
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
            package = nixome-support.packages.${system}.nixome-qubes-vm-kernel;
          };
          nixomeQubeKernelOpts = {
            configSystem = templateBuilders.mkQubeKernelOpts nixome-support.nixosConfigurations.qube-template-config-system;
          };
          nixomeTemplatePackages = {
            configSystem = nixome-support.packages.${system}.nixome-nixos-config-system-template;
          };
          nixomePrebuiltTemplatePackages = {
            work = nixome-support.packages.${system}.nixome-nixos-work-template;
          };
          nixomePrebuiltTemplateSources = {
            work = "${nixome-support.packages.${system}.nixome-nixos-work-template}/nixome-nixos-work";
            nixome-nixos-work =
              "${nixome-support.packages.${system}.nixome-nixos-work-template}/nixome-nixos-work";
          };
        };
        modules = [
          nixome-support.nixosModules.dom0
          "${nixomeSupport}/machines/vm-gui.nix"
          "${nixomeSupport}/machines/config-system.nix"
          ./nixome/dom0-host.nix
        ];
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
