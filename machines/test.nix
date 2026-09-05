{ lib, ... }:
{
  nixome.dom0 = {
    enable = true;
    user = "nixome";
    gui.enable = false;
    observability.enable = true;
  };

  users.users.nixome = {
    isNormalUser = true;
    description = "Nixome test operator";
    extraGroups = [ "wheel" ];
  };

  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = lib.mkDefault false;
  boot.initrd.systemd.enable = true;

  fileSystems."/" = {
    device = "/dev/disk/by-label/nixome-root";
    fsType = "ext4";
  };

  networking.hostName = lib.mkDefault "dom0";

  system.stateVersion = "26.05";
}
