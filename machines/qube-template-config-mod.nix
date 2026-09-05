{ lib, ... }:
{
  imports = [
    ./qube-template-base.nix
  ];

  nixome.qubeGuest.role = "config-mod";
  networking.hostName = "nixome-config-mod-template";
}
