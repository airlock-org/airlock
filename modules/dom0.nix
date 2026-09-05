{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.nixome.dom0;
  observeDom0 = pkgs.callPackage ../pkgs/nixome-observe { };
  qubesSmoke = pkgs.callPackage ../pkgs/nixome-qubes-smoke { };

  baselineOptOuts = [
    # Upstream requires these because dedicated sys-net/sys-usb are not ready.
    "dedicated-sys-net"
    "dedicated-sys-usb"
  ]
  ++ lib.optional (!cfg.gui.enable) "dom0-is-gui";
in
{
  options.nixome.dom0 = {
    enable = lib.mkEnableOption "the Nixome Qubes-on-NixOS dom0 proof of concept";

    user = lib.mkOption {
      type = lib.types.str;
      default = "nixome";
      description = ''
        Local user that receives access to the Qubes control group.
        Qubes dom0 is effectively a single-user control environment.
      '';
    };

    gui.enable = lib.mkEnableOption "Qubes GUI integration in dom0";

    observability.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Install the development-only nixome-observe-dom0 command.
        It reports Xen, systemd, and Qubes control-stack state as JSON.
      '';
    };

    dom0Resources = {
      memoryMiB = lib.mkOption {
        type = lib.types.ints.unsigned;
        default = 4096;
        description = "Initial Xen dom0 memory allocation in MiB.";
      };

      maxMemoryMiB = lib.mkOption {
        type = lib.types.ints.unsigned;
        default = cfg.dom0Resources.memoryMiB;
        description = "Maximum Xen dom0 memory allocation in MiB.";
      };
    };

    extraOptOutRecommendedConfiguration = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = ''
        Extra entries for virtualisation.qubes.optOutRecommendedConfiguration.
        Keep this empty unless a target machine needs a documented exception.
      '';
    };

    libvirtFirewallBackend = lib.mkOption {
      type = lib.types.enum [
        "iptables"
        "nftables"
      ];
      default = "iptables";
      description = ''
        Compatibility backend for nixpkgs' libvirtd module.
        nixos-qubes currently sets "none", which current nixpkgs rejects.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.user != "";
        message = "nixome.dom0.user must name the local Qubes control user.";
      }
      {
        assertion = cfg.dom0Resources.memoryMiB > 0;
        message = "nixome.dom0.dom0Resources.memoryMiB must be greater than zero.";
      }
      {
        assertion = cfg.dom0Resources.maxMemoryMiB >= cfg.dom0Resources.memoryMiB;
        message = "nixome.dom0.dom0Resources.maxMemoryMiB must be >= memoryMiB.";
      }
    ];

    nixpkgs.config.allowUnfreePredicate = lib.mkDefault (
      pkg:
      builtins.elem (pkg.pname or (lib.getName pkg)) [
        "qubes-artwork"
        "qubes-vmm-stubdom-linux"
      ]
    );

    boot.initrd.systemd.enable = lib.mkDefault true;

    virtualisation.qubes = {
      dom0.enable = true;
      user = cfg.user;

      # Upstream currently asserts this false. Track the gap explicitly rather
      # than pretending the full Qubes OS security posture is implemented.
      secure = false;

      optOutRecommendedConfiguration = lib.mkBefore (
        baselineOptOuts ++ cfg.extraOptOutRecommendedConfiguration
      );
    };

    virtualisation.xen = {
      boot.builderVerbosity = lib.mkDefault "info";
      dom0Resources = {
        memory = lib.mkDefault cfg.dom0Resources.memoryMiB;
        maxMemory = lib.mkDefault cfg.dom0Resources.maxMemoryMiB;
      };
    };

    virtualisation.libvirtd.firewallBackend = lib.mkForce cfg.libvirtFirewallBackend;

    environment.systemPackages = lib.optionals cfg.observability.enable [
      observeDom0
      qubesSmoke
    ];
  };
}
