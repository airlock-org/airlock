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
    "nixome-managed" = "1";
    "nixome-role" = "sys-config-staging";
    "nixome-config-staging" = "1";
    "nixome-config-applied-message" = "initial";
  };
  services."network-manager" = false;
}
