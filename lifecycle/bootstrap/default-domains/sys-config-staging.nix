{ configNixPrivateSizeBytes }:
{
  guestHostName = "sys-config-staging";
  label = "gray";
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
    "airlock-role" = "sys-config-staging";
    "airlock-config-staging" = "1";
    "airlock-config-applied-message" = "initial";
  };
  services."network-manager" = false;
}
