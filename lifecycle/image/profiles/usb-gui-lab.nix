{ ... }:
{
  imports = [ ./usb-headless.nix ];
  # GUI-only hardware validation profile. It deliberately contains no general
  # work, browser, network-provider, or remote-development Qubes.
  airlock.inputProxy.sender.enable = true;
  environment.etc."airlock/usb-gui-lab".text =
    "experimental; declarative GPU assignment and one boot start\n";
  airlock.guiGpu = {
    enable = true;
    startOnBoot = true;
    bootFramebuffer.enable = true;
  };
  airlock.amdVbios = {
    enable = true;
    domains.sys-gui = { };
  };
}
