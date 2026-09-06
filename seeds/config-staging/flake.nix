{
  description = "Nixome machine configuration candidate";

  outputs =
    { self }:
    let
      topologySchema = import ./nixome/topology.nix;

      config = {
        templates = { };
        kernels = { };
        prefs = { };
        domains = {
          config-staging = {
            template = "nixome-nixos-config-staging";
            label = "gray";
            prefs = {
              include_in_backups = true;
            };
            features = {
              "nixome-config-applied-message" = "initial";
            };
          };
        };
        qrexecPolicy.rules = [
          {
            service = "nixome.PolicyProbe";
            source = "dom0";
            target = "config-vault";
            action = "allow";
          }
        ];
      };
    in
    {
      nixome = {
        metadata = {
          schema = "nixome.config-repo.v1";
          purpose = "candidate";
          topology_schema = topologySchema.schema;
        };

        inherit config;
        topology = topologySchema.mkTopology config;
      };
    };
}
