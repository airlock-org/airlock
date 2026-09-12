{ lib, ... }:
{
  airlock.dom0 = {
    enable = true;
    user = "airlock";
    gui.enable = false;
  };

  users.users.airlock = {
    isNormalUser = true;
    description = "Airlock test operator";
    extraGroups = [ "wheel" ];
  };

  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = lib.mkDefault false;
  boot.initrd.systemd.enable = true;

  fileSystems."/" = {
    device = "/dev/disk/by-label/airlock-root";
    fsType = "ext4";
  };

  networking.hostName = lib.mkDefault "dom0";

  system.stateVersion = "26.05";
}
