{ config, lib, pkgs, modulesPath, ... }:
{
  imports = [
    "${modulesPath}/virtualisation/disk-image.nix"
    ../../bootstrap/gui-minimal.nix
    ./dev-mode.nix
  ];

  # Physical developer system: deliberately separate from vm.nix's shared
  # password, password SSH and passwordless sudo defaults.
  airlock.dom0 = {
    enable = true;
    user = "airlock";
    gui.enable = true;
    devModeNetworkProxy.enable = false;
    dom0Resources = { memoryMiB = 8192; maxMemoryMiB = 8192; };
  };
  users.users.airlock = {
    isNormalUser = true;
    uid = 1000;
    extraGroups = [ "wheel" "qubes" "networkmanager" ];
    hashedPasswordFile = "/var/lib/airlock/usb/local-password.hash";
  };
  users.users.root.hashedPassword = "!";
  security.sudo.wheelNeedsPassword = true;

  # Keep upstream's dom0 hostname: Qubes utilities can use it as the domain
  # identity. Give this device its distinct Tailscale name during enrollment.
  networking.useDHCP = false;
  networking.networkmanager.enable = true;
  networking.firewall.interfaces.tailscale0.allowedTCPPorts = [ 22 ];
  services.tailscale.enable = true;
  services.openssh = {
    enable = true;
    openFirewall = false;
    settings = {
      PasswordAuthentication = false;
      KbdInteractiveAuthentication = false;
      PermitRootLogin = "no";
      AuthorizedKeysFile = lib.mkForce ".ssh/authorized_keys";
    };
  };
  services.udisks2.enable = lib.mkForce false;
  services.gvfs.enable = lib.mkForce false;
  services.xserver.enable = true;
  services.xserver.displayManager.lightdm.enable = true;
  services.libinput.enable = true;

  boot.loader.systemd-boot.enable = true;
  boot.loader.systemd-boot.configurationLimit = 5;
  boot.loader.timeout = 10;
  boot.loader.efi.canTouchEfiVariables = false;
  # Never let boot-entry wildcard ordering select the hardware experiment.
  # Run after upstream's Xen entry builder and dom0's recovery title update.
  boot.loader.systemd-boot.extraInstallCommands = lib.mkOrder 1700 ''
    normal_entry="$(${pkgs.findutils}/bin/find ${config.boot.loader.efi.efiSysMountPoint}/loader/entries \
      -maxdepth 1 -type f -name 'xen-generation-*.conf' ! -name '*specialisation*' |
      ${pkgs.coreutils}/bin/sort -V | ${pkgs.coreutils}/bin/tail -n 1)"
    test -n "$normal_entry"
    normal_entry="$(${pkgs.coreutils}/bin/basename "$normal_entry")"
    ${pkgs.gnused}/bin/sed -i "s/^default .*/default $normal_entry/" \
      ${config.boot.loader.efi.efiSysMountPoint}/loader/loader.conf
  '';
  boot.initrd.systemd.enable = true;
  boot.kernelParams = [ "console=tty0" "console=hvc0" ];
  boot.growPartition = false;
  fileSystems."/".device = lib.mkForce "/dev/disk/by-label/airlock-usb";
  fileSystems."/".autoResize = lib.mkForce false;
  fileSystems."/boot".device = lib.mkForce "/dev/disk/by-label/AIRLOCK_EFI";
  image = { baseName = "airlock-usb-dev"; format = "raw"; efiSupport = true; };
  virtualisation.diskSize = 65536;
  system.build.image = lib.mkForce (import "${modulesPath}/../lib/make-disk-image.nix" {
    inherit config lib pkgs;
    inherit (config.image) baseName format;
    diskSize = config.virtualisation.diskSize;
    partitionTableType = "efi";
    bootSize = "1G";
    label = "airlock-usb";
    # The pinned upstream EFI layout starts the ESP at 8 MiB. Relabel only
    # this build artifact, avoiding a generic ESP label on physical hardware.
    postVM = ''
      ${pkgs.mtools}/bin/mlabel -i "$out/${config.image.baseName}.img@@8388608" ::AIRLOCK_EFI
    '';
    contents = [ {
      source = pkgs.writeText "airlock-usb-image" "airlock-usb-dev-v1\n";
      target = "/airlock-usb-image";
    } ];
  });

  services.journald.extraConfig = "Storage=persistent\nSystemMaxUse=1G";
  environment.etc."airlock/usb-dev".text = "airlock-usb-dev-v1\n";
  environment.systemPackages = with pkgs; [
    curl jq gnumake python3 cryptsetup rsync xdotool e2fsprogs openssl
    (writeShellScriptBin "airlock-usb" ''
      export PATH=${lib.makeBinPath [ coreutils util-linux cryptsetup e2fsprogs git openssh openssl ]}:$PATH
      exec ${python3}/bin/python3 ${../tools/usb-dev.py} "$@"
    '')
    (writeShellScriptBin "airlock-usb-amd-preflight" ''
      exec ${python3}/bin/python3 ${../tools/amd-preflight.py} "$@"
    '')
    (lib.hiPrio (writeShellScriptBin "codex" ''
      if ! ${util-linux}/bin/mountpoint -q /home/airlock/.codex; then
        echo 'Unlock the encrypted credentials first: sudo airlock-usb vault-unlock' >&2
        exit 1
      fi
      export CODEX_HOME=/home/airlock/.codex
      exec ${codex}/bin/codex "$@"
    ''))
  ];
  systemd.tmpfiles.rules = [
    "d /var/lib/airlock/usb 0700 root root - -"
    "d /home/airlock/dev 0755 airlock users - -"
    "d /home/airlock/.airlock-logs 0700 airlock users - -"
  ];
  # The reusable guest is locked. Personalization supplies a local password
  # hash outside Nix; propagate it through dom0's existing Qubes authority.
  systemd.services.airlock-usb-gui-password = {
    requires = [ "airlock-setup-config.service" ];
    after = [ "airlock-setup-config.service" ];
    before = [ "display-manager.service" ];
    serviceConfig = { Type = "oneshot"; RemainAfterExit = true; };
    path = [ (pkgs.python3.withPackages (ps: [ ps.qubes-core-admin-client ])) ];
    script = ''
      set -eu
      test -s /var/lib/airlock/usb/local-password.hash
      # Empty declarative PCI lists do not remove mutable Qubes assignments.
      # Fail closed if an experiment left one behind; use text recovery.
      python3 - <<'PY'
      import qubesadmin
      vm = qubesadmin.Qubes().domains['sys-gui']
      if list(vm.devices['pci'].get_assigned_devices()):
          raise SystemExit('sys-gui has PCI assignments; remove them in USB text recovery before hybrid boot')
      PY
      { printf 'user:'; cat /var/lib/airlock/usb/local-password.hash; } |
        qvm-run --pass-io --no-gui --user root sys-gui 'chpasswd -e'
    '';
  };
  systemd.services.display-manager.requires = [ "airlock-usb-gui-password.service" ];
  systemd.services.display-manager.after = [ "airlock-usb-gui-password.service" ];
  # Persisted autostart symlinks must not race declarative setup or recovery.
  # The hybrid relay/password service explicitly starts sys-gui through Qubes.
  systemd.services."qubes-vm@".enable = lib.mkForce false;
  system.stateVersion = "26.05";
}
