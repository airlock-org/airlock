{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.airlock.machineDeployment;
  engine = pkgs.callPackage ../../runtime/tools/pkgs/airlock-machine-deploy { };
  setup = pkgs.callPackage ../tools/pkgs/airlock-setup-config { };
  guardedGui = config.airlock.guiGpu.enable && config.airlock.guiGpu.startOnBoot;
  bootPrerequisites =
    [ "airlock-setup-config.service" ]
    ++ lib.optional guardedGui "airlock-start-gui-gpu.service"
    ++ lib.optional config.airlock.qubes.startNetworkProvidersOnBoot.enable
      "airlock-start-network-providers.service"
    ++ lib.optional config.airlock.qubes.deferredSetupConfigOnBoot.enable
      "airlock-setup-config-deferred.service";
in
{
  options.airlock.machineDeployment = {
    enable = lib.mkEnableOption "persistent repository-owned machine deployments";
    profile = lib.mkOption {
      type = lib.types.str;
      default = "default";
    };
  };
  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ engine ];
    environment.etc."airlock/machine-profile".text = cfg.profile + "\n";
    systemd.services = lib.mkMerge [
      (lib.mkIf config.airlock.qubes.setupConfigOnBoot.enable {
        airlock-machine-autostart = {
          description = "Start declared autostart VMs after guarded bootstrap";
          wantedBy = [ "multi-user.target" ];
          requires = bootPrerequisites;
          after = bootPrerequisites;
          restartIfChanged = false;
          stopIfChanged = false;
          serviceConfig = {
            Type = "oneshot";
            RemainAfterExit = true;
            ExecStart = "${engine}/bin/airlock-machine-deploy boot-start${lib.optionalString guardedGui " --require-running sys-gui"}";
          };
        };
        airlock-setup-config.serviceConfig.ExecStart = lib.mkForce "${engine}/bin/airlock-machine-deploy boot-setup ${setup}/bin/airlock-setup-config --apply${
          lib.optionalString (
            config.airlock.qubes.setupConfigOnBoot.scope == "boot-critical"
          ) " --boot-critical"
        }";
      })
      (lib.mkIf config.airlock.qubes.deferredSetupConfigOnBoot.enable {
        airlock-setup-config-deferred.serviceConfig.ExecStart = lib.mkForce "${engine}/bin/airlock-machine-deploy boot-setup ${setup}/bin/airlock-setup-config --apply --deferred";
      })
    ];
  };
}
