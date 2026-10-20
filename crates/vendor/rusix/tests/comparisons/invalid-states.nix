{ lib, config, ... }: {
  options.demo = {
    transport = lib.mkOption {
      type = lib.types.submodule {
        options = {
          tls = lib.mkOption { type = lib.types.bool; };
          certificate = lib.mkOption { type = lib.types.nullOr lib.types.str; default = null; };
          privateKey = lib.mkOption { type = lib.types.nullOr lib.types.str; default = null; };
        };
      };
    };
  };
  config = {
    demo = {
      transport = { tls = true; certificate = "/run/keys/service.pem"; privateKey = "/run/keys/service.key"; };
    };
    assertions = [{
      assertion = config.demo.transport.tls == (config.demo.transport.certificate != null)
        && config.demo.transport.tls == (config.demo.transport.privateKey != null);
      message = "certificate and private key presence must match TLS";
    }];
  };
}
