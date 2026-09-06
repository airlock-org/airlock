{
  appliedMessage ? "initial",
  extraDomains ? { },
}:
let
  configDomains = {
    config-vault = {
      template = "nixome-nixos-config-vault";
      label = "black";
      netvm = null;
      prefs = {
        autostart = false;
        include_in_backups = true;
        memory = 1024;
        maxmem = 2048;
        vcpus = 1;
      };
      features = {
        "nixome-managed" = "1";
        "nixome-role" = "config-vault";
        "nixome-config-authority" = "1";
      };
      services = {
        "network-manager" = false;
      };
    };

    config-staging = {
      template = "nixome-nixos-config-staging";
      label = "gray";
      netvm = null;
      prefs = {
        autostart = false;
        include_in_backups = true;
        memory = 2048;
        maxmem = 4096;
        vcpus = 2;
      };
      features = {
        "nixome-managed" = "1";
        "nixome-role" = "config-staging";
        "nixome-config-staging" = "1";
        "nixome-config-applied-message" = appliedMessage;
      };
    };

    config-validator = {
      template = "nixome-nixos-config-validator";
      label = "blue";
      netvm = null;
      prefs = {
        autostart = false;
        include_in_backups = true;
        memory = 2048;
        maxmem = 4096;
        vcpus = 2;
      };
      features = {
        "nixome-managed" = "1";
        "nixome-role" = "config-validator";
        "nixome-config-validator" = "1";
      };
    };
  };
in
{
  templates = { };
  kernels = { };
  prefs = {
    default_kernel = "";
  };
  domains = configDomains // extraDomains;
  qrexecPolicy.rules = [
    {
      service = "nixome.PolicyProbe";
      source = "dom0";
      target = "config-vault";
      action = "allow";
    }
  ];
}
