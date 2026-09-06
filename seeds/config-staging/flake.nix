{
  description = "Nixome machine configuration candidate";

  outputs = { self }: {
    nixome = {
      metadata = {
        schema = "nixome.config-repo.v1";
        purpose = "candidate";
      };

      topology = {
        schema = "nixome.qubes-topology.v1";
        templates = { };
        kernels = { };
        prefs = { };
        domains = {
          config-staging = {
            class = "AppVM";
            template = "nixome-nixos-config-staging";
            label = "gray";
            netvm = null;
            prefs = { };
            features = {
              "nixome-config-applied-message" = "initial";
            };
            services = { };
          };
        };
      };
    };
  };
}
