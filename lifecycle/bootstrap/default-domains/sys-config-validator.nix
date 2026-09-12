{ configNixPrivateSizeBytes }:
{
  guestHostName = "sys-config-validator";
  label = "blue";
  rootImageSizeMiB = 128;
  state.persistentSize = configNixPrivateSizeBytes;
  prefs = {
    autostart = false;
    guivm = null;
    include_in_backups = true;
    memory = 512;
    maxmem = 1024;
    vcpus = 2;
  };
  features = {
    "airlock-managed" = "1";
    "airlock-role" = "sys-config-validator";
    "airlock-config-validator" = "1";
  };
  services."network-manager" = false;
}
