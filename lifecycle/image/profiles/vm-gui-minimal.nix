{ lib, ... }:
{
  imports = [ ../../bootstrap/gui-minimal.nix ];

  # Nested test harness only: retain the base image's loopback-forwarded SSH
  # access without installing the full development toolset or a proxy to an
  # absent sys-firewall. This is not a physical-machine release profile.
  networking.useDHCP = true;
  airlock.dom0.devModeNetworkProxy.enable = false;
  image.baseName = lib.mkForce "airlock-dom0-vm-gui-minimal";
}
