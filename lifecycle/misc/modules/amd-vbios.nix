{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.airlock.amdVbios;
  package = pkgs.python3.pkgs.callPackage ../../runtime/tools/pkgs/airlock-amd-vbios { };
in
{
  options.airlock.amdVbios = {
    enable = lib.mkEnableOption "automatic AMD VFCT delivery through Qubes startup";
    domains = lib.mkOption {
      default = { };
      description = "Opted-in HVM domains; PCI assignment remains managed by Qubes.";
      type = lib.types.attrsOf (
        lib.types.submodule {
          options.guestSlot = lib.mkOption {
            type = lib.types.ints.between 8 31;
            default = 16;
            description = "Explicit guest bus-zero PCI slot, shared by libvirt and VFCT.";
          };
        }
      );
    };
    package = lib.mkOption {
      type = lib.types.package;
      readOnly = true;
      default = package;
      description = "Qubes extension and offline firmware validator.";
    };
  };
  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = config.virtualisation.qubes.dom0.enable;
        message = "AMD VFCT preparation requires Qubes dom0.";
      }
      {
        assertion =
          cfg.domains != { }
          && lib.all (name: name != "dom0" && builtins.match "[a-zA-Z][a-zA-Z0-9_-]*" name != null) (
            builtins.attrNames cfg.domains
          );
        message = "AMD VFCT requires explicit, valid guest domain names.";
      }
    ];
    environment.systemPackages = [ package ];
    systemd.tmpfiles.rules = [ "d /run/airlock-amd-vbios 0700 root root - -" ];
    environment.etc = {
      "airlock/amd-vbios.json".text = builtins.toJSON { inherit (cfg) domains; };
    }
    // lib.mapAttrs' (
      name: _:
      lib.nameValuePair "qubes/templates/libvirt/xen/by-name/${name}.xml" {
        source = ../../image/templates/amd-vbios.xml;
      }
    ) cfg.domains;
  };
}
