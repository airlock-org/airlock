{ lib, ... }:
{
  imports = [ ./usb-headless.nix ];
  # Device selection belongs to the imported machine hardware profile.
  # Forwarding still requires explicit activation after the guest is ready.
  airlock.inputProxy.sender.enable = true;
  environment.etc."airlock/usb-amd-lab".text = "experimental; manual GPU assignment only\n";
  airlock.amdVbios = {
    enable = true;
    domains.sys-gui = { };
  };
  boot.blacklistedKernelModules = [ "amdgpu" "radeon" ];
  # The lab owns the AMD GPU through pciback, so it must override the normal
  # ThinkPad profile's early amdgpu console setup.
  boot.initrd.kernelModules = lib.mkForce [ "xen-pciback" ];
  boot.kernelParams = [
    "xen-pciback.hide=(0000:04:00.0)"
    "initcall_blacklist=sysfb_init"
    "video=efifb:off"
  ];
  # Nothing here relaxes reset checks or enables permissive PCI access.
  # Selecting this entry can blank the panel even if assignment never occurs.
  virtualisation.xen.boot.params = [ "iommu=required" ];
}
