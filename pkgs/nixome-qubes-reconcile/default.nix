{
  coreutils,
  diffutils,
  gnugrep,
  gnused,
  jq,
  lib,
  python3,
  qubes-desktop-linux-common ? python3.pkgs.qubes-desktop-linux-common,
  qubes-core-admin ? python3.pkgs.qubes-core-admin,
  sudo,
  systemd,
  util-linux,
  writeShellApplication,
}:

writeShellApplication {
  name = "nixome-qubes-reconcile";
  excludeShellChecks = [ "SC2016" ];

  runtimeInputs = [
    coreutils
    diffutils
    gnugrep
    gnused
    jq
    qubes-core-admin
    qubes-desktop-linux-common
    sudo
    python3.pkgs.qubes-core-admin-client
    systemd
    util-linux
  ];

  text = builtins.readFile ../../scripts/reconcile-qubes-topology.sh;

  meta = {
    description = "Reconcile Nixome's declared Qubes topology on dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
