{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.nixome.qubeGuest;
  qubesAgentLinux = pkgs.qubes-core-agent-linux;

  qubesGuestPath = [
    pkgs.coreutils
    pkgs.diffutils
    pkgs.e2fsprogs
    pkgs.findutils
    pkgs.gawk
    pkgs.gnugrep
    pkgs.kmod
    pkgs.qubes-core-qubesdb
    pkgs.qubes-linux-utils
    pkgs.systemd
    pkgs.util-linux
  ];

  rolePackages = {
    base = [ ];
    config-vault = [
      configVaultInit
      pkgs.git
    ];
    agent = [
      agentFetchConfig
      pkgs.git
      pkgs.jq
      pkgs.nix
    ];
  };

  configVaultInit = pkgs.writeShellScriptBin "nixome-config-vault-init" ''
        set -eu

        repo="${cfg.configRepoPath}"
        if [ -d "$repo/.git" ]; then
          printf 'config repo already exists: %s\n' "$repo"
          exit 0
        fi

        mkdir -p "$repo"
        cd "$repo"
        ${pkgs.git}/bin/git init
        cat > README.md <<'EOF'
    # Nixome Machine Configuration

    This repository is intended to be the authoritative Git state for this
    machine's Nixome configuration.
    EOF
        ${pkgs.git}/bin/git add README.md
        ${pkgs.git}/bin/git \
          -c user.name='Nixome Config Vault' \
          -c user.email='nixome-config-vault@localhost' \
          commit -m 'init nixome config repository'
  '';

  configExportService = pkgs.writeShellScript "nixome-config-export-qrexec" ''
    set -eu

    repo="${cfg.configRepoPath}"
    if [ ! -d "$repo/.git" ]; then
      printf 'missing Git repo at %s; run nixome-config-vault-init first\n' "$repo" >&2
      exit 1
    fi

    cd "$repo"
    exec ${pkgs.git}/bin/git bundle create - --branches --tags
  '';

  agentFetchConfig = pkgs.writeShellScriptBin "nixome-agent-fetch-config" ''
    set -eu

    target="''${1:-sys-config-vault}"
    dest="''${2:-$HOME/nixome-config-readonly}"
    tmpdir="$(mktemp -d)"
    bundle="$tmpdir/nixome-config.bundle"

    ${pkgs.python3.pkgs.qubes-core-qrexec.domU}/bin/qrexec-client-vm \
      "$target" nixome.Config.Export > "$bundle"

    if [ -e "$dest" ]; then
      printf 'destination already exists: %s\n' "$dest" >&2
      exit 1
    fi

    ${pkgs.git}/bin/git clone "$bundle" "$dest"
    chmod -R a-w "$dest"
    printf 'read-only config snapshot written to %s\n' "$dest"
  '';
in
{
  options.nixome.qubeGuest = {
    enable = lib.mkEnableOption "a Nixome NixOS Qubes guest template";

    role = lib.mkOption {
      type = lib.types.enum [
        "base"
        "config-vault"
        "agent"
      ];
      default = "base";
      description = "Nixome role-specific userspace to place in the guest template.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "user";
      description = "Primary unprivileged user in the guest template.";
    };

    configRepoPath = lib.mkOption {
      type = lib.types.str;
      default = "/home/${cfg.user}/nixome-config";
      description = "Authoritative config repository path in the config-vault role.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.user != "";
        message = "nixome.qubeGuest.user must not be empty.";
      }
      {
        assertion = lib.hasPrefix "/home/" cfg.configRepoPath;
        message = "nixome.qubeGuest.configRepoPath should live under /home for Qubes private storage.";
      }
    ];

    boot.loader.grub.enable = false;
    boot.initrd.systemd.enable = lib.mkDefault true;
    boot.initrd.availableKernelModules = [
      "xen-blkfront"
      "xen-fbfront"
      "xen-kbdfront"
      "xen-netfront"
      "xen-pcifront"
      "xen-scsifront"
      "xen-tpmfront"
    ];
    boot.initrd.kernelModules = [
      "xen-blkfront"
      "xen-netfront"
      "dm_mod"
      "dm_snapshot"
    ];
    boot.kernelParams = [
      "console=hvc0"
    ];

    fileSystems."/" = {
      device = "/dev/mapper/dmroot";
      fsType = "ext4";
    };

    networking.hostName = lib.mkDefault "nixome-qube";
    networking.useDHCP = lib.mkDefault true;

    users.groups.qubes = { };
    users.users.${cfg.user} = {
      isNormalUser = true;
      group = "users";
      extraGroups = [ "qubes" ];
    };

    systemd.tmpfiles.packages = [
      qubesAgentLinux
      pkgs.qubes-linux-utils
    ];

    systemd.packages = [
      pkgs.qubes-core-qubesdb.daemon
      pkgs.python3.pkgs.qubes-core-qrexec.domU
      pkgs.qubes-linux-utils
    ];

    systemd.services.qubes-sysinit = {
      enable = true;
      wantedBy = [ "sysinit.target" ];
      after = [
        "systemd-modules-load.service"
        "qubes-db.service"
      ];
      requires = [ "qubes-db.service" ];
      before = [ "sysinit.target" ];
      path = qubesGuestPath;
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${qubesAgentLinux}/lib/qubes/init/qubes-sysinit.sh";
      };
      unitConfig.DefaultDependencies = false;
    };

    systemd.services.nixome-qubes-rw-init = {
      description = "Initialize Qubes private storage mounts for NixOS guests";
      wantedBy = [ "multi-user.target" ];
      wants = [ "dev-xvdb.device" ];
      after = [
        "dev-xvdb.device"
        "qubes-sysinit.service"
      ];
      requires = [ "qubes-sysinit.service" ];
      before = [
        "systemd-user-sessions.service"
        "qubes-qrexec-agent.service"
      ];
      path = qubesGuestPath;
      script = ''
        set -eu

        mkdir -p /rw /home /usr/local

        if [ ! -e /dev/xvdb ]; then
          echo "Qubes private device /dev/xvdb is not present" >&2
          exit 1
        fi

        ${qubesAgentLinux}/lib/qubes/init/setup-rwdev.sh

        if ! mountpoint -q /rw; then
          mount -t auto -o defaults,discard,nosuid,nodev /dev/xvdb /rw
        fi

        ${qubesAgentLinux}/lib/qubes/init/setup-rw.sh

        . ${qubesAgentLinux}/lib/qubes/init/functions
        initialize_home /rw/home ifneeded

        if ! mountpoint -q /home; then
          mount --bind /rw/home /home
          mount -o remount,bind,nosuid,nodev /home
        fi

        mkdir -p /rw/usrlocal
        if ! mountpoint -q /usr/local; then
          mount --bind /rw/usrlocal /usr/local
        fi
      '';
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
    };

    systemd.services.qubes-db = {
      enable = true;
      wantedBy = [ "sysinit.target" ];
      after = [ "systemd-modules-load.service" ];
    };

    systemd.services.qubes-qrexec-agent = {
      enable = true;
      wantedBy = [ "multi-user.target" ];
      wants = [ "nixome-qubes-rw-init.service" ];
      after = [
        "nixome-qubes-rw-init.service"
        "systemd-user-sessions.service"
      ];
      aliases = [ "qubes-core-agent.service" ];
    };

    systemd.services.qubes-meminfo-writer = {
      enable = true;
      wantedBy = [ "multi-user.target" ];
      before = [ "systemd-user-sessions.service" ];
    };

    environment.etc."qubes-rpc/nixome.Config.Export" = lib.mkIf (cfg.role == "config-vault") {
      source = configExportService;
      mode = "0555";
    };

    environment.systemPackages = [
      pkgs.qubes-core-qubesdb
      qubesAgentLinux
      pkgs.qubes-linux-utils
      pkgs.python3.pkgs.qubes-core-qrexec
      pkgs.python3.pkgs.qubes-core-qrexec.domU
    ]
    ++ (rolePackages.${cfg.role} or [ ]);

    nix.settings.experimental-features = lib.mkIf (cfg.role == "agent") [
      "nix-command"
      "flakes"
    ];

    services.timesyncd.enable = false;
    system.stateVersion = "26.05";
  };
}
