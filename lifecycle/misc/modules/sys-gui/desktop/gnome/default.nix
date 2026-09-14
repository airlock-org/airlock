{ config, lib, pkgs, ... }:
let
  cfg = config.airlock.sysGuiDesktop;
  dashToDock = pkgs.gnomeExtensions.dash-to-dock;
in
{
  config = lib.mkIf (cfg.session == "gnome") {
    services.desktopManager.gnome.enable = true;

    # Sys-gui has no NetVM and should not acquire host-facing services merely
    # because the desktop implementation normally enables them by default.
    networking.networkmanager.enable = lib.mkForce false;
    hardware.bluetooth.enable = lib.mkForce false;
    services.avahi.enable = lib.mkForce false;
    services.gnome.gnome-initial-setup.enable = lib.mkForce false;
    services.gnome.gnome-remote-desktop.enable = lib.mkForce false;
    services.gnome.gnome-user-share.enable = lib.mkForce false;
    services.gnome.rygel.enable = lib.mkForce false;

    environment.systemPackages = [ dashToDock ];

    # Install and enable the extension from Nix. Lock only the extension list:
    # the user can still customize Dash to Dock's own settings in GNOME.
    programs.dconf.profiles.user.databases = [
      {
        settings."org/gnome/shell".enabled-extensions = [ dashToDock.extensionUuid ];
        locks = [ "/org/gnome/shell/enabled-extensions" ];
      }
    ];
  };
}
