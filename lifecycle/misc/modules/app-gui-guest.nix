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
  monitorLayout = pkgs.writeShellScript "airlock-qubes-set-monitor-layout" ''
    set -eu

    # The RPC handler runs outside the graphical login shell.  Give the
    # upstream script every command it invokes explicitly; otherwise a
    # missing xrandr makes its 100 ms Xorg readiness loop run forever.
    export PATH=${lib.makeBinPath [
      pkgs.coreutils
      pkgs.gnugrep
      pkgs.libxcvt
      pkgs.xrandr
    ]}

    # A layout request can race Xorg startup, but it must not become a
    # permanent polling process if the display never appears.
    exec ${pkgs.coreutils}/bin/timeout --signal=TERM --kill-after=5s 60s \
      ${agent}/bin/qubes-set-monitor-layout "$@"
  '';
  sessionReady = pkgs.writeShellScript "airlock-app-session-ready" ''
    set -eu

    [ "''${QREXEC_REMOTE_DOMAIN:-}" = sys-gui ] || {
      echo "airlock.AppSessionReady is restricted to sys-gui" >&2
      exit 1
    }
    [ -z "''${QREXEC_SERVICE_ARGUMENT:-}" ] || {
      echo "airlock.AppSessionReady does not accept arguments" >&2
      exit 1
    }

    # force-user routes this service through the graphical user's
    # qrexec-fork-server.  Also prove that the inherited session environment
    # can reach X and D-Bus before a pending StartApp request is released.
    [ -n "''${DISPLAY:-}" ]
    [ -n "''${XDG_RUNTIME_DIR:-}" ]
    exec ${pkgs.coreutils}/bin/timeout --signal=TERM --kill-after=5s 60s \
      ${pkgs.bash}/bin/bash -c \
      'until [ -S "$XDG_RUNTIME_DIR/bus" ] && ${pkgs.xdpyinfo}/bin/xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; do ${pkgs.coreutils}/bin/sleep 0.2; done'
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
      pkgs.xdpyinfo
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
      # qvm-start-gui and the guest can start concurrently.  Retry if the
      # GuiVM vchan backend was not ready for the first connection attempt.
      unitConfig.StartLimitIntervalSec = 0;
      serviceConfig = {
        Restart = "on-failure";
        RestartSec = "5s";
      };
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
      "qubes-rpc/qubes.SetMonitorLayout".source = monitorLayout;
      "qubes/rpc-config/qubes.SetMonitorLayout".text = "force-user='${config.airlock.qubeGuest.user}'\n";
      "qubes-rpc/airlock.AppSessionReady" = {
        source = sessionReady;
        mode = "0555";
      };
      "qubes/rpc-config/airlock.AppSessionReady" = {
        text = "force-user='${config.airlock.qubeGuest.user}'\n";
        mode = "0444";
      };
    };
  };
}
