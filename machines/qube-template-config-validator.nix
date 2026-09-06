{ lib, ... }:
{
  imports = [
    ./qube-template-base.nix
  ];

  nixome.qubeGuest.role = "config-validator";
  networking.hostName = "nixome-config-validator-template";
}
