{ sysNetPrivateSizeBytes }:
{
  guestHostName = "sys-net";
  label = "red";
  rootImageSizeMiB = 512;
  netvm = null;
  state.persistentSize = sysNetPrivateSizeBytes;
  prefs = {
    autostart = true;
    guivm = null;
    include_in_backups = true;
    memory = 256;
    maxmem = 512;
    provides_network = true;
    vcpus = 2;
    virt_mode = "hvm";
  };
  features = {
    "airlock-managed" = "1";
    "airlock-role" = "sys-net";
    "airlock-network-provider" = "sys-net";
  };
  services = {
    "network-manager" = true;
    "qubes-firewall" = true;
    "qubes-network" = true;
    "qubes-updates-proxy" = true;
  };
  devices.pci = [ ];
}
