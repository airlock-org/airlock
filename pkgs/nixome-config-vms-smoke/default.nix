{
  coreutils,
  gnugrep,
  gnused,
  jq,
  lib,
  systemd,
  writeShellApplication,
}:

writeShellApplication {
  name = "nixome-config-vms-smoke";

  runtimeInputs = [
    coreutils
    gnugrep
    gnused
    jq
    systemd
  ];

  text = builtins.readFile ../../scripts/nixos-config-vms-smoke.sh;

  meta = {
    description = "Run a Nixome smoke test for sys-config-vault and sys-config-mod";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
