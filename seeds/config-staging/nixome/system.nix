{
  appliedMessage ? "initial",
  extraPackages ? [ ],
  extraSystemdServices ? { },
  rebootRequests ? { },
  system ? "x86_64-linux",
}:
{
  dom0 = {
    profile = {
      source = {
        build = "packages.${system}.nixome-dom0-profile";
      };
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
      system = rebootRequests.system or null;
      kernel = rebootRequests.kernel or null;
      xen = rebootRequests.xen or null;
      bootloader = rebootRequests.bootloader or null;
      qubes = rebootRequests.qubes or null;
    };
  };
}
