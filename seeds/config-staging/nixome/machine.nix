{
  appliedMessage,
  pkgs ? null,
  system,
  templateBuilders ? null,
  templateSet,
  ...
}:
{
  # Primary edit surface for this Nixome machine.
  #
  # Edit this file in config-staging, commit the change, validate it in
  # config-validator, then ask dom0 to apply it. config-vault records the
  # candidate and deployment history.

  # Add NixOS-built TemplateVMs and AppVMs here. High-level fields are turned
  # into normal NixOS modules before the Qubes template source is built.
  #
  # Example:
  #
  # templates.definitions.nixome-nixos-dev = {
  #   role = "dev-template";
  #   sourceLabel = "local-config";
  #   hostName = "nixome-dev-template";
  #   packages = with pkgs; [
  #     git
  #     jq
  #     ripgrep
  #   ];
  #   etc."nixome/template-role".text = "dev\n";
  #   configuration = {
  #     programs.bash.shellAliases.ll = "ls -l";
  #   };
  #   appVms.nixome-dev = {
  #     label = "blue";
  #     features."nixome-role" = "dev";
  #   };
  # };
  templates.definitions = { };

  # Add or override raw Qubes topology entries here. These become normalized
  # nixome.topology JSON in config-validator before dom0 applies them.
  topology = {
    extraTemplates = { };
    extraKernels = { };
    extraPrefs = { };
    extraDomains = { };
    extraQrexecRules = [ ];
  };

  # Config authority VMs are fixed-name system roles. The default keeps all
  # three AppVMs on one shared TemplateVM so nested development stays practical.
  # Set templateMode = "dedicated" to make config-vault, config-staging, and
  # config-validator use separate role-specific TemplateVM artifacts built from
  # this same candidate flake.
  configVms = {
    templateMode = "shared";
    domains = { };
  };

  # Add dom0 host configuration here. These fields compile into normal NixOS
  # modules inside nixosConfigurations.nixome-dom0.
  dom0.host = {
    packages = [ ];
    etc = { };
    systemd = { };

    # For ordinary NixOS options that do not have a high-level field here.
    configuration = { };

    # Advanced escape hatch: raw NixOS modules appended after the high-level
    # host fields.
    modules = [ ];
  };

  # Backward-compatible raw NixOS modules for dom0. Prefer dom0.host for new
  # config.
  dom0.extraModules = [ ];

  # Change system args such as dom0SystemSwitchAction = "boot" when a candidate
  # should install a generation for the next reboot instead of switching live.
  system.args = { };
}
