{
  guestHostName = "sys-dev";
  label = "red";
  rootImageSizeMiB = 1024;
  netvm = null;
  prefs = {
    autostart = true;
    guivm = null;
    include_in_backups = true;
    memory = 2048;
    maxmem = 4096;
    provides_network = true;
    vcpus = 2;
    virt_mode = "hvm";
  };
  features = {
    "airlock-managed" = "1";
    "airlock-role" = "sys-dev";
    # This development-only driver domain takes the physical NIC until the
    # later sys-net/sys-firewall stage replaces it.
    "airlock-auto-network-pci" = "1";
    "airlock-network-provider" = "sys-dev";
  };
  services = {
    "network-manager" = true;
    "qubes-firewall" = true;
    "qubes-network" = true;
  };
  devices.pci = [ ];
  state.persistentSize = 17179869184;
}
