{
  guestHostName = "usr-home";
  label = "blue";
  rootImageSizeMiB = 1024;
  netvm = null;
  prefs = {
    autostart = false;
    include_in_backups = true;
    memory = 2048;
    maxmem = 4096;
    vcpus = 2;
  };
  features = {
    "airlock-managed" = "1";
    "airlock-role" = "usr-home";
    "menu-items" = "";
  };
  services."network-manager" = false;
  state.persistentSize = 17179869184;
}
