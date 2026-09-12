{
  airlockGuiDomain,
  airlockQubeKernel,
  ...
}:
{
  # Use the normal Qubes topology/storage machinery without importing the
  # full configuration workflow or its VM closures and input sources.
  airlock.qubes = {
    enable = true;
    setupConfigOnBoot = {
      enable = true;
      scope = "all";
    };
    startNetworkProvidersOnBoot.enable = false;
    deferredSetupConfigOnBoot.enable = false;
    configCurrentBootstrapOnBoot.enable = false;
    configAccessPolicy.enable = false;
    prefs = {
      default_kernel = "";
      default_guivm = "sys-gui";
      default_netvm = null;
    };
    kernels.${airlockQubeKernel.name}.source = "${airlockQubeKernel.package}";
    domains.sys-gui = airlockGuiDomain;
  };
}
