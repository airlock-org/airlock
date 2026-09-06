{ lib, ... }:
{
  imports = [
    ./qube-template-base.nix
  ];

  nixome.qubeGuest.role = "config-verify";
  networking.hostName = "nixome-config-verify-template";
}
