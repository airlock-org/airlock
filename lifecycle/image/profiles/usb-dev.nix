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

  networking.hostName = "airlock-usb";
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
  boot.initrd.systemd.enable = true;
  boot.kernelParams = [ "console=tty0" "console=hvc0" ];
  boot.growPartition = false;
  fileSystems."/".device = lib.mkForce "/dev/disk/by-label/airlock-usb";
  fileSystems."/".autoResize = lib.mkForce false;
  image = { baseName = "airlock-usb-dev"; format = "raw"; efiSupport = true; };
  virtualisation.diskSize = 65536;
  system.build.image = lib.mkForce (import "${modulesPath}/../lib/make-disk-image.nix" {
    inherit config lib pkgs;
    inherit (config.image) baseName format;
    diskSize = config.virtualisation.diskSize;
    partitionTableType = "efi";
    bootSize = "1G";
    label = "airlock-usb";
  });

  services.journald.extraConfig = "Storage=persistent\nSystemMaxUse=1G";
  environment.etc."airlock/usb-dev".text = "airlock-usb-dev-v1\n";
  environment.systemPackages = with pkgs; [
    curl jq gnumake python3 cryptsetup rsync xdotool
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
    path = [ pkgs.python3.pkgs.qubes-core-admin-client ];
    script = ''
      set -eu
      test -s /var/lib/airlock/usb/local-password.hash
      { printf 'user:'; cat /var/lib/airlock/usb/local-password.hash; } |
        qvm-run --pass-io --no-gui --user root sys-gui 'chpasswd -e'
    '';
  };
  systemd.services.display-manager.requires = [ "airlock-usb-gui-password.service" ];
  systemd.services.display-manager.after = [ "airlock-usb-gui-password.service" ];
  system.stateVersion = "26.05";
}
