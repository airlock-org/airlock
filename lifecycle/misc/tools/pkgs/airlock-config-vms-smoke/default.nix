{
  callPackage,
  coreutils,
  gnugrep,
  jq,
  lib,
  airlock-config-apply ? callPackage ../../../../runtime/tools/pkgs/airlock-config-apply { },
  airlock-config-inputs-import ? callPackage ../airlock-config-inputs-import { },
  python3,
  systemd,
  writeShellApplication,
}:

let
  initialConfigVms = builtins.toFile "airlock-config-vms.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/airlock/config-vms.nix
  );
  initialDom0HostConfig = builtins.toFile "airlock-dom0-host.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/airlock/dom0-host.nix
  );
  initialConfigFlake = builtins.toFile "airlock-config-flake.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/flake.nix
  );
  initialLocalConfig = builtins.toFile "airlock-local.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/airlock/local.nix
  );
  initialRuntimeConfig = builtins.toFile "airlock-runtime.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/airlock/runtime.nix
  );
  initialSystemConfig = builtins.toFile "airlock-system.nix" (
    builtins.readFile ../../../../bootstrap/initial-runtime-config/airlock/system.nix
  );
  systemSchemaSource = builtins.toFile "airlock-system-schema.nix" (
    builtins.readFile ../../../lib/system.nix
  );
  topologySchemaSource = builtins.toFile "airlock-topology.nix" (
    builtins.readFile ../../../lib/topology.nix
  );
in
writeShellApplication {
  name = "airlock-config-vms-smoke";
  excludeShellChecks = [ "SC2016" ];

  runtimeInputs = [
    coreutils
    gnugrep
    jq
    airlock-config-apply
    airlock-config-inputs-import
    python3.pkgs.qubes-core-qrexec.dom0
    systemd
  ];

  text = ''
    export AIRLOCK_INITIAL_CONFIG_VMS=${lib.escapeShellArg initialConfigVms}
    export AIRLOCK_INITIAL_DOM0_HOST_CONFIG=${lib.escapeShellArg initialDom0HostConfig}
    export AIRLOCK_INITIAL_CONFIG_FLAKE=${lib.escapeShellArg initialConfigFlake}
    export AIRLOCK_INITIAL_LOCAL_CONFIG=${lib.escapeShellArg initialLocalConfig}
    export AIRLOCK_INITIAL_RUNTIME_CONFIG=${lib.escapeShellArg initialRuntimeConfig}
    export AIRLOCK_INITIAL_SYSTEM_CONFIG=${lib.escapeShellArg initialSystemConfig}
    export AIRLOCK_SYSTEM_SCHEMA_SOURCE=${lib.escapeShellArg systemSchemaSource}
    export AIRLOCK_TOPOLOGY_SCHEMA_SOURCE=${lib.escapeShellArg topologySchemaSource}
  ''
  + builtins.readFile ../../scripts/nixos-config-vms-smoke.sh;

  meta = {
    description = "Run a Airlock smoke test for sys-config-vault, sys-config-staging, and sys-config-validator";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
