{
  description = "Nixome machine configuration candidate";

  inputs.nixpkgs.url = "__NIXOME_NIXPKGS_URL__";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      topologySchema = import ./nixome/topology.nix;
      configVms = import ./nixome/config-vms.nix;
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

      config = configVms {
        extraTemplates = templateSet.topology.templates;
        extraDomains = templateSet.topology.domains;
      };
      topology = topologySchema.mkTopology config;
    in
    {
      packages.${system} = templateSet.packages;

      checks.${system}.topology =
        templateBuilders.pkgs.runCommand "nixome-config-topology-check"
          {
            topology = builtins.toJSON topology;
          }
          ''
            printf '%s\n' "$topology" > "$out"
          '';

      nixome = {
        metadata = {
          schema = "nixome.config-repo.v1";
          purpose = "candidate";
          topology_schema = topologySchema.schema;
        };

        inherit config;
        inherit topology;
      };
    };
}
