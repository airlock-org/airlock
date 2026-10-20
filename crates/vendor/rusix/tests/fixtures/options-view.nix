# Ordinary Nix modules exercise declared references, not generated option schemas.
{
  schema = { lib, ... }: {
    options.services.example = {
      enable = lib.mkEnableOption "the options view fixture";
      port = lib.mkOption { type = lib.types.int; default = 5432; };
      dataDir = lib.mkOption { type = lib.types.str; default = "/base"; };
      payload = lib.mkOption { type = lib.types.attrs; default = { dynamic = 42; }; };
      names = lib.mkOption { type = lib.types.listOf lib.types.str; default = [ "a" "b" ]; };
      optional = lib.mkOption { type = lib.types.nullOr lib.types.str; default = null; };
      values = lib.mkOption { type = lib.types.attrsOf lib.types.int; default = { x = 1; }; };
      hashValues = lib.mkOption { type = lib.types.attrsOf lib.types.int; default = { y = 2; }; };
      settings = lib.mkOption {
        type = lib.types.attrsOf lib.types.anything;
        default = { port = 1234; jit = "off"; arbitrary = 0.5; };
      };
    };
    options.services."literal.node \${key}\"" = lib.mkOption {
      type = lib.types.attrsOf lib.types.bool;
      default = { "with space" = true; OtherFlag = false; };
    };
  };
  ordinary = { services.example.port = 6432; };
  force = { lib, ... }: { services.example.port = lib.mkForce 7432; };
  failing = { services.example.port = throw "unused option view was evaluated"; };
  zero = { services.example.port = 0; };
}
