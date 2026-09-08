{
  config,
  lib,
  pkgs,
  modulesPath,
  ...
}:
let
  cfg = config.nixome.minimalGuest;
  minimalOverride = lib.mkOverride 50;
  defaultOff = minimalOverride false;
  emptyHwdb = pkgs.runCommand "nixome-empty-hwdb.bin" { } ''
    touch "$out"
  '';
in
{
  imports = [
    "${modulesPath}/profiles/minimal.nix"
  ];

  options.nixome.minimalGuest = {
    enable = lib.mkEnableOption "the Nixome minimal closure-backed guest base";

    nix.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether this guest role carries explicit Nix CLI tooling.";
    };

    networking.enable = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Whether this guest role needs normal NixOS guest networking services.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.package = lib.mkDefault pkgs.systemdMinimal;

    environment.defaultPackages = minimalOverride [ ];
    environment.corePackages = minimalOverride [
      pkgs.bashInteractive
      pkgs.coreutils
      pkgs.findutils
      pkgs.gawk
      pkgs.gnugrep
      pkgs.gnused
      pkgs.gnutar
      pkgs.gzip
      pkgs.kmod
      pkgs.procps
      pkgs.shadow
      pkgs.systemdMinimal
      pkgs.util-linux
    ];

    documentation.enable = lib.mkDefault false;
    documentation.doc.enable = lib.mkDefault false;
    documentation.info.enable = lib.mkDefault false;
    documentation.man.enable = lib.mkDefault false;
    documentation.nixos.enable = lib.mkDefault false;

    system.etc.overlay.enable = lib.mkDefault true;
    services.userborn.enable = lib.mkDefault true;

    programs.command-not-found.enable = defaultOff;
    programs.bash.completion.enable = defaultOff;
    programs.less.enable = defaultOff;
    programs.nano.enable = defaultOff;

    environment.systemPackages = lib.mkIf cfg.nix.enable [
      pkgs.nix
    ];

    system.disableInstallerTools = minimalOverride true;

    nix.enable = defaultOff;
    nix.daemon.enable = defaultOff;
    nix.gc.automatic = defaultOff;
    nix.optimise.automatic = defaultOff;

    networking.useDHCP = minimalOverride cfg.networking.enable;
    networking.dhcpcd.enable = minimalOverride cfg.networking.enable;
    networking.firewall.enable = defaultOff;
    networking.resolvconf.enable = minimalOverride cfg.networking.enable;
    services.resolved.enable = defaultOff;

    boot.initrd.systemd.dbus.enable = defaultOff;
    boot.initrd.systemd.suppressedUnits = lib.mkAfter [
      "systemd-bsod.service"
    ];
    boot.initrd.systemd.suppressedStorePaths = lib.mkAfter [
      "${config.boot.initrd.systemd.package}/lib/systemd/systemd-bsod"
    ];
    boot.bcache.enable = defaultOff;
    boot.initrd.services.bcache.enable = defaultOff;
    boot.kexec.enable = defaultOff;
    services.fstrim.enable = defaultOff;
    services.lvm.enable = defaultOff;
    boot.initrd.services.lvm.enable = defaultOff;

    console.enable = defaultOff;
    fonts.fontconfig.enable = defaultOff;
    i18n.defaultLocale = lib.mkDefault "C.UTF-8";
    i18n.glibcLocales = minimalOverride null;
    security.sudo.enable = defaultOff;
    security.wrappers = lib.mkForce { };

    services.dbus.enable = defaultOff;
    environment.etc."udev/hwdb.bin".source = minimalOverride emptyHwdb;
    services.timesyncd.enable = defaultOff;
    systemd.coredump.enable = defaultOff;
    systemd.oomd.enable = defaultOff;
    systemd.shutdownRamfs.enable = defaultOff;
    systemd.tmpfiles.rules = lib.mkAfter [
      "d /root 0700 root root -"
    ];

    systemd.services.lastlog2-import.enable = defaultOff;
    systemd.services.console-getty.enable = defaultOff;
    systemd.services."container-getty@".enable = defaultOff;
    systemd.services."getty@".enable = defaultOff;
    systemd.services."serial-getty@".enable = defaultOff;
    systemd.services.linger-users.enable = defaultOff;
    systemd.services.nscd.enable = defaultOff;
    systemd.services.prepare-kexec.enable = defaultOff;
    systemd.services.post-boot.enable = defaultOff;
    systemd.services.sleep-actions.enable = defaultOff;
    systemd.services.suid-sgid-wrappers.enable = defaultOff;
    systemd.services.systemd-backlight.enable = defaultOff;
    systemd.services."systemd-backlight@".enable = defaultOff;
    systemd.services.systemd-hostnamed.enable = defaultOff;
    systemd.services.systemd-importd.enable = defaultOff;
    systemd.services.systemd-localed.enable = defaultOff;
    systemd.services."systemd-nspawn@".enable = defaultOff;
    systemd.services.systemd-pstore.enable = defaultOff;
    systemd.services.systemd-timedated.enable = defaultOff;

    system.fsPackages = minimalOverride [ ];
    boot.supportedFilesystems = lib.mkDefault {
      ext4 = true;
      erofs = true;
    };
  };
}
