{ ... }:
{
  imports = [ ./usb-headless.nix ];
  # Device selection belongs to the imported machine hardware profile.
  # Forwarding still requires explicit activation after the guest is ready.
  airlock.inputProxy.sender.enable = true;
  environment.etc."airlock/usb-amd-lab".text =
    "experimental; declarative GPU assignment and one boot start\n";
  airlock.guiGpu = {
    enable = true;
    startOnBoot = true;
  };
  airlock.amdVbios = {
    enable = true;
    domains.sys-gui = { };
  };
  # Nothing here relaxes reset checks or enables permissive PCI access.
  # The guest's display manager and input forwarding remain manual.
}
