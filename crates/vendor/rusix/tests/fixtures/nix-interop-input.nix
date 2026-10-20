# Local input-shaped object. No flake registry, fetch, or package build.
{
  functions.lazy = _: { good = 42; bad = throw "delayed external call failed"; };
  values.lazy = { good = 42; bad = throw "unselected opaque value failed"; };
  packages.example = builtins.derivation {
    name = "rusix-external-1.0";
    pname = "rusix-external";
    version = "1.0";
    system = "x86_64-linux";
    builder = "/never-built-by-rusix";
  };
  nixosModules.example = { lib, ... }: {
    options.services.rusixExternal.enable = lib.mkEnableOption "local example";
  };
  nixosModules.broken = { lib, ... }: {
    options.services.rusixExternal.value = lib.mkOption {
      type = lib.types.int;
      default = throw "local external module failed";
    };
  };
  nixosModules.invalid = 42;
  overlays.example = final: prev: {
    rusixOverlayHello = prev.hello;
  };
}
