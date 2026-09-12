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
    # Evaluating the full candidate NixOS configurations exceeds 1 GiB.
    memory = 2048;
    maxmem = 4096;
    vcpus = 2;
  };
  features = {
    "airlock-managed" = "1";
    "airlock-role" = "sys-config-validator";
    "airlock-config-validator" = "1";
  };
  services."network-manager" = false;
}
