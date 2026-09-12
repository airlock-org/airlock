{
  guestHostName = "sys-config-vault";
  label = "black";
  rootImageSizeMiB = 128;
  prefs = {
    autostart = false;
    guivm = null;
    include_in_backups = true;
    memory = 256;
    maxmem = 512;
    vcpus = 1;
  };
  features = {
    "nixome-managed" = "1";
    "nixome-role" = "sys-config-vault";
    "nixome-config-authority" = "1";
  };
  services."network-manager" = false;
}
