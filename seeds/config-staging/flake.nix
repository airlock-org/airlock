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
      prebuiltTemplateSources = import ./nixome/prebuilt-template-sources.nix;
      nixomeSupport = "__NIXOME_SUPPORT_PATH__";
      nixosQubesSource = "__NIXOME_NIXOS_QUBES_PATH__";
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
        extraTemplates = templateSet.topology.templates;
        extraDomains = templateSet.topology.domains;
      };
      topology = topologySchema.mkTopology config;
      machineSystemConfig = systemConfig { inherit appliedMessage system; };
      machineSystem = systemSchema.mkSystem machineSystemConfig;
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
    };
}
