{ lib, ... }:
{
  imports = [
    ./qube-template-base.nix
  ];

  nixome.qubeGuest.role = "config-staging";
  networking.hostName = "nixome-config-staging-template";
}
