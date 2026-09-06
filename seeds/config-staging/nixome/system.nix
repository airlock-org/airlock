{
  appliedMessage ? "initial",
  extraPackages ? [ ],
  extraSystemdServices ? { },
  dom0SystemAttr ? "nixosConfigurations.nixome-dom0.config.system.build.toplevel",
  rebootRequests ? { },
  system ? "x86_64-linux",
}:
{
  dom0 = {
    profile = {
      source = "dom0-profile";
      activationCommand = "bin/nixome-dom0-profile-activate";
      activationAction = "run";
      message = appliedMessage;
    };

    packages = [
      { name = "nixome-dom0-profile"; }
    ]
    ++ extraPackages;

    systemd.services = extraSystemdServices;

    nixos = {
      system = rebootRequests.system or {
        source = {
          build = dom0SystemAttr;
        };
        switchAction = "switch";
        reason = "staged dom0 NixOS system generation";
      };
      kernel = rebootRequests.kernel or null;
      xen = rebootRequests.xen or null;
      bootloader = rebootRequests.bootloader or null;
      qubes = rebootRequests.qubes or null;
    };
  };
}
