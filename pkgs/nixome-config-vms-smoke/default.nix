{
  callPackage,
  coreutils,
  gnugrep,
  jq,
  lib,
  nixome-config-apply ? callPackage ../nixome-config-apply { },
  nixome-config-seed-inputs-import ? callPackage ../nixome-config-seed-inputs-import { },
  python3,
  systemd,
  writeShellApplication,
}:

let
  configVmsSeed = builtins.toFile "nixome-config-vms.nix" (
    builtins.readFile ../../seeds/config-staging/nixome/config-vms.nix
  );
  dom0HostSeed = builtins.toFile "nixome-dom0-host.nix" (
    builtins.readFile ../../seeds/config-staging/nixome/dom0-host.nix
  );
  flakeSeed = builtins.toFile "nixome-config-flake.nix" (
    builtins.readFile ../../seeds/config-staging/flake.nix
  );
  localSeed = builtins.toFile "nixome-local.nix" (
    builtins.readFile ../../seeds/config-staging/nixome/local.nix
  );
  machineSeed = builtins.toFile "nixome-machine.nix" (
    builtins.readFile ../../seeds/config-staging/nixome/machine.nix
  );
  systemSeed = builtins.toFile "nixome-system.nix" (
    builtins.readFile ../../seeds/config-staging/nixome/system.nix
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
  excludeShellChecks = [ "SC2016" ];

  runtimeInputs = [
    coreutils
    gnugrep
    jq
    nixome-config-apply
    nixome-config-seed-inputs-import
    python3.pkgs.qubes-core-qrexec.dom0
    systemd
  ];

  text = ''
    export NIXOME_CONFIG_VMS_SEED=${lib.escapeShellArg configVmsSeed}
    export NIXOME_DOM0_HOST_SEED=${lib.escapeShellArg dom0HostSeed}
    export NIXOME_FLAKE_SEED=${lib.escapeShellArg flakeSeed}
    export NIXOME_LOCAL_SEED=${lib.escapeShellArg localSeed}
    export NIXOME_MACHINE_SEED=${lib.escapeShellArg machineSeed}
    export NIXOME_SYSTEM_SEED=${lib.escapeShellArg systemSeed}
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
