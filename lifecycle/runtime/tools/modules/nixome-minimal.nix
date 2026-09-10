{
  config,
  lib,
  modulesPath,
  pkgs,
  ...
}:
let
  cfg = config.nixome.minimalGuest;
  packageTrim = lib.mkOverride 50;
  defaultOff = packageTrim false;
in
{
  imports = [
    "${modulesPath}/profiles/minimal.nix"
  ];

  options.nixome.minimalGuest = {
    enable = lib.mkEnableOption "the Nixome minimal closure-backed guest base";

    nix.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether this guest role carries explicit Nix CLI tooling.";
    };
  };

  config = lib.mkIf cfg.enable {
    # Keep the normal NixOS/Qubes guest runtime. This module only trims
    # convenience packages and documentation; it must not replace systemd/dbus,
    # QubesDB, qrexec, or other runtime services with bespoke variants.
    environment.defaultPackages = packageTrim [ ];

    system.etc.overlay.enable = lib.mkDefault true;
    services.userborn.enable = lib.mkDefault true;

    documentation.enable = lib.mkDefault false;
    documentation.doc.enable = lib.mkDefault false;
    documentation.info.enable = lib.mkDefault false;
    documentation.man.enable = lib.mkDefault false;
    documentation.nixos.enable = lib.mkDefault false;

    programs.command-not-found.enable = defaultOff;
    programs.bash.completion.enable = defaultOff;
    programs.less.enable = defaultOff;
    programs.nano.enable = defaultOff;

    environment.systemPackages = lib.mkIf cfg.nix.enable [
      pkgs.nix
    ];

    system.disableInstallerTools = packageTrim true;
  };
}
