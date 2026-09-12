{
  guestHostName = "cmd-vm";
  label = "yellow";
  rootImageSizeMiB = 1024;
  netvm = "sys-firewall";
  prefs = {
    autostart = false;
    include_in_backups = true;
    memory = 2048;
    maxmem = 4096;
    vcpus = 2;
  };
  features = {
    "airlock-managed" = "1";
    "airlock-role" = "cmd";
  };
  services = {
    "network-manager" = false;
  };
  state.persistentSize = 8589934592;
}
