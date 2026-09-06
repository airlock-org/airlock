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
  systemSeed = builtins.toFile "nixome-system.nix" (
    builtins.readFile ../../seeds/config-staging/nixome/system.nix
  );
  templatesSeed = builtins.toFile "nixome-templates.nix" (
    builtins.readFile ../../seeds/config-staging/nixome/templates.nix
  );
  systemSchemaSeed = builtins.toFile "nixome-system-schema.nix" (
    builtins.readFile ../../lib/nixome/system.nix
  );
  topologySchemaSeed = builtins.toFile "nixome-topology.nix" (
    builtins.readFile ../../lib/nixome/topology.nix
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
    export NIXOME_SYSTEM_SEED=${lib.escapeShellArg systemSeed}
    export NIXOME_TEMPLATES_SEED=${lib.escapeShellArg templatesSeed}
    export NIXOME_SYSTEM_SCHEMA_SEED=${lib.escapeShellArg systemSchemaSeed}
    export NIXOME_TOPOLOGY_SCHEMA_SEED=${lib.escapeShellArg topologySchemaSeed}
  ''
  + builtins.readFile ../../scripts/nixos-config-vms-smoke.sh;

  meta = {
    description = "Run a Nixome smoke test for config-vault, config-staging, and config-validator";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
