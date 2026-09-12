{
  guestHostName = "sys-gui";
  label = "black";
  rootImageSizeMiB = 512;
  netvm = null;
  prefs = {
    autostart = true;
    guivm = "dom0";
    include_in_backups = true;
    memory = 1024;
    maxmem = 2048;
    vcpus = 2;
  };
  features = {
    gui = "1";
    "gui-allow-fullscreen" = "1";
    "airlock-managed" = "1";
    "airlock-role" = "sys-gui";
    servicevm = "1";
  };
  services = {
    guivm = true;
    "guivm-gui-agent" = true;
    "network-manager" = false;
  };
}
