{ config, lib, pkgs, ... }:
let
  cfg = config.airlock.sysGuiDesktop;
in
{
  config = lib.mkIf (cfg.session == "gnome") {
    services.desktopManager.gnome = {
      enable = true;
      extraGSettingsOverridePackages = lib.optionals config.airlock.sysGuiGuest.personalLaunchers.enable [
        # The override below changes org.gnome.shell. DING's own schema lives
        # inside its extension directory, rather than NixOS's standard schema
        # location, so it must not be passed to the override compiler.
        pkgs.gnome-shell
      ];
      extraGSettingsOverrides = lib.optionalString config.airlock.sysGuiGuest.personalLaunchers.enable ''
        [org.gnome.shell]
        enabled-extensions=['ding@rastersoft.com']
      '';
    };
    # Where usr-home is part of the topology, applications belong in that
    # offline desktop-work VM. Minimal profiles without it retain GNOME's
    # regular core application set.
    services.gnome.core-apps.enable = lib.mkForce (
      !config.airlock.sysGuiGuest.personalLaunchers.enable
    );
    environment.systemPackages =
      lib.optionals config.airlock.sysGuiGuest.personalLaunchers.enable [
        # Accessibility controls the sys-gui session itself.
        pkgs.gnome-tecla
        pkgs.gnomeExtensions.desktop-icons-ng-ding
        # DING delegates desktop file operations to the Nautilus executable.
        # The Airlock Files launcher still routes ordinary file-manager use to
        # usr-home, whose desktop entry takes precedence over Nautilus's.
        pkgs.nautilus
      ];

    # Sys-gui has no NetVM and should not acquire host-facing services merely
    # because the desktop implementation normally enables them by default.
    networking.networkmanager.enable = lib.mkForce false;
    hardware.bluetooth.enable = lib.mkForce false;
    services.avahi.enable = lib.mkForce false;
    services.gnome.gnome-initial-setup.enable = lib.mkForce false;
    services.gnome.gnome-remote-desktop.enable = lib.mkForce false;
    services.gnome.gnome-user-share.enable = lib.mkForce false;
    services.gnome.rygel.enable = lib.mkForce false;

  };
}
