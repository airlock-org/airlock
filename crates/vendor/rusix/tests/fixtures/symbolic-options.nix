# Ordinary NixOS modules: Rust neither models nor selects their option values.
{
  schema = { lib, ... }: {
    options.services.example = {
      enable = lib.mkEnableOption "the symbolic dependency fixture";
      port = lib.mkOption { type = lib.types.port; };
    };
  };
  ordinary = { services.example.port = 6432; };
  force = { lib, ... }: { services.example.port = lib.mkForce 7432; };
  zero = { services.example.port = 0; };
  failing = { services.example.port = throw "unused port was evaluated"; };
}
