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
    bootFramebuffer.enable = lib.mkEnableOption "experimental temporary firmware console, detached before the sole GPU guest starts";
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
              assertion = !cfg.bootFramebuffer.enable || cfg.startOnBoot;
              message = "The boot framebuffer requires ordered GPU startup and handoff.";
            }
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
          ]
          ++ lib.optionals (!cfg.bootFramebuffer.enable) [
            "initcall_blacklist=sysfb_init"
            "video=efifb:off"
          ]
          ++ lib.optionals cfg.bootFramebuffer.enable [
            "fbcon=nodefer"
          ];
          # Do not use vga=...,keep: Xen must relinquish its early console.
          virtualisation.xen.boot.params = [
            "iommu=required"
          ]
          ++ lib.optional cfg.bootFramebuffer.enable "vga=current";
          boot.initrd.availableKernelModules = lib.optionals cfg.bootFramebuffer.enable [ "simpledrm" ];
          airlock.qubes.setupConfigOnBoot.statusDevice = lib.mkIf cfg.bootFramebuffer.enable "/dev/tty1";
          systemd.services."getty@tty1".enable = lib.mkIf cfg.bootFramebuffer.enable false;
          systemd.services.airlock-setup-config.serviceConfig.ExecStartPre =
            lib.mkIf cfg.bootFramebuffer.enable
              [ "-${pkgs.kbd}/bin/chvt 1" ];
          # A physical-GPU profile must win over a prepared domain descriptor.
          # Otherwise a descriptor's empty PCI list leaves sys-gui running with
          # no display device after the boot framebuffer is released.
          airlock.qubes.domains.sys-gui.devices.pci = lib.mkForce [
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
            requires = [ "airlock-setup-config.service" ];
            # Bootstrap restarts qubesd after adopting VM storage. Requires
            # would propagate that stop and cancel our still-pending start.
            # Keep ordering/pull-in, then check daemon readiness after setup.
            wants = [ "qubesd.service" ];
            after = [
              "airlock-setup-config.service"
              "qubesd.service"
            ];
            restartIfChanged = false;
            stopIfChanged = false;
            serviceConfig = {
              Type = "oneshot";
              RemainAfterExit = true;
              ExecStartPre = [
                "${pkgs.systemd}/bin/systemctl is-active --quiet qubesd.service"
                "${checker}/bin/airlock-gui-gpu-check --phase start"
              ]
              ++ lib.optional cfg.bootFramebuffer.enable "${pkgs.python3}/bin/python3 ${../tools/scripts/boot-framebuffer.py} --gpu ${gpu.bdf} --report /var/log/airlock/boot-framebuffer-handoff.json";
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
