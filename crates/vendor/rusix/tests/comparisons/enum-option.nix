{ lib, config, ... }: {
  options.demo = {
    mode = lib.mkOption { type = lib.types.enum [ "server" "client" ]; };
    acceptsConnections = lib.mkOption { type = lib.types.bool; };
  };
  config.demo = {
    mode = "server";
    acceptsConnections = config.demo.mode == "server";
  };
}
