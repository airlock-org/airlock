{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.airlock.sysGuiGuest;
  guiAgent = pkgs.qubes-gui-agent-linux;
  guestUser = config.airlock.qubeGuest.user;
  hybrid = cfg.displayMode == "hybrid";
  physicalXserver = pkgs.writeShellScript "airlock-gui-physical-xserver" ''
    export LD_PRELOAD=${pkgs.qubes-gui-daemon}/lib/qubes-gui-daemon/shmoverride.so
    exec ${pkgs.xorg-server}/bin/Xorg "$@"
  '';
  filesLauncher = pkgs.writeShellScriptBin "airlock-launch-files" ''
    set -eu
    exec ${pkgs.python3.pkgs.qubes-core-admin-client}/bin/qvm-run \
      --auto --service usr-home qubes.StartApp+org.gnome.Nautilus
  '';
  browserLauncher = pkgs.writeShellScriptBin "airlock-launch-browser" ''
    set -eu
    if [ "$#" -eq 0 ]; then
      exec ${pkgs.python3.pkgs.qubes-core-admin-client}/bin/qvm-run \
        --auto --service usr-browser qubes.StartApp+firefox
    fi
    # URI activation is deliberately a separate fixed RPC, rather than a
    # general shell capability from sys-gui into the browser domain.
    exec ${pkgs.python3.pkgs.qubes-core-admin-client}/bin/qvm-run \
      --auto --service usr-browser "airlock.OpenUrl+$1"
  '';
  filesDesktop = pkgs.makeDesktopItem {
    name = "org.gnome.Nautilus";
    desktopName = "Files";
    genericName = "File Manager";
    exec = "airlock-launch-files";
    icon = "org.gnome.Nautilus";
    categories = [ "GNOME" "GTK" "Utility" "FileManager" ];
    mimeTypes = [ "inode/directory" ];
  };
  browserDesktop = pkgs.makeDesktopItem {
    name = "firefox";
    desktopName = "Firefox";
    genericName = "Web Browser";
    exec = "airlock-launch-browser %u";
    icon = "firefox";
    categories = [ "Network" "WebBrowser" ];
    mimeTypes = [
      "x-scheme-handler/http"
      "x-scheme-handler/https"
      "text/html"
      "application/xhtml+xml"
    ];
  };
  panelLaunchers = pkgs.writeShellScript "airlock-bootstrap-desktop-launchers" ''
    set -eu

    config_home="''${XDG_CONFIG_HOME:-$HOME/.config}"
    marker="$config_home/airlock/desktop-bootstrap-v1"
    legacy_marker="$config_home/airlock/desktop-launchers-v1"
    if [ -e "$marker" ] || [ -e "$legacy_marker" ]; then
      exit 0
    fi

    panel=""
    for _attempt in $(${pkgs.coreutils}/bin/seq 1 30); do
      panel="$(${pkgs.xfconf}/bin/xfconf-query -c xfce4-panel -p /panels -l 2>/dev/null |
        ${pkgs.gnused}/bin/sed -n 's#^\(/panels/panel-[0-9][0-9]*\)$#\1#p' |
        ${pkgs.coreutils}/bin/head -n 1 || true)"
      [ -n "$panel" ] && break
      ${pkgs.coreutils}/bin/sleep 1
    done
    [ -n "$panel" ] || exit 0

    # Recreate the panel's plugin-id array with two new launcher plugins.
    # This desktop-bootstrap action runs only for a fresh Airlock user
    # profile, preserving later panel changes in the persistent sys-gui home.
    ids="$(${pkgs.xfconf}/bin/xfconf-query -c xfce4-panel -p "$panel/plugin-ids" 2>/dev/null |
      ${pkgs.gnugrep}/bin/grep -E '^[0-9]+$' | ${pkgs.coreutils}/bin/tr '\n' ' ' || true)"
    next=100
    for id in $ids; do
      [ "$id" -ge "$next" ] && next=$((id + 1))
    done

    launcher_root="$config_home/xfce4/panel"
    ${pkgs.coreutils}/bin/install -d -m 0700 "$launcher_root"
    for desktop in org.gnome.Nautilus firefox; do
      id="$next"
      next=$((next + 1))
      ${pkgs.xfconf}/bin/xfconf-query -c xfce4-panel -p "/plugins/plugin-$id" \
        -n -t string -s launcher
      ${pkgs.coreutils}/bin/install -d -m 0700 "$launcher_root/launcher-$id"
      case "$desktop" in
        org.gnome.Nautilus) source=${filesDesktop}/share/applications/org.gnome.Nautilus.desktop ;;
        firefox) source=${browserDesktop}/share/applications/firefox.desktop ;;
      esac
      ${pkgs.coreutils}/bin/install -m 0600 \
        "$source" \
        "$launcher_root/launcher-$id/$desktop.desktop"
      ids="$ids$id "
    done
    # --force-array is necessary when the original panel has a single plugin.
    # shellcheck disable=SC2086
    ${pkgs.xfconf}/bin/xfconf-query -c xfce4-panel -p "$panel/plugin-ids" \
      -a -t int -s $ids
    ${pkgs.coreutils}/bin/install -d -m 0700 "$(${pkgs.coreutils}/bin/dirname "$marker")"
    : > "$marker"
  '';
  # LightDM owns the inner display and authenticates the desktop user. The
  # Qubes agent still owns the outer Xorg display (:1) forwarded to dom0.
  loginXserver = pkgs.writeShellScript "airlock-gui-login-xserver" ''
    set -euo pipefail
    export DISPLAY=:1
    unset XAUTHORITY
    ${pkgs.coreutils}/bin/timeout 60 ${pkgs.bash}/bin/bash -c '
      until ${pkgs.xdpyinfo}/bin/xdpyinfo -display :1 >/dev/null 2>&1; do
        ${pkgs.coreutils}/bin/sleep 1
      done
    '

    # NixOS/LightDM adds Xorg logging and physical-seat options that Xephyr
    # does not implement. Preserve display selection, -auth and access control.
    args=()
    while [ "$#" -gt 0 ]; do
      case "$1" in
        -logfile|-seat) shift 2 ;;
        vt[0-9]*|-novtswitch) shift ;;
        *) args+=("$1"); shift ;;
      esac
    done
    export LD_PRELOAD=${pkgs.qubes-gui-daemon}/lib/qubes-gui-daemon/shmoverride.so
    exec ${pkgs.xorg-server}/bin/Xephyr \
      -sw-cursor -no-host-grab -output DUMMY0 "''${args[@]}"
  '';
  guiSession = pkgs.writeShellScript "airlock-gui-login-session" ''
    set -euo pipefail
    if [ "''${QREXEC_REMOTE_DOMAIN:-}" != dom0 ]; then
      echo "GUI login may only be started by dom0" >&2
      exit 1
    fi
    exec 9>/run/airlock-gui-login.lock
    ${pkgs.util-linux}/bin/flock -n 9 || exit 1
    trap '${pkgs.systemd}/bin/systemctl stop --no-block display-manager.service' EXIT
    trap 'exit 0' HUP INT TERM
    ${pkgs.systemd}/bin/systemctl start display-manager.service
    # Logging out returns to the guest greeter, so the relay stays connected.
    while ${pkgs.systemd}/bin/systemctl is-active --quiet display-manager.service; do
      ${pkgs.coreutils}/bin/sleep 2
    done
    ${pkgs.systemd}/bin/journalctl -u display-manager.service -n 30 --no-pager >&2
    exit 1
  '';
  # XFCE's panel power action calls `xfce4-session-logout --halt`. In a GUI
  # domain, upstream would halt only this VM, which destroys the active
  # desktop relay. Route precisely that action through the dedicated dom0
  # qrexec service; all other session actions retain XFCE's normal behavior.
  poweroffSessionAction = lib.hiPrio (pkgs.writeShellScriptBin "xfce4-session-logout" ''
    set -eu
    if [ "''${1:-}" = "--halt" ]; then
      exec ${pkgs.python3.pkgs.qubes-core-admin-client}/bin/qvm-run \
        --pass-io --no-gui --service dom0 airlock.Dom0Poweroff
    fi
    exec ${pkgs.xfce4-session}/bin/xfce4-session-logout "$@"
  '');
in
{
  imports = [ ./qubes-input-proxy.nix ];
  options.airlock.sysGuiGuest.enable = lib.mkEnableOption "the Airlock hybrid sys-gui role";
  options.airlock.sysGuiGuest.displayMode = lib.mkOption {
    type = lib.types.enum [ "hybrid" "physical-amd-lab" ];
    default = "hybrid";
    description = "Hybrid relay by default; physical AMD Xorg is an experimental lab mode.";
  };
  options.airlock.sysGuiGuest.physicalDesktopOnBoot = lib.mkEnableOption
    "automatic physical desktop login after guest boot (not the hybrid relay)";
  options.airlock.sysGuiGuest.personalLaunchers.enable = lib.mkOption {
    type = lib.types.bool;
    default = true;
    description = "Install Files and Firefox launchers targeting usr-home and usr-browser.";
  };

  config = lib.mkIf cfg.enable {
    airlock.inputProxy.receiver.enable = lib.mkDefault (!hybrid);
    # Keep a text-only physical diagnostic mode available. graphical.target
    # itself pulls in LightDM, independently of the service's wantedBy.
    systemd.defaultUnit = lib.mkIf (!hybrid) (lib.mkForce (
      if cfg.physicalDesktopOnBoot then "graphical.target" else "multi-user.target"
    ));
    boot.kernelModules = [
      "xen-gntalloc"
      "xen-gntdev"
      "xen-evtchn"
    ];

    environment.systemPackages = [
      guiAgent
      pkgs.qubes-gui-daemon
      pkgs.qubes-manager
      pkgs.qubes-artwork
      pkgs.python3.pkgs.qubes-core-admin-client
      pkgs.python3.pkgs.qubes-desktop-linux-common
      pkgs.python3.pkgs.qubes-desktop-linux-manager
      pkgs.xfce4-session
      poweroffSessionAction
      pkgs.xfce4-panel
      pkgs.xfdesktop
      pkgs.xdpyinfo
      pkgs.xinit
      pkgs.xorgserver
      pkgs.xdotool
      pkgs.xterm
    ]
    ++ lib.optionals cfg.personalLaunchers.enable [
      filesLauncher
      browserLauncher
      filesDesktop
      browserDesktop
    ];

    services.udev.packages = [ guiAgent ];
    services.xserver = {
      enable = true;
      desktopManager.xfce.enable = true;
      desktopManager.xterm.enable = false;
      displayManager = {
        xserverBin = lib.mkOverride 50 (if hybrid then loginXserver else physicalXserver);
        xserverArgs = lib.mkIf hybrid (lib.mkForce [ ]);
        lightdm = {
          enable = true;
          extraConfig = lib.mkIf hybrid ''
            logind-check-graphical=false
            minimum-vt=8
          '';
          extraSeatDefaults = lib.mkIf hybrid ''
            type=xlocal
            xserver-share=true
          '';
        };
      };
    };
    services.displayManager = {
      defaultSession = "xfce";
      autoLogin.enable = false;
    };
    systemd.services.display-manager = {
      # Hybrid always waits for its dom0 relay; physical boot is opt-in.
      wantedBy = lib.mkForce (lib.optional (!hybrid && cfg.physicalDesktopOnBoot) "graphical.target");
      requires = lib.optional hybrid "qubes-gui-agent.service";
      after = [ "airlock-qubes-rw-init.service" ] ++ lib.optional hybrid "qubes-gui-agent.service";
    };
    systemd.packages = [ guiAgent ];
    systemd.services.qubes-gui-agent = {
      enable = hybrid;
      wantedBy = [ "multi-user.target" ];
      after = [
        "qubes-db.service"
        "qubes-qrexec-agent.service"
        "systemd-user-sessions.service"
      ];
      # LightDM manages Xephyr, not the outer Xorg server: both must run.
      unitConfig.ConditionPathExists = lib.mkForce "";
      path = [
        pkgs.coreutils
        pkgs.gnused
        pkgs.qubes-core-agent-linux
        pkgs.qubes-core-qubesdb
        pkgs.shadow
        pkgs.util-linux
        pkgs.systemd
      ];
    };

    # Use NixOS PAM modules and session setup. The packaged Fedora PAM file
    # includes system-auth/postlogin, neither of which exists on NixOS.
    security.pam.services.qubes-gui-agent = {
      rootOK = true;
      startSession = true;
    };

    environment.etc = {
      "X11/xorg-qubes.conf.template".source = "${guiAgent}/etc/X11/xorg-qubes.conf.template";
      "X11/xinit/xinitrc".source = "${guiAgent}/bin/qubes-session";
      "qubes-rpc/qubes.GuiVMSession" = lib.mkIf hybrid { source = guiSession; };
      "qubes-rpc/qubes.SetMonitorLayout".source = "${guiAgent}/etc/qubes-rpc/qubes.SetMonitorLayout";
      "qubes-rpc/qubes.WindowIconUpdater".source =
        "${pkgs.qubes-gui-daemon}/etc/qubes-rpc/qubes.WindowIconUpdater";
      "qubes/guid.conf".source = "${pkgs.qubes-gui-daemon}/etc/qubes/guid.conf";
      "qubes/rpc-config/qubes.GuiVMSession".source =
        "${guiAgent}/etc/qubes/rpc-config/qubes.GuiVMSession";
      "qubes/rpc-config/qubes.SetMonitorLayout".text = "force-user='${guestUser}'\n";
      "qubes/rpc-config/qubes.WindowIconUpdater".source =
        "${pkgs.qubes-gui-daemon}/etc/qubes/rpc-config/qubes.WindowIconUpdater";
      "xdg/autostart/qubes-icon-receiver.desktop".source =
        "${pkgs.qubes-gui-daemon}/etc/xdg/autostart/qubes-icon-receiver.desktop";
      "xdg/autostart/qubes-screen-layout-watches.desktop".source =
        "${pkgs.qubes-gui-daemon}/etc/xdg/autostart/qubes-screen-layout-watches.desktop";
      "xdg/autostart/airlock-bootstrap-desktop-launchers.desktop" = lib.mkIf cfg.personalLaunchers.enable {
        text = ''
          [Desktop Entry]
          Type=Application
          Name=Airlock desktop bootstrap
          Exec=${panelLaunchers}
          OnlyShowIn=XFCE;
          X-GNOME-Autostart-Phase=Panel
        '';
      };
    };

    # Canonical desktop IDs keep XFCE's menu, launcher search, dock and MIME
    # defaults pointed at the two user-facing domains instead of per-VM copies.
    xdg.mime.defaultApplications = lib.mkIf cfg.personalLaunchers.enable {
      "inode/directory" = "org.gnome.Nautilus.desktop";
      "x-scheme-handler/http" = "firefox.desktop";
      "x-scheme-handler/https" = "firefox.desktop";
      "text/html" = "firefox.desktop";
      "application/xhtml+xml" = "firefox.desktop";
    };

    systemd.tmpfiles.rules = [
      "d /var/log/qubes 0755 root root - -"
      "f /run/qubes-service/guivm 0644 root root - -"
      "f /run/qubes-service-environment 0644 root root - -"
    ] ++ lib.optional hybrid "f /run/qubes-service/guivm-gui-agent 0644 root root - -";
  };
}
