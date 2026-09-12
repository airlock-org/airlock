{
  lib,
  pkgs,
  ...
}:
{
  imports = [
    ./vm-base.nix
  ];

  airlock.dom0.gui.enable = true;

  services.xserver = {
    enable = true;
    displayManager.lightdm.enable = true;
  };

  services.displayManager = {
    defaultSession = "sys-gui";
    autoLogin = {
      enable = true;
      user = "airlock";
    };
  };

  services.libinput.enable = true;

  environment.systemPackages = with pkgs; [
    xdotool
  ];

  image.baseName = "airlock-dom0-vm-gui";
}
