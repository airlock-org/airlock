{
  coreutils,
  gnugrep,
  gnused,
  jq,
  lib,
  python3,
  systemd,
  writeShellApplication,
}:

writeShellApplication {
  name = "nixome-qubes-reconcile";
  excludeShellChecks = [ "SC2016" ];

  runtimeInputs = [
    coreutils
    gnugrep
    gnused
    jq
    python3.pkgs.qubes-core-admin-client
    systemd
  ];

  text = builtins.readFile ../../scripts/reconcile-qubes-topology.sh;

  meta = {
    description = "Reconcile Nixome's declared Qubes topology on dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
