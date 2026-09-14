{ config, lib, pkgs, ... }:
{
  # Observed on 21CFCTO1WW. No internal disk mount or PCI assignment here.
  hardware.cpu.amd.updateMicrocode = true;
  hardware.enableRedistributableFirmware = true;
  boot.initrd.availableKernelModules = [ "xhci_pci" "usb_storage" "uas" "sd_mod" ];
  # Xen's firmware console disappears as dom0 takes over. Load amdgpu in the
  # initrd so the internal panel has a DRM framebuffer before the USB root,
  # Qubes setup and display manager are available.
  boot.initrd.kernelModules = lib.optional (!config.airlock.guiGpu.enable) "amdgpu";
  boot.kernelModules = [ "thinkpad_acpi" "r8169" "ath11k_pci" ];
  services.xserver.videoDrivers = [ "amdgpu" ];
  hardware.graphics.enable = true;
  # Inventory only: the optional physical-GPU profile enables reservation and
  # assignment. Other installations supply their own machine-local selection.
  airlock.guiGpu.device = {
    bdf = "0000:04:00.0";
    port = "00_08.1-00_00.0";
    vendor = "1002";
    device = "1681";
    driver = "amdgpu";
  };
  environment.systemPackages = [ pkgs.pciutils pkgs.usbutils ];
  # Observed stable evdev links on this machine. This declaration is inert
  # unless a physical-GUI profile opts into the sender. Do not copy eventN
  # numbers, touchscreen devices, or this selection into generic modules.
  airlock.inputProxy.sender.devices = {
    keyboard = {
      path = "/dev/input/by-path/platform-i8042-serio-0-event-kbd";
      type = "keyboard";
    };
    pointingstick = {
      path = "/dev/input/by-path/platform-i8042-serio-1-event-mouse";
      type = "mouse";
    };
    touchpad = {
      path = "/dev/input/by-path/platform-AMDI0010:01-event-mouse";
      type = "tablet";
    };
  };
}
