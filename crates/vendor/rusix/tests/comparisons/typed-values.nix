{ lib, ... }: {
  options.demo = {
    port = lib.mkOption { type = lib.types.port; };
    userId = lib.mkOption { type = lib.types.ints.u16; };
    host = lib.mkOption { type = lib.types.str; };
    owner = lib.mkOption { type = lib.types.str; };
  };
  config.demo = { port = 1000; userId = 1000; host = "admin"; owner = "admin"; };
}
