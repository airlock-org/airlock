{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.nixome.dom0;
  imageCfg = config.nixome.image;
  qubeBootstrap = pkgs.callPackage ../pkgs/nixome-qube-bootstrap { };
  qubeGuiSmoke = pkgs.callPackage ../pkgs/nixome-qube-gui-smoke { };
  qubesStorage = pkgs.python3.pkgs.callPackage ../pkgs/nixome-qubes-storage { };
  observeDom0 = pkgs.callPackage ../pkgs/nixome-observe { };
  qubesSmoke = pkgs.callPackage ../pkgs/nixome-qubes-smoke { };
  dom0DevPackages = with pkgs; [
    codex
    git
    zellij
  ];
  xorgQubesShmoverride = pkgs.writeShellScript "X-qubes-shmoverride" ''
    export LD_PRELOAD=${pkgs.qubes-gui-daemon}/lib/qubes-gui-daemon/shmoverride.so''${LD_PRELOAD:+:$LD_PRELOAD}
    exec ${pkgs.xorg-server}/bin/X "$@"
  '';

  baselineOptOuts = [
    # Upstream requires these because dedicated sys-net/sys-usb are not ready.
    "dedicated-sys-net"
    "dedicated-sys-usb"
  ]
  ++ lib.optional (!cfg.gui.enable) "dom0-is-gui";
in
{
  options.nixome.image.devMode.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      Build this whole Nixome OS image as a development image.
      This is a build-time image selection flag, not a runtime mode.
    '';
  };

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
      default = imageCfg.devMode.enable;
      defaultText = lib.literalExpression "config.nixome.image.devMode.enable";
      description = ''
        Install the development-only nixome-observe-dom0 command.
        It reports Xen, systemd, and Qubes control-stack state as JSON.
        Defaults to the image-level development mode flag.
      '';
    };

    testingTools.enable = lib.mkOption {
      type = lib.types.bool;
      default = imageCfg.devMode.enable;
      defaultText = lib.literalExpression "config.nixome.image.devMode.enable";
      description = ''
        Install development-only bootstrap and smoke-test commands.
        These commands may create or modify Qubes runtime state.
        Defaults to the image-level development mode flag.
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
    time.timeZone = lib.mkDefault "UTC";
    # Qubes GUI needs shmoverride preloaded in the Xorg server. The NixOS
    # display-manager module sets xserverBin at normal priority, so this must
    # be stronger than a default while GUI support is enabled.
    services.xserver.displayManager.xserverBin = lib.mkIf cfg.gui.enable (
      lib.mkOverride 50 xorgQubesShmoverride
    );

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

    systemd.services.nixome-qubes-store-init = {
      description = "Initialize the Qubes XML store for Nixome dom0";
      after = [ "systemd-tmpfiles-setup.service" ];
      before = [ "qubesd.service" ];
      requiredBy = [ "qubesd.service" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      script = ''
        if [ ! -f /var/lib/qubes/qubes.xml ]; then
          echo "setting up Qubes XML store..."
          ${pkgs.python3.pkgs.qubes-core-admin}/bin/qubes-create --offline-mode
        fi
      '';
    };

    systemd.services.qubesd = {
      after = [ "nixome-qubes-store-init.service" ];
      requires = [ "nixome-qubes-store-init.service" ];
      environment.PYTHONPATH = lib.mkBefore "${qubesStorage}/${pkgs.python3.sitePackages}";
      path = [
        pkgs.erofs-utils
        pkgs.lvm2
        pkgs.util-linux
      ];
    };

    systemd.tmpfiles.rules = [
      "d /var/lib/nixome 0700 root root - -"
      "d /var/lib/nixome/closure-store 0700 root root - -"
    ];

    networking.useDHCP = lib.mkDefault imageCfg.devMode.enable;

    environment.systemPackages = [
      qubesStorage
    ]
    ++ lib.optionals cfg.observability.enable [
      observeDom0
    ]
    ++ lib.optionals cfg.testingTools.enable [
      qubeBootstrap
      qubeGuiSmoke
      qubesSmoke
    ]
    ++ lib.optionals imageCfg.devMode.enable dom0DevPackages;

    environment.etc = lib.mkIf imageCfg.devMode.enable {
      "nixome/dev-mode".text = ''
        enabled
      '';
    };
  };
}
