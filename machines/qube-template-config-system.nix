{ lib, nixomePrebuiltTemplatePackages ? { }, ... }:
{
  imports = [
    ./qube-template-base.nix
  ];

  nixome.qubeGuest.role = "config-system";
  networking.hostName = "nixome-config-system-template";

  environment.systemPackages = lib.attrValues nixomePrebuiltTemplatePackages;
}
