{ networkDebug }:
{
  guestHostName = "sys-firewall";
  label = "green";
  rootImageSizeMiB = 512;
  netvm = "sys-net";
  prefs = {
    autostart = !networkDebug;
    guivm = null;
    include_in_backups = true;
    memory = 256;
    maxmem = 512;
    provides_network = true;
    vcpus = 1;
  };
  features = {
    "airlock-managed" = "1";
    "airlock-role" = "sys-firewall";
    "airlock-network-provider" = "sys-firewall";
    servicevm = "1";
  };
  services = {
    "network-manager" = false;
    "qubes-firewall" = true;
    "qubes-network" = true;
    "qubes-updates-proxy" = true;
  };
}
