{
  config,
  lib,
  pkgs,
  ...
}:
let
  agent = pkgs.qubes-gui-agent-linux;
  coreAgent = pkgs.qubes-core-agent-linux;
  python = pkgs.python3.withPackages (ps: [
    ps.pyxdg
    ps.pygobject3
    ps.dbus-python
  ]);
  autostart = pkgs.writeShellScript "qubes-session-autostart" ''
    export PYTHONPATH="${coreAgent}/${pkgs.python3.sitePackages}''${PYTHONPATH:+:$PYTHONPATH}"
    exec ${python}/bin/python ${coreAgent.src}/app-menu/qubes-session-autostart "$@"
  '';
  upstreamSession = pkgs.runCommand "airlock-qubes-app-session" { } ''
    cp ${agent}/bin/qubes-session "$out"
    chmod u+w "$out"
    substituteInPlace "$out" \
      --replace-fail /usr/bin/qubes-session-autostart ${autostart}
  '';
  session = pkgs.writeShellScript "airlock-qubes-scaled-session" ''
    # Export before upstream imports the environment into D-Bus/systemd and
    # starts qrexec-fork-server. StartApp must inherit the same display scale.
    export GDK_SCALE=${toString config.airlock.appGui.scale}
    export GDK_DPI_SCALE=1
    export XCURSOR_SIZE=${toString (24 * config.airlock.appGui.scale)}
    exec ${upstreamSession}
  '';
in
{
  options.airlock.appGui.scale = lib.mkOption {
    type = lib.types.ints.between 1 4;
    default = 1;
    description = "Integer X11 rendering scale matching the GuiVM's Xwayland scale (not its fractional monitor scale).";
  };

  config = {
    # Qubes supplies a private X server and forwards individual windows to the
    # GuiVM. App VMs do not need their own desktop or display manager.
    environment.systemPackages = [
      agent
      pkgs.dbus
      # qubes.SetMonitorLayout calls these by name.  Without them, a guest
      # that connects before the GuiVM reaches its final display size keeps
      # the initial 1280x800 X screen and its pointer cannot reach the rest of
      # a subsequently enlarged application window.
      pkgs.libxcvt
      pkgs.xrandr
      pkgs.xsetroot
    ];
    services.udev.packages = [ agent ];
    services.dbus.enable = true;
    # Preserve the upstream GUI agent requirement for shared display buffers.
    boot.kernel.sysctl."vm.compact_unevictable_allowed" = 0;
    systemd.packages = [ agent ];
    systemd.services.qubes-gui-agent = {
      # qubes-gui-runuser has a 256-byte PATH buffer. The generated Nix service
      # PATH exceeds it and makes the Xorg session fail before PAM starts.
      environment.ENV_PATH = "/run/current-system/sw/bin:/run/wrappers/bin";
      enable = true;
      wantedBy = [ "multi-user.target" ];
      requires = [
        "qubes-db.service"
        "airlock-qubes-rw-init.service"
      ];
      after = [
        "qubes-db.service"
        "qubes-qrexec-agent.service"
        "airlock-qubes-rw-init.service"
        "systemd-user-sessions.service"
      ];
      path = [
        pkgs.coreutils
        pkgs.gnused
        coreAgent
        pkgs.qubes-core-qubesdb
        pkgs.shadow
        pkgs.util-linux
        pkgs.systemd
        pkgs.dbus
        pkgs.xsetroot
      ];
    };
    security.pam.services.qubes-gui-agent = {
      rootOK = true;
      startSession = true;
    };
    environment.etc = {
      # The upstream desktop file still points at /usr/bin. Without this server,
      # qrexec launches applications outside the graphical session environment.
      "xdg/autostart/qubes-qrexec-fork-server.desktop".text = ''
        [Desktop Entry]
        Name=qrexec-fork-server
        Exec=${pkgs.python3.pkgs.qubes-core-qrexec.domU}/bin/qrexec-fork-server
        Terminal=false
        Type=Application
      '';
      "X11/xorg-qubes.conf.template".source = "${agent}/etc/X11/xorg-qubes.conf.template";
      "X11/xinit/xinitrc".source = session;
      "qubes-rpc/qubes.SetMonitorLayout".source = "${agent}/etc/qubes-rpc/qubes.SetMonitorLayout";
      "qubes/rpc-config/qubes.SetMonitorLayout".text = "force-user='${config.airlock.qubeGuest.user}'\n";
    };
  };
}
