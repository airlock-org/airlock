{ config, pkgs, ... }:
let
  agent = pkgs.qubes-gui-agent-linux;
  coreAgent = pkgs.qubes-core-agent-linux;
  python = pkgs.python3.withPackages (ps: [ ps.pyxdg ps.pygobject3 ps.dbus-python ]);
  autostart = pkgs.writeShellScript "qubes-session-autostart" ''
    export PYTHONPATH="${coreAgent}/${pkgs.python3.sitePackages}''${PYTHONPATH:+:$PYTHONPATH}"
    exec ${python}/bin/python ${coreAgent.src}/app-menu/qubes-session-autostart "$@"
  '';
  session = pkgs.runCommand "airlock-qubes-app-session" { } ''
    cp ${agent}/bin/qubes-session "$out"
    chmod u+w "$out"
    substituteInPlace "$out" \
      --replace-fail /usr/bin/qubes-session-autostart ${autostart}
  '';
in
{
  # Qubes supplies a private X server and forwards individual windows to the
  # GuiVM. App VMs do not need their own desktop or display manager.
  environment.systemPackages = [ agent pkgs.dbus pkgs.xsetroot ];
  services.udev.packages = [ agent ];
  services.dbus.enable = true;
  # Preserve the upstream GUI agent requirement for shared display buffers.
  boot.kernel.sysctl."vm.compact_unevictable_allowed" = 0;
  systemd.packages = [ agent ];
  systemd.services.qubes-gui-agent = {
    enable = true;
    wantedBy = [ "multi-user.target" ];
    requires = [ "qubes-db.service" "airlock-qubes-rw-init.service" ];
    after = [
      "qubes-db.service"
      "qubes-qrexec-agent.service"
      "airlock-qubes-rw-init.service"
      "systemd-user-sessions.service"
    ];
    path = [
      pkgs.coreutils pkgs.gnused coreAgent pkgs.qubes-core-qubesdb
      pkgs.shadow pkgs.util-linux pkgs.systemd pkgs.dbus pkgs.xsetroot
    ];
  };
  security.pam.services.qubes-gui-agent = {
    rootOK = true;
    startSession = true;
  };
  environment.etc = {
    "X11/xorg-qubes.conf.template".source = "${agent}/etc/X11/xorg-qubes.conf.template";
    "X11/xinit/xinitrc".source = session;
    "qubes-rpc/qubes.SetMonitorLayout".source = "${agent}/etc/qubes-rpc/qubes.SetMonitorLayout";
    "qubes/rpc-config/qubes.SetMonitorLayout".text = "force-user='${config.airlock.qubeGuest.user}'\n";
  };
}
