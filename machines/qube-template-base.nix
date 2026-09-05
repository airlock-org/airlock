{ lib, modulesPath, ... }:
{
  imports = [
    "${modulesPath}/profiles/minimal.nix"
    ../modules/qube-guest.nix
  ];

  nixome.qubeGuest.enable = true;

  documentation.enable = lib.mkDefault false;
  programs.command-not-found.enable = lib.mkDefault false;
}
