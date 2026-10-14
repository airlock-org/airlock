{ config, lib, pkgs, modulesPath, ... }:
{
  imports = [ "${modulesPath}/virtualisation/disk-image.nix" ];

  # Physical boot baseline: the existing dom0 control stack, with no guest
  # topology or desktop. Do not import USB dev or a laptop hardware profile.
  airlock.dom0 = {
    enable = true;
    user = "user";
    gui.enable = false;
    devModeNetworkProxy.enable = false;
    dom0Resources = { memoryMiB = 4096; maxMemoryMiB = 4096; };
  };
  airlock.qubes.setupConfigOnBoot.enable = false;
  systemd.defaultUnit = "multi-user.target";
  systemd.services."qubes-vm@".enable = lib.mkForce false;

  # Dom0's default-block USB policy has no exception for the live root disk.
  # Keep USB storage accessible in this diagnostic image only.
  services.usbguard.enable = lib.mkForce false;

  users.users.user = {
    isNormalUser = true;
    uid = 1000;
    extraGroups = [ "wheel" "qubes" "networkmanager" ];
    # Deliberately public development login; no personalization step.
    initialPassword = "password";
  };
  users.users.root.hashedPassword = "!";
  security.sudo.wheelNeedsPassword = true;

  networking.useDHCP = false;
  networking.networkmanager.enable = true;
  services.tailscale.enable = true;
  services.openssh = {
    enable = true;
    openFirewall = false;
    settings = {
      PasswordAuthentication = false;
      KbdInteractiveAuthentication = false;
      PermitRootLogin = "no";
    };
  };
  networking.firewall.interfaces.tailscale0.allowedTCPPorts = [ 22 ];

  # Generic storage support for physical USB and the QEMU test harness.
  hardware.enableRedistributableFirmware = true;
  boot.initrd.availableKernelModules = [
    "xhci_pci" "ehci_pci" "usb_storage" "uas" "sd_mod"
    "virtio_pci" "virtio_blk" "virtio_scsi"
  ];
  boot.initrd.systemd.enable = true;
  boot.kernelParams = [ "console=hvc0" "console=tty0" ];
  virtualisation.xen.boot.params = [ "console=vga,com1" "com1=115200,8n1" ];
  boot.loader.systemd-boot.enable = true;
  boot.loader.timeout = 10;
  boot.loader.efi.canTouchEfiVariables = false;
  boot.growPartition = false;
  fileSystems."/" = {
    device = lib.mkForce "/dev/disk/by-label/airlock-console";
    autoResize = lib.mkForce false;
  };
  fileSystems."/boot".device = lib.mkForce "/dev/disk/by-label/ESP";
  image = { baseName = "airlock-usb-dom0-console"; format = "raw"; efiSupport = true; };
  virtualisation.diskSize = 12288;
  system.build.image = lib.mkForce (import "${modulesPath}/../lib/make-disk-image.nix" {
    inherit config lib pkgs;
    inherit (config.image) baseName format;
    diskSize = config.virtualisation.diskSize;
    partitionTableType = "efi";
    # Match the developer image's space for boot generations and install staging.
    bootSize = "3G";
    label = "airlock-console";
  });

  services.journald.settings.Journal = {
    Storage = "persistent";
    SystemMaxUse = "256M";
    # Reduce buffered log loss if a black-screen boot needs a forced shutdown.
    SyncIntervalSec = "1s";
  };
  environment.systemPackages = with pkgs; [ pciutils usbutils ];
  environment.etc."airlock/usb-dom0-console".text = "airlock-usb-dom0-console-v1\n";
  environment.loginShellInit = ''
    if [ -t 1 ]; then
      echo 'Airlock dom0 console baseline. Local login: user / password'
      echo 'Check Xen: cat /proc/xen/capabilities; sudo xl list'
    fi
  '';
  system.stateVersion = "26.05";
}
