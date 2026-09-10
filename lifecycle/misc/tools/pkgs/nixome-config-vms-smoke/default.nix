{
  callPackage,
  coreutils,
  gnugrep,
  jq,
  lib,
  nixome-config-apply ? callPackage ../../../../runtime/tools/pkgs/nixome-config-apply { },
  nixome-config-inputs-import ? callPackage ../nixome-config-inputs-import { },
  python3,
  systemd,
  writeShellApplication,
}:

let
  initialConfigVms = builtins.toFile "nixome-config-vms.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/nixome/config-vms.nix
  );
  initialDom0HostConfig = builtins.toFile "nixome-dom0-host.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/nixome/dom0-host.nix
  );
  initialConfigFlake = builtins.toFile "nixome-config-flake.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/flake.nix
  );
  initialLocalConfig = builtins.toFile "nixome-local.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/nixome/local.nix
  );
  initialRuntimeConfig = builtins.toFile "nixome-runtime.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/nixome/runtime.nix
  );
  initialSystemConfig = builtins.toFile "nixome-system.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/nixome/system.nix
  );
  systemSchemaSource = builtins.toFile "nixome-system-schema.nix" (
    builtins.readFile ../../../lib/system.nix
  );
  topologySchemaSource = builtins.toFile "nixome-topology.nix" (
    builtins.readFile ../../../lib/topology.nix
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
    nixome-config-inputs-import
    python3.pkgs.qubes-core-qrexec.dom0
    systemd
  ];

  text = ''
    export NIXOME_INITIAL_CONFIG_VMS=${lib.escapeShellArg initialConfigVms}
    export NIXOME_INITIAL_DOM0_HOST_CONFIG=${lib.escapeShellArg initialDom0HostConfig}
    export NIXOME_INITIAL_CONFIG_FLAKE=${lib.escapeShellArg initialConfigFlake}
    export NIXOME_INITIAL_LOCAL_CONFIG=${lib.escapeShellArg initialLocalConfig}
    export NIXOME_INITIAL_RUNTIME_CONFIG=${lib.escapeShellArg initialRuntimeConfig}
    export NIXOME_INITIAL_SYSTEM_CONFIG=${lib.escapeShellArg initialSystemConfig}
    export NIXOME_SYSTEM_SCHEMA_SOURCE=${lib.escapeShellArg systemSchemaSource}
    export NIXOME_TOPOLOGY_SCHEMA_SOURCE=${lib.escapeShellArg topologySchemaSource}
  ''
  + builtins.readFile ../../scripts/nixos-config-vms-smoke.sh;

  meta = {
    description = "Run a Nixome smoke test for config-vault, config-staging, and config-validator";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
