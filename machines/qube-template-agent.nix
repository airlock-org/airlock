{ lib, ... }:
{
  imports = [
    ./qube-template-base.nix
  ];

  nixome.qubeGuest.role = "agent";
  networking.hostName = "nixome-agent-template";
}
