{ ... }:
{
  imports = [ ./usb-headless.nix ];
  environment.etc."airlock/usb-amd-lab".text = "experimental; manual GPU assignment only\n";
  boot.blacklistedKernelModules = [ "amdgpu" "radeon" ];
  boot.initrd.kernelModules = [ "xen-pciback" ];
  boot.kernelParams = [
    "xen-pciback.hide=(0000:04:00.0)"
    "initcall_blacklist=sysfb_init"
    "video=efifb:off"
  ];
  # Nothing here relaxes reset checks or enables permissive PCI access.
  # Selecting this entry can blank the panel even if assignment never occurs.
  virtualisation.xen.boot.params = [ "iommu=required" ];
}
