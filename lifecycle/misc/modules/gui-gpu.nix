{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.airlock.guiGpu;
  gpu = cfg.device;
  checker = pkgs.callPackage ../tools/pkgs/airlock-gui-gpu-check { };
in
{
  options.airlock.guiGpu = {
    enable = lib.mkEnableOption "declarative physical sys-gui GPU ownership";
    startOnBoot = lib.mkEnableOption "one sys-gui start after validated bootstrap";
    device = lib.mkOption {
      default = null;
      description = "Machine-local GPU selection; declaring it alone does not reserve hardware.";
      type = lib.types.nullOr (
        lib.types.submodule {
          options = {
            bdf = lib.mkOption {
              type = lib.types.strMatching "0000:[0-9a-f]{2}:[0-1][0-9a-f]\\.[0-7]";
              description = "Host segment-zero PCI address used for early Xen reservation.";
            };
            port = lib.mkOption {
              type = lib.types.strMatching "[0-9a-f_.-]+";
              description = "Qubes PCI port from this machine's inventory, checked against bdf.";
            };
            vendor = lib.mkOption { type = lib.types.strMatching "[0-9a-f]{4}"; };
            device = lib.mkOption { type = lib.types.strMatching "[0-9a-f]{4}"; };
            driver = lib.mkOption {
              type = lib.types.enum [
                "amdgpu"
                "i915"
                "xe"
                "nouveau"
                "nvidia"
              ];
              description = "Dom0 graphics driver to exclude from the initrd and blacklist.";
            };
          };
        }
      );
    };
  };
  config = lib.mkMerge [
    (lib.mkIf (gpu != null) {
      # Also guard a switch back to the hybrid profile: its empty desired PCI
      # list does not erase Qubes' persistent assignment from the physical lab.
      environment.etc."airlock/gui-gpu.json".text = builtins.toJSON (gpu // { inherit (cfg) enable; });
    })
    (lib.mkIf cfg.enable (
      lib.mkMerge [
        {
          assertions = [
            {
              assertion = gpu != null;
              message = "airlock.guiGpu requires an explicit machine GPU selection.";
            }
            {
              assertion = gpu == null || !(builtins.elem gpu.driver config.boot.initrd.kernelModules);
              message = "Remove the selected GPU driver from the machine's explicit initrd.kernelModules when guiGpu is enabled.";
            }
            {
              assertion =
                config.virtualisation.qubes.dom0.enable
                && config.airlock.qubes.setupConfigOnBoot.enable
                && config.airlock.qubes.setupConfigOnBoot.scope == "all";
              message = "Physical sys-gui GPU assignment requires dom0 and full boot topology setup.";
            }
            {
              assertion =
                config.airlock.qubes.domains ? sys-gui
                && (config.airlock.qubes.domains.sys-gui.prefs.virt_mode or "") == "hvm"
                && (config.airlock.qubes.domains.sys-gui.features."video-model" or "") == "none"
                && !(config.airlock.qubes.domains.sys-gui.prefs.autostart or false);
              message = "Physical sys-gui requires HVM, video-model=none and autostart=false; use guiGpu.startOnBoot.";
            }
          ];
        }
        (lib.mkIf (gpu != null) {
          boot.blacklistedKernelModules = [ gpu.driver ];
          boot.initrd.kernelModules = [ "xen-pciback" ];
          boot.kernelParams = [
            "xen-pciback.hide=(${gpu.bdf})"
            "initcall_blacklist=sysfb_init"
            "video=efifb:off"
          ];
          virtualisation.xen.boot.params = [ "iommu=required" ];
          airlock.qubes.domains.sys-gui.devices.pci = [
            {
              backend = "dom0";
              id = gpu.port;
              mode = "required";
              match = "port";
              options = { };
            }
          ];
          environment.systemPackages = [ checker ];
          # No persistent qvm autostart link may bypass bootstrap validation.
          systemd.services."qubes-vm@sys-gui".enable = lib.mkForce false;
          systemd.services.airlock-start-gui-gpu = lib.mkIf cfg.startOnBoot {
            description = "Validate and start the physical sys-gui GPU once";
            wantedBy = [ "multi-user.target" ];
            requires = [
              "airlock-setup-config.service"
              "qubesd.service"
            ];
            after = [
              "airlock-setup-config.service"
              "qubesd.service"
            ];
            restartIfChanged = false;
            stopIfChanged = false;
            serviceConfig = {
              Type = "oneshot";
              RemainAfterExit = true;
              ExecStartPre = [ "${checker}/bin/airlock-gui-gpu-check --phase start" ];
              ExecStart = "${pkgs.python3.pkgs.qubes-core-admin-client}/bin/qvm-start sys-gui";
              # Never loop over GPU resets, restart the VM on exit, or hide failure.
              Restart = "no";
            };
          };
        })
      ]
    ))
  ];
}
