{
  description = "Nixome machine configuration candidate";

  outputs =
    { self }:
    let
      topologySchema = import ./nixome/topology.nix;
      configVms = import ./nixome/config-vms.nix;

      config = configVms { };
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
