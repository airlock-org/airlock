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
in
{
  options.airlock.sysGuiGuest.enable = lib.mkEnableOption "the Airlock hybrid sys-gui role";

  config = lib.mkIf cfg.enable {
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
      pkgs.xfce4-panel
      pkgs.xfdesktop
      pkgs.xdpyinfo
      pkgs.xinit
      pkgs.xorgserver
      pkgs.xdotool
      pkgs.xterm
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
        xserverBin = lib.mkOverride 50 loginXserver;
        xserverArgs = lib.mkForce [ ];
        lightdm = {
          enable = true;
          extraConfig = ''
            logind-check-graphical=false
            minimum-vt=8
          '';
          extraSeatDefaults = ''
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
      # Start only when dom0 has connected the Qubes display transport.
      wantedBy = lib.mkForce [ ];
      requires = [ "qubes-gui-agent.service" ];
      after = [ "qubes-gui-agent.service" "airlock-qubes-rw-init.service" ];
    };
    systemd.packages = [ guiAgent ];
    systemd.services.qubes-gui-agent = {
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
      "qubes-rpc/qubes.GuiVMSession".source = guiSession;
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
    };

    # Canonical desktop IDs keep XFCE's menu, launcher search, dock and MIME
    # defaults pointed at the two user-facing domains instead of per-VM copies.
    xdg.mime.defaultApplications = {
      "inode/directory" = "org.gnome.Nautilus.desktop";
      "x-scheme-handler/http" = "firefox.desktop";
      "x-scheme-handler/https" = "firefox.desktop";
      "text/html" = "firefox.desktop";
      "application/xhtml+xml" = "firefox.desktop";
    };

    systemd.tmpfiles.rules = [
      "d /var/log/qubes 0755 root root - -"
      "f /run/qubes-service/guivm 0644 root root - -"
      "f /run/qubes-service/guivm-gui-agent 0644 root root - -"
      "f /run/qubes-service-environment 0644 root root - -"
    ];
  };
}
