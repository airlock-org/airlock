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
    description = "Run a Nixome smoke test for config-vault, config-staging, and config-validator";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
