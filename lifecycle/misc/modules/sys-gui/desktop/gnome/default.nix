{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.airlock.sysGuiDesktop;
  hostPoweroff = pkgs.writeShellScript "airlock-gnome-host-poweroff" ''
    exec ${pkgs.python3.pkgs.qubes-core-admin-client}/bin/qvm-run \
      --pass-io --no-gui --service dom0 airlock.Dom0Poweroff
  '';
  hostPoweroffExtension = pkgs.stdenvNoCC.mkDerivation {
    pname = "airlock-gnome-host-poweroff";
    version = "1";
    passthru = { inherit hostPoweroff; };
    src = ./host-poweroff-extension;
    dontBuild = true;
    installPhase = ''
      runHook preInstall
      extension_dir="$out/share/gnome-shell/extensions/airlock-host-poweroff@airlock.local"
      mkdir -p "$extension_dir"
      cp extension.js metadata.json "$extension_dir/"
      substituteInPlace "$extension_dir/extension.js" \
        --replace-fail '@hostPoweroff@' '${hostPoweroff}'
      runHook postInstall
    '';
  };
  enabledExtensions = [
    "airlock-host-poweroff@airlock.local"
  ]
  ++ lib.optional config.airlock.sysGuiGuest.personalLaunchers.enable "ding@rastersoft.com";
  enabledExtensionsGVariant = "[${
    lib.concatMapStringsSep "," (extension: "'${extension}'") enabledExtensions
  }]";
in
{
  config = lib.mkIf (cfg.session == "gnome") {
    # GNOME's greeter and screen locking require GDM. Its NixOS module
    # disables the common sys-gui LightDM default.
    services.displayManager.gdm = {
      enable = true;
      # The GUI VM must stay awake while waiting for a login.
      autoSuspend = false;
    };
    services.desktopManager.gnome = {
      enable = true;
      extraGSettingsOverridePackages = [
        # The override below changes org.gnome.shell. DING's own schema lives
        # inside its extension directory, rather than NixOS's standard schema
        # location, so it must not be passed to the override compiler.
        pkgs.gnome-shell
      ];
      extraGSettingsOverrides = ''
        [org.gnome.shell]
        enabled-extensions=${enabledExtensionsGVariant}
      '';
    };
    # Where usr-home is part of the topology, applications belong in that
    # offline desktop-work VM. Minimal profiles without it retain GNOME's
    # regular core application set.
    services.gnome.core-apps.enable = lib.mkForce (
      !config.airlock.sysGuiGuest.personalLaunchers.enable
    );
    environment.systemPackages = [
      hostPoweroffExtension
    ]
    ++ lib.optionals config.airlock.sysGuiGuest.personalLaunchers.enable [
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
