{ lib, ... }: {
  options.demo.endpoint = lib.mkOption {
    type = lib.types.submodule {
      options = {
        host = lib.mkOption { type = lib.types.str; };
        port = lib.mkOption { type = lib.types.port; };
      };
    };
  };
  config.demo.endpoint = { host = "service.internal"; port = 443; };
}
