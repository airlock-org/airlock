{ pkgs, ... }:
{
  # Observed on 21CFCTO1WW. No internal disk mount or PCI assignment here.
  hardware.cpu.amd.updateMicrocode = true;
  hardware.enableRedistributableFirmware = true;
  boot.initrd.availableKernelModules = [ "xhci_pci" "usb_storage" "uas" "sd_mod" ];
  # Xen's firmware console disappears as dom0 takes over. Load amdgpu in the
  # initrd so the internal panel has a DRM framebuffer before the USB root,
  # Qubes setup and display manager are available.
  boot.initrd.kernelModules = [ "amdgpu" ];
  boot.kernelModules = [ "thinkpad_acpi" "r8169" "ath11k_pci" ];
  services.xserver.videoDrivers = [ "amdgpu" ];
  hardware.graphics.enable = true;
  environment.systemPackages = [ pkgs.pciutils pkgs.usbutils ];
}
