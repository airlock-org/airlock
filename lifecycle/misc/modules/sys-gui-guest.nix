{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.nixome.sysGuiGuest;
  guiAgent = pkgs.qubes-gui-agent-linux.overrideAttrs (old: {
    postFixup = (old.postFixup or "") + ''
      # Failed transient sessions must be collected so another login can start
      # the same named unit without a manual reset-failed in the GUI domain.
      substituteInPlace "$out/bin/qubes-start-xephyr" \
        --replace-fail '--wait' '--wait --collect'
    '';
  });
  guestUser = config.nixome.qubeGuest.user;
in
{
  options.nixome.sysGuiGuest.enable = lib.mkEnableOption "the Nixome hybrid sys-gui role";

  config = lib.mkIf cfg.enable {
    boot.kernelModules = [
      "xen-gntalloc"
      "xen-gntdev"
      "xen-evtchn"
    ];

    environment.systemPackages = [
      guiAgent
      pkgs.qubes-gui-daemon
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
    ];

    services.udev.packages = [ guiAgent ];
    services.xserver.desktopManager.xfce.enable = true;
    systemd.packages = [ guiAgent ];
    systemd.services.qubes-gui-agent = {
      wantedBy = [ "multi-user.target" ];
      after = [
        "qubes-db.service"
        "qubes-qrexec-agent.service"
        "systemd-user-sessions.service"
      ];
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
      "qubes-rpc/qubes.GuiVMSession".source = "${guiAgent}/etc/qubes-rpc/qubes.GuiVMSession";
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

    systemd.tmpfiles.rules = [
      "d /var/log/qubes 0755 root root - -"
      "f /run/qubes-service/guivm 0644 root root - -"
      "f /run/qubes-service/guivm-gui-agent 0644 root root - -"
      "f /run/qubes-service-environment 0644 root root - -"
    ];
  };
}
