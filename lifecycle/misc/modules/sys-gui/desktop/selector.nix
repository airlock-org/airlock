{ config, lib, ... }:
let
  cfg = config.airlock.sysGuiDesktop;
  displayManagerSession = if cfg.session == "hyprland" then "airlock-hyprland" else cfg.session;
in
{
  imports = [ ./hyprland/default.nix ./gnome/default.nix ];

  options.airlock.sysGuiDesktop.session = lib.mkOption {
    type = lib.types.enum [ "xfce" "gnome" "hyprland" ];
    default = "xfce";
    description = "Desktop implementation selected for the physical sys-gui session.";
  };

  config = {
    # The implementation modules stay installed as independent Nix modules;
    # this selector is the one place that decides which receives the session.
    airlock.sysGuiHyprland.enable = lib.mkDefault (cfg.session == "hyprland");
    # The common sys-gui role supplies XFCE as its conservative default.
    # A selected implementation must override that default deterministically.
    services.displayManager.defaultSession = lib.mkForce displayManagerSession;
  };
}
