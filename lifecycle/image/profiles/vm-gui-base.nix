{
  lib,
  pkgs,
  ...
}:
{
  imports = [
    ./vm-base.nix
  ];

  nixome.dom0.gui.enable = true;

  services.xserver = {
    enable = true;
    desktopManager.xfce.enable = true;
    displayManager.lightdm.enable = true;
  };

  services.displayManager = {
    defaultSession = "xfce";
    autoLogin = {
      enable = true;
      user = "nixome";
    };
  };

  services.libinput.enable = true;

  environment.systemPackages = with pkgs; [
    xterm
    xdotool
  ];

  image.baseName = "nixome-dom0-vm-gui";
}
