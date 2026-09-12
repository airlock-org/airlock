{ mkAirlockQubeBuilders, kernelName }:
system:
(mkAirlockQubeBuilders system).mkNixNativeDomain {
  name = "sys-gpu";
  configurationName = "airlock-domain-sys-gpu";
  guestRole = "base";
  inherit kernelName;
  modules = [ ../modules/sys-gpu-guest.nix ];
  label = "red";
  netvm = null;
  prefs = {
    autostart = false;
    guivm = null;
    memory = 2048;
    maxmem = 2048;
    vcpus = 2;
    virt_mode = "hvm";
  };
  features = {
    "airlock-role" = "sys-gpu";
    "airlock-gpu-experiment" = "1";
    servicevm = "1";
    gui = "0";
  };
  services."network-manager" = false;
  # The initial nested-Xen experiment tests lifecycle and driver diagnostics.
  # Physical assignments require a separate, hardware-specific configuration.
  devices.pci = [ ];
}
