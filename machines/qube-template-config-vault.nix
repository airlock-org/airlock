{ lib, ... }:
{
  imports = [
    ./qube-template-base.nix
  ];

  nixome.qubeGuest.role = "config-vault";
  networking.hostName = "nixome-config-vault-template";
}
