{ pkgs, lib, ... }:
{
  # Shared desktop configuration for the home VM.
  imports = [
    ./airlock-guest.nix
    ./gnome-session.nix
  ];

  networking.hostName = "usr-home";
  networking.useDHCP = false;
  time.timeZone = "Europe/London";

  users.users.user = {
    isNormalUser = true;
    description = "Airlock user";
    extraGroups = [ "audio" "video" ];
    home = "/home/user";
    shell = pkgs.bashInteractive;
  };

  environment.systemPackages = with pkgs; [
    baobab
    evince
    file-roller
    gnome-calculator
    gnome-calendar
    gnome-clocks
    gnome-text-editor
    loupe
    seahorse
    keepassxc
  ];

  programs.bash.enableCompletion = true;
  programs.command-not-found.enable = false;
  programs.dconf.enable = true;
  programs.gnome-disks.enable = true;
  programs.gnome-terminal.enable = false;

  services.dbus.enable = true;
  services.gvfs.enable = true;
  services.udisks2.enable = true;
  services.upower.enable = true;
  services.gnome.gnome-keyring.enable = true;

  xdg.portal = {
    enable = true;
    config.common.default = [ "gnome" "gtk" ];
    extraPortals = [
      pkgs.xdg-desktop-portal-gnome
      pkgs.xdg-desktop-portal-gtk
    ];
  };

  xdg.mime.enable = true;
  xdg.mime.defaultApplications = {
    "application/pdf" = [ "org.gnome.Evince.desktop" ];
    "application/zip" = [ "org.gnome.FileRoller.desktop" ];
    "image/jpeg" = [ "org.gnome.Loupe.desktop" ];
    "image/png" = [ "org.gnome.Loupe.desktop" ];
    "text/plain" = [ "org.gnome.TextEditor.desktop" ];
  };

  fonts = {
    fontDir.enable = true;
    enableDefaultPackages = true;
    packages = with pkgs; [
      dejavu_fonts
      liberation_ttf
      noto-fonts
      noto-fonts-color-emoji
    ];
    fontconfig.defaultFonts = {
      sansSerif = [ "DejaVu Sans" ];
      serif = [ "DejaVu Serif" ];
      monospace = [ "DejaVu Sans Mono" ];
      emoji = [ "Noto Color Emoji" ];
    };
  };

  # Session preferences are kept declarative alongside application packages.
  programs.dconf.profiles.user.databases = [
    {
      settings = {
        "org/gnome/desktop/interface" = {
          color-scheme = "prefer-dark";
          clock-show-weekday = true;
          clock-show-seconds = false;
          enable-animations = true;
          enable-hot-corners = false;
          show-battery-percentage = true;
          text-scaling-factor = 1.0;
        };
        "org/gnome/desktop/background" = {
          picture-options = "zoom";
          primary-color = "#10202b";
          secondary-color = "#071017";
          color-shading-type = "solid";
        };
        "org/gnome/desktop/calendar" = {
          show-weekdate = false;
        };
        "org/gnome/desktop/peripherals/mouse" = {
          natural-scroll = false;
          speed = 0.0;
        };
        "org/gnome/desktop/peripherals/touchpad" = {
          natural-scroll = true;
          tap-to-click = true;
          two-finger-scrolling-enabled = true;
          disable-while-typing = true;
        };
        "org/gnome/desktop/privacy" = {
          remember-recent-files = false;
          recent-files-max-age = 7;
          remove-old-temp-files = true;
          remove-old-trash-files = true;
          old-files-age = 30;
        };
        "org/gnome/desktop/wm/preferences" = {
          button-layout = "appmenu:minimize,maximize,close";
          num-workspaces = 4;
          focus-mode = "click";
          action-double-click-titlebar = "toggle-maximize";
        };
        "org/gnome/mutter" = {
          dynamic-workspaces = true;
          workspaces-only-on-primary = true;
          attach-modal-dialogs = true;
        };
        "org/gnome/nautilus/preferences" = {
          default-folder-viewer = "list-view";
          show-delete-permanently = false;
          search-filter-time-type = "last_modified";
        };
        "org/gnome/TextEditor" = {
          show-line-numbers = true;
          highlight-current-line = true;
          indent-style = "space";
          tab-width = lib.gvariant.mkUint32 2;
          use-system-font = false;
          custom-font = "Monospace 12";
        };
      };
    }
  ];

  # These values are shared by interactive and application sessions.
  environment.sessionVariables = {
    EDITOR = "gnome-text-editor";
    VISUAL = "gnome-text-editor";
    GTK_USE_PORTAL = "1";
    XDG_CURRENT_DESKTOP = "GNOME";
    XDG_SESSION_DESKTOP = "gnome";
    NIXOS_OZONE_WL = "1";
  };

  environment.pathsToLink = [
    "/share/applications"
    "/share/icons"
    "/share/glib-2.0/schemas"
    "/share/xdg-desktop-portal"
  ];

  security.polkit.enable = true;
  security.rtkit.enable = true;

  services.pipewire = {
    enable = true;
    alsa.enable = true;
    pulse.enable = true;
    jack.enable = false;
  };

  services.journald.extraConfig = ''
    SystemMaxUse=256M
    RuntimeMaxUse=64M
    MaxRetentionSec=7day
  '';

  systemd.user.services.desktop-ready = {
    description = "Record that the home desktop is ready";
    wantedBy = [ "graphical-session.target" ];
    after = [ "graphical-session.target" ];
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = true;
      ExecStart = "${pkgs.coreutils}/bin/true";
    };
  };

  system.stateVersion = "26.11";
}
