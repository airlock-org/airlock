{
  coreutils,
  gnugrep,
  jq,
  lib,
  python3,
  systemd,
  writeShellApplication,
}:

let
  configVmsSeed = builtins.toFile "nixome-config-vms.nix" (
    builtins.readFile ../../seeds/config-staging/nixome/config-vms.nix
  );
in
writeShellApplication {
  name = "nixome-config-vms-smoke";

  runtimeInputs = [
    coreutils
    gnugrep
    jq
    python3.pkgs.qubes-core-qrexec.dom0
    systemd
  ];

  text = ''
    export NIXOME_CONFIG_VMS_SEED=${lib.escapeShellArg configVmsSeed}
  ''
  + builtins.readFile ../../scripts/nixos-config-vms-smoke.sh;

  meta = {
    description = "Run a Nixome smoke test for config-vault, config-staging, and config-validator";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
