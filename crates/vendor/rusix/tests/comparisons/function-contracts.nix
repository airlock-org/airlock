{ lib, ... }:
let
  configureService = { host, port, enableTLS ? true }: {
    endpoint = { inherit host port; };
    transport.tls = enableTLS;
  };
in {
  options.demo = {
    endpoint = lib.mkOption {
      type = lib.types.submodule {
        options = {
          host = lib.mkOption { type = lib.types.str; };
          port = lib.mkOption { type = lib.types.port; };
        };
      };
    };
    transport.tls = lib.mkOption { type = lib.types.bool; };
  };
  config.demo = configureService { host = "service.internal"; port = 8080; enableTLS = false; };
}
