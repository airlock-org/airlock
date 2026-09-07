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

  # Add NixOS modules for dom0. These are evaluated into the candidate
  # nixosConfigurations.nixome-dom0 system generation.
  dom0.extraModules = [ ];

  # Change system args such as dom0SystemSwitchAction = "boot" when a candidate
  # should install a generation for the next reboot instead of switching live.
  system.args = { };
}
