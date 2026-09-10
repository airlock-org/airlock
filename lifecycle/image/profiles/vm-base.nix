{
  lib,
  modulesPath,
  pkgs,
  ...
}:
{
  imports = [
    "${modulesPath}/virtualisation/disk-image.nix"
  ];

  nixome.dom0 = {
    enable = true;
    user = "nixome";
    gui.enable = lib.mkDefault false;
    dom0Resources = {
      memoryMiB = 4096;
      maxMemoryMiB = 4096;
    };
  };

  users.users.nixome = {
    isNormalUser = true;
    description = "Nixome nested-VM test operator";
    initialPassword = "nixome";
    extraGroups = [
      "wheel"
      "qubes"
    ];
  };

  # Development smoke scripts need unattended sudo inside throwaway nested VMs.
  # A physical machine profile should override this.
  security.sudo.wheelNeedsPassword = false;

  services.openssh = {
    enable = true;
    settings = {
      PasswordAuthentication = true;
      PermitRootLogin = "no";
    };
  };

  environment.systemPackages = with pkgs; [
    curl
    jq
  ];

  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = lib.mkDefault false;
  boot.initrd.systemd.enable = true;
  boot.initrd.availableKernelModules = [
    "virtio_blk"
    "virtio_pci"
    "virtio_scsi"
  ];
  boot.kernelParams = [
    "console=tty0"
    "console=hvc0"
  ];

  virtualisation.xen.boot.params = [
    "console=com1,vga"
    "com1=115200,8n1"
  ];

  image = {
    baseName = lib.mkDefault "nixome-dom0-vm";
    format = "qcow2";
    efiSupport = true;
  };

  # Sparse qcow2 capacity. The base image is smaller; templates/qubes need room.
  virtualisation.diskSize = 131072;

  networking.hostName = lib.mkDefault "dom0";

  system.stateVersion = "26.05";
}
