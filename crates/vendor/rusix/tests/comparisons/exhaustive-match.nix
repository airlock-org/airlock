{ lib, config, ... }:
let
  firewallPolicy = mode: if mode == "server" then [ 443 ] else [];
  servicePolicy = mode: mode == "server";
in {
  options.demo = {
    mode = lib.mkOption { type = lib.types.enum [ "server" "client" ]; };
    firewall.allowedPorts = lib.mkOption { type = lib.types.listOf lib.types.port; };
    service.acceptsConnections = lib.mkOption { type = lib.types.bool; };
  };
  config.demo = {
    mode = "client";
    firewall.allowedPorts = firewallPolicy config.demo.mode;
    service.acceptsConnections = servicePolicy config.demo.mode;
  };
}
