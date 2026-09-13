{ config, lib, pkgs, modulesPath, ... }:
{
  imports = [
    "${modulesPath}/virtualisation/disk-image.nix"
    ../../bootstrap/gui-minimal.nix
    ./dev-mode.nix
    ./usb-prepared-store.nix
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
  # The image build creates this account before offline personalization supplies
  # its hash. With mutable users, activation preserves that existing locked
  # password instead of applying hashedPasswordFile. Keep USB users declarative
  # so every boot (including specialisations) applies the provisioned hash.
  users.mutableUsers = false;
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
  # The inherited dom0 USB policy would block this image's USB root disk.
  # Restrict this development exception to the USB image and its recovery modes.
  services.usbguard.enable = lib.mkForce false;
  services.xserver.enable = true;
  services.xserver.displayManager.lightdm.enable = true;
  services.libinput.enable = true;

  boot.loader.systemd-boot.enable = true;
  boot.loader.systemd-boot.configurationLimit = 5;
  boot.loader.timeout = 10;
  boot.loader.efi.canTouchEfiVariables = false;
  boot.initrd.systemd.enable = true;
  boot.kernelParams = [ "console=tty0" "console=hvc0" ];
  # Keep the POC image fixed-size: it avoids first-boot partition mutation.
  boot.growPartition = false;
  fileSystems."/".device = lib.mkForce "/dev/disk/by-label/airlock-usb";
  fileSystems."/".autoResize = lib.mkForce false;
  # NixOS's disk-image builder formats its EFI partition with this label before
  # it invokes switch-to-configuration to install systemd-boot. Keep the
  # runtime configuration aligned with that build-time filesystem.
  fileSystems."/boot".device = lib.mkForce "/dev/disk/by-label/ESP";
  image = { baseName = "airlock-usb-dev"; format = "raw"; efiSupport = true; };
  # A fixed 32-GiB developer image: 1 GiB EFI plus roughly 31 GiB root.
  # Keep headroom for writable VM state and developer updates. Guest backing
  # data is already present at its final path; first boot only verifies/adopts it.
  virtualisation.diskSize = 32768;
  system.build.image = lib.mkForce (import "${modulesPath}/../lib/make-disk-image.nix" {
    inherit config lib pkgs;
    inherit (config.image) baseName format;
    diskSize = config.virtualisation.diskSize;
    partitionTableType = "efi";
    bootSize = "1G";
    label = "airlock-usb";
    contents = [
      {
        source = pkgs.writeText "airlock-usb-image" "airlock-usb-dev-v1\n";
        target = "/airlock-usb-image";
      }
      {
        source = "${config.system.build.usbPreparedGuestStore}/pool";
        target = "/var/lib/airlock/closure-store";
        # Preserve sealed files' read-only owner mode. Qubes verifies this
        # before exposing the EROFS view to a guest. Remove only group/other
        # access from the prepared source tree.
        mode = config.airlock.image.preparedStoreMode;
      }
    ];
  });

  services.journald.settings.Journal = {
    Storage = "persistent";
    SystemMaxUse = "1G";
    SyncIntervalSec = "1s";
  };
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
      if ! ${util-linux}/bin/mountpoint -q /home/airlock/.codex \
        && ! test -f /var/lib/airlock/usb/insecure-codex-auth; then
        echo 'Unlock credentials with: sudo airlock-usb vault-unlock' >&2
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
  # Persisted autostart symlinks must not race declarative setup or recovery.
  # The hybrid desktop path explicitly starts sys-gui through Qubes.
  systemd.services."qubes-vm@".enable = lib.mkForce false;
  system.stateVersion = "26.05";
}
