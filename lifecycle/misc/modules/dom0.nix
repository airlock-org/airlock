{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.airlock.dom0;
  imageCfg = config.airlock.image;
  qubeBootstrap = pkgs.callPackage ../../bootstrap/tools/pkgs/airlock-qube-bootstrap { };
  qubeGuiSmoke = pkgs.callPackage ../tools/pkgs/airlock-qube-gui-smoke { };
  qubesStorage = pkgs.python3.pkgs.callPackage ../../runtime/tools/pkgs/airlock-qubes-storage { };
  observeDom0 = pkgs.callPackage ../../runtime/tools/pkgs/airlock-observe { };
  qubesSmoke = pkgs.callPackage ../tools/pkgs/airlock-qubes-smoke { };
  qrexecDom0 = pkgs.python3.pkgs.qubes-core-qrexec.dom0;
  qubesAdminClient = pkgs.python3.pkgs.qubes-core-admin-client;
  dom0DevPackages = with pkgs; [
    codex
    git
    zellij
  ];
  devNetworkProxy = cfg.devModeNetworkProxy;
  devNetworkProxyListen = "${devNetworkProxy.listenAddress}:${toString devNetworkProxy.port}";
  devNetworkProxyUrl = "http://${devNetworkProxyListen}";
  sysGuiVisibleSystemDomains = lib.filter (
    name: lib.hasPrefix "sys-" name
  ) (builtins.attrNames config.airlock.qubes.domains);
  devNetworkProxyEnv = {
    http_proxy = devNetworkProxyUrl;
    https_proxy = devNetworkProxyUrl;
    HTTP_PROXY = devNetworkProxyUrl;
    HTTPS_PROXY = devNetworkProxyUrl;
    no_proxy = "127.0.0.1,localhost,::1";
    NO_PROXY = "127.0.0.1,localhost,::1";
  };
  dom0DevNetworkProxyPolicyPackage = pkgs.runCommand "airlock-dom0-network-proxy-policy" { } ''
    install -d "$out/etc/qubes/policy.d"
    cat > "$out/etc/qubes/policy.d/35-airlock-dom0-network-proxy.policy" <<'EOF'
    # Managed by Airlock. Let dev-mode dom0 reach the updates proxy through its configured network qube.
    qubes.UpdatesProxy * dom0 ${devNetworkProxy.target} allow
    EOF
  '';
  dom0PoweroffService = pkgs.writeShellScript "airlock-dom0-poweroff" ''
    set -eu

    # qrexec policy is the authority boundary; retain this check so a future
    # policy mistake cannot turn the handler into a general dom0 power service.
    [ "''${QREXEC_REMOTE_DOMAIN:-}" = "sys-gui" ] || {
      echo "airlock.Dom0Poweroff is restricted to sys-gui" >&2
      exit 1
    }

    exec ${pkgs.systemd}/bin/systemctl poweroff
  '';
  sysGuiPolicyPackage = pkgs.runCommand "airlock-sys-gui-policy" { } ''
    install -d "$out/etc/qubes/policy.d"
    cat > "$out/etc/qubes/policy.d/50-airlock-sys-gui.policy" <<'EOF'
    # Based on Qubes' qvm/template-gui.jinja for the hybrid sys-gui domain.
    policy.EvalGUI +qubes.ClipboardPaste sys-gui dom0 allow
    qubes.GetImageRGBA * sys-gui @tag:guivm-sys-gui allow
    qubes.GetAppmenus * sys-gui @tag:guivm-sys-gui allow
    qubes.GetAppmenus * sys-gui @type:TemplateVM allow
    # The display-manager session enters the GUI domain through this RPC.
    # Keep it limited to dom0 starting the declared sys-gui domain.
    qubes.GuiVMSession * dom0 sys-gui allow
    qubes.GuiVMSession * @anyvm @anyvm deny
    # The GUI domain may request one host action: a clean power-off. It has no
    # shell or general systemd authority in dom0.
    airlock.Dom0Poweroff * sys-gui dom0 allow
    qubes.ClipboardCopy + sys-gui @tag:guivm-sys-gui allow
    qubes.ClipboardPaste + sys-gui @tag:guivm-sys-gui allow
    qubes.SetMonitorLayout * sys-gui @tag:guivm-sys-gui allow
    qubes.StartApp * sys-gui @tag:guivm-sys-gui allow
    qubes.StartApp * sys-gui @dispvm:@tag:guivm-sys-gui allow
    qubes.SyncAppMenus * @tag:guivm-sys-gui dom0 allow target=sys-gui
    qubes.WindowIconUpdater * @tag:guivm-sys-gui dom0 allow target=sys-gui
    qubes.Notifications * @tag:guivm-sys-gui @default allow target=sys-gui autostart=no
    qubes.WaitForSession * sys-gui @tag:guivm-sys-gui allow
    admin.vm.List * sys-gui dom0 allow
    admin.vm.List * sys-gui @tag:guivm-sys-gui allow target=dom0
    admin.Events * sys-gui dom0 allow
    admin.Events * sys-gui @tag:guivm-sys-gui allow target=dom0
    # System domains do not carry the app-VM GUI tag. Make their presence and
    # state visible to Qubes Manager without exposing every domain globally.
    ${lib.concatMapStringsSep "\n" (domain: ''
      admin.vm.List * sys-gui ${domain} allow target=dom0
      admin.Events * sys-gui ${domain} allow target=dom0
    '') sysGuiVisibleSystemDomains}
    admin.label.Get * sys-gui dom0 allow
    admin.label.Index * sys-gui dom0 allow
    admin.vm.feature.Set +keyboard-layout sys-gui sys-gui allow target=dom0
    admin.vm.property.Get * sys-gui dom0 allow
    admin.vm.volume.List * sys-gui dom0 allow
    admin.vm.device.pci.Available * sys-gui dom0 allow
    admin.vm.device.mic.Available * sys-gui dom0 allow
    admin.vm.feature.Get +internal sys-gui dom0 allow
    admin.vm.feature.Get +servicevm sys-gui dom0 allow
    admin.vm.CurrentState * sys-gui dom0 allow

    # Desktop management uses the Admin API, never a dom0 shell. Reads of
    # global metadata are explicit; host writes remain denied.
    admin.property.List * sys-gui dom0 allow
    admin.property.Get * sys-gui dom0 allow
    admin.property.GetAll * sys-gui dom0 allow
    admin.property.GetDefault * sys-gui dom0 allow
    admin.vm.property.List * sys-gui dom0 allow
    admin.vm.property.GetAll * sys-gui dom0 allow
    admin.vm.property.GetDefault * sys-gui dom0 allow
    admin.label.List * sys-gui dom0 allow
    admin.pool.List * sys-gui dom0 allow
    admin.pool.Info * sys-gui dom0 allow
    admin.pool.UsageDetails * sys-gui dom0 allow
    admin.pool.ListDrivers * sys-gui dom0 allow
    admin.vmclass.List * sys-gui dom0 allow
    # Creation is a global API call, but only these VM classes are permitted.
    ${lib.concatMapStringsSep "\n" (class: ''
      admin.vm.Create.${class} * sys-gui dom0 allow
      admin.vm.CreateInPool.${class} * sys-gui dom0 allow
    '') [ "AppVM" "StandaloneVM" "TemplateVM" ]}
    * * sys-gui dom0 deny

    # VM Manager and per-VM settings. In particular there is no wildcard
    # admin permission, no policy editing, and no host device assignment.
    # Per-VM Admin API requests are addressed by their VM name, but qubesd
    # itself runs in dom0. Redirect each explicit operation there; without the
    # redirect Qubes Manager cannot read even a VM's qid and exits at startup.
    ${lib.concatMapStringsSep "\n" (method: "${method} * sys-gui @anyvm allow target=dom0") [
      "admin.vm.CurrentState"
      "admin.vm.property.List"
      "admin.vm.property.Get"
      "admin.vm.property.GetAll"
      "admin.vm.property.Help"
      "admin.vm.property.GetDefault"
      "admin.vm.property.Set"
      "admin.vm.property.Reset"
      "admin.vm.feature.List"
      "admin.vm.feature.Get"
      "admin.vm.feature.CheckWithTemplate"
      "admin.vm.feature.CheckWithNetvm"
      "admin.vm.feature.CheckWithAdminVM"
      "admin.vm.feature.Set"
      "admin.vm.feature.Remove"
      "admin.vm.tag.List"
      "admin.vm.volume.List"
      "admin.vm.volume.Info"
      "admin.vm.volume.ListSnapshots"
      "admin.vm.volume.Resize"
      "admin.vm.firewall.Get"
      "admin.vm.firewall.Set"
      "admin.vm.Start"
      "admin.vm.Shutdown"
      "admin.vm.Kill"
      "admin.vm.Pause"
      "admin.vm.Unpause"
      "admin.vm.Remove"
    ]}
    * * sys-gui @anyvm deny
    EOF
  '';
  sysGuiSessionPackage =
    pkgs.runCommand "airlock-sys-gui-session"
      {
        passthru.providedSessions = [ "sys-gui" ];
      }
      ''
        install -d "$out/share/xsessions"
        cat > "$out/share/xsessions/sys-gui.desktop" <<EOF
        [Desktop Entry]
        Name=GUI Domain (sys-gui)
        Exec=${qubesAdminClient}/bin/qubes-guivm-session sys-gui
        Type=Application
        EOF
      '';
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
  imports = [
    ./qubes-input-proxy.nix
    ./amd-vbios.nix
    ./gui-gpu.nix
  ];
  # Upstream installs these desktop frontends for every dom0. Keep the
  # backend/CLI packages, including desktop-linux-common's RPC helpers, but
  # expose the graphical applications only in the GUI domain.
  options.environment.systemPackages = lib.mkOption {
    apply = packages: lib.filter (package:
      !(cfg.enable && cfg.gui.enable && builtins.elem (package.pname or (lib.getName package)) [
        "qubes-manager"
        "qubes-desktop-linux-manager"
        "qubes-artwork"
      ])
    ) packages;
  };

  options.airlock.image.devMode.enable = lib.mkOption {
    type = lib.types.bool;
    default = false;
    description = ''
      Build this whole Airlock OS image as a development image.
      This is a build-time image selection flag, not a runtime mode.
    '';
  };

  options.airlock.dom0 = {
    enable = lib.mkEnableOption "the Airlock Qubes-on-NixOS dom0 proof of concept";

    user = lib.mkOption {
      type = lib.types.str;
      default = "airlock";
      description = ''
        Local user that receives access to the Qubes control group.
        Qubes dom0 is effectively a single-user control environment.
      '';
    };

    gui.enable = lib.mkEnableOption "Qubes GUI integration in dom0";

    observability.enable = lib.mkOption {
      type = lib.types.bool;
      default = imageCfg.devMode.enable;
      defaultText = lib.literalExpression "config.airlock.image.devMode.enable";
      description = ''
        Install the development-only airlock-observe-dom0 command.
        It reports Xen, systemd, and Qubes control-stack state as JSON.
        Defaults to the image-level development mode flag.
      '';
    };

    testingTools.enable = lib.mkOption {
      type = lib.types.bool;
      default = imageCfg.devMode.enable;
      defaultText = lib.literalExpression "config.airlock.image.devMode.enable";
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

    devModeNetworkProxy = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = imageCfg.devMode.enable;
        defaultText = lib.literalExpression "config.airlock.image.devMode.enable";
        description = ''
          In development images, expose a dom0 loopback HTTP proxy that
          forwards requests over qrexec to the selected network qube.
        '';
      };

      target = lib.mkOption {
        type = lib.types.str;
        default = "sys-firewall";
        description = "Network qube that serves qubes.UpdatesProxy for dev-mode dom0.";
      };

      listenAddress = lib.mkOption {
        type = lib.types.str;
        default = "127.0.0.1";
        description = "Loopback address where dom0 listens for the development proxy.";
      };

      port = lib.mkOption {
        type = lib.types.port;
        default = 8082;
        description = "Loopback TCP port where dom0 listens for the development proxy.";
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
        message = "airlock.dom0.user must name the local Qubes control user.";
      }
      {
        assertion = cfg.dom0Resources.memoryMiB > 0;
        message = "airlock.dom0.dom0Resources.memoryMiB must be greater than zero.";
      }
      {
        assertion = cfg.dom0Resources.maxMemoryMiB >= cfg.dom0Resources.memoryMiB;
        message = "airlock.dom0.dom0Resources.maxMemoryMiB must be >= memoryMiB.";
      }
      {
        assertion = !devNetworkProxy.enable || devNetworkProxy.target != "";
        message = "airlock.dom0.devModeNetworkProxy.target must not be empty when the proxy is enabled.";
      }
      {
        assertion = !devNetworkProxy.enable || lib.match "[A-Za-z0-9_.-]+" devNetworkProxy.target != null;
        message = "airlock.dom0.devModeNetworkProxy.target must be a simple Qubes domain name.";
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
    services.xserver.desktopManager.xfce.enable = lib.mkIf cfg.gui.enable false;
    services.xserver.desktopManager.xterm.enable = lib.mkIf cfg.gui.enable false;
    # Qubes GUI needs shmoverride preloaded in the Xorg server. The NixOS
    # display-manager module sets xserverBin at normal priority, so this must
    # be stronger than a default while GUI support is enabled.
    services.xserver.displayManager.xserverBin = lib.mkIf cfg.gui.enable (
      lib.mkOverride 50 xorgQubesShmoverride
    );
    services.displayManager.sessionPackages = lib.optional cfg.gui.enable sysGuiSessionPackage;
    services.displayManager.defaultSession = lib.mkIf cfg.gui.enable (lib.mkOverride 40 "sys-gui");
    # This is a display relay. Authentication and the login UI live in sys-gui.
    services.displayManager.autoLogin = lib.mkIf cfg.gui.enable {
      enable = true;
      user = cfg.user;
    };
    services.xserver.displayManager.lightdm = lib.mkIf cfg.gui.enable {
      greeter.enable = false;
      greeters.gtk.enable = false;
      autoLogin.timeout = 0;
    };
    systemd.services.display-manager.after = lib.optional cfg.gui.enable "airlock-setup-config.service";
    airlock.qubes.setupConfigOnBoot.statusDevice = lib.mkIf cfg.gui.enable (lib.mkDefault "/dev/tty1");

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

    # The upstream Xen builder runs first and changes loader.conf to select the
    # matching xen-* entry. Keep the ordinary NixOS entry as an explicit
    # recovery path rather than something that looks suitable for Qubes use.
    boot.loader.systemd-boot.extraInstallCommands = lib.mkAfter ''
      # extraInstallCommands is emitted as its own strict shell script, so it
      # cannot use the efiMountPoint shell variable from systemd-boot's
      # installer.  Interpolate the configured mount point here instead.
      for entry in "${config.boot.loader.efi.efiSysMountPoint}"/loader/entries/nixos-*.conf; do
        [ -f "$entry" ] || continue
        ${pkgs.gnugrep}/bin/grep -Fq "(Recovery: without Xen)" "$entry" || \
          ${pkgs.gnused}/bin/sed -i \
            's/^title \(.*\)$/title \1 (Recovery: without Xen)/' \
            "$entry"
      done
    '';

    virtualisation.libvirtd.firewallBackend = lib.mkForce cfg.libvirtFirewallBackend;
    # Qubes owns VM startup. Libvirt's defaults save running guests at host
    # shutdown and resume them on the next boot regardless of autostart flags,
    # racing topology reconciliation (including USB profile changes).
    virtualisation.libvirtd.onBoot = "ignore";
    virtualisation.libvirtd.onShutdown = "shutdown";

    systemd.services.airlock-qubes-store-init = {
      description = "Initialize the Qubes XML store for Airlock dom0";
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
      after = [ "airlock-qubes-store-init.service" ];
      requires = [ "airlock-qubes-store-init.service" ];
      environment.PYTHONPATH =
        "${qubesStorage}/${pkgs.python3.sitePackages}"
        + lib.optionalString config.airlock.amdVbios.enable ":${config.airlock.amdVbios.package}/${pkgs.python3.sitePackages}";
      path = [
        pkgs.erofs-utils
        pkgs.lvm2
        pkgs.sudo
        pkgs.util-linux
      ];
    };

    systemd.tmpfiles.rules = [
      "d /var/lib/airlock 0700 root root - -"
      "d /var/lib/airlock/closure-store 0700 root root - -"
    ];

    environment.etc."qubes-rpc/airlock.Dom0Poweroff" = lib.mkIf cfg.gui.enable {
      source = dom0PoweroffService;
      mode = "0555";
    };

    networking.useDHCP = lib.mkDefault imageCfg.devMode.enable;
    networking.dhcpcd.extraConfig = lib.mkIf devNetworkProxy.enable (
      lib.mkAfter ''
        nogateway
      ''
    );

    virtualisation.qubes.packages =
      lib.optionals devNetworkProxy.enable [
        dom0DevNetworkProxyPolicyPackage
      ]
      ++ lib.optional cfg.gui.enable sysGuiPolicyPackage;

    systemd.sockets.airlock-dom0-updates-proxy = lib.mkIf devNetworkProxy.enable {
      description = "Listen for dev-mode dom0 traffic to the updates proxy";
      wantedBy = [ "sockets.target" ];
      listenStreams = [ devNetworkProxyListen ];
      socketConfig = {
        Accept = true;
        BindToDevice = "lo";
      };
    };

    systemd.services."airlock-dom0-updates-proxy@" = lib.mkIf devNetworkProxy.enable {
      description = "Forward a dev-mode dom0 proxy connection to the network qube";
      wants = [
        "airlock-start-network-providers.service"
        "qubes-qrexec-policy-daemon.service"
        "qubesd.service"
      ];
      after = [
        "airlock-start-network-providers.service"
        "qubes-qrexec-policy-daemon.service"
        "qubesd.service"
      ];
      serviceConfig = {
        ExecStart = "${qrexecDom0}/bin/qrexec-client ${
          lib.escapeShellArgs [
            "--use-stdin-socket"
            "-d"
            devNetworkProxy.target
            "DEFAULT:QUBESRPC qubes.UpdatesProxy+ dom0"
          ]
        }";
        StandardInput = "socket";
        StandardOutput = "inherit";
        StandardError = "journal";
      };
    };

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

    environment.etc."airlock/dev-mode" = lib.mkIf imageCfg.devMode.enable {
      text = ''
        enabled
      '';
    };

    environment.sessionVariables = lib.mkIf devNetworkProxy.enable devNetworkProxyEnv;
    systemd.services.nix-daemon.environment = lib.mkIf devNetworkProxy.enable devNetworkProxyEnv;
  };
}
