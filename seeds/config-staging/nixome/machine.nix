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

  # Declare additional traditional Qubes domains here. Nixome supports the
  # existing TemplateVM/AppVM model alongside Nix-native domains.
  #
  # Traditional Qubes-backed domain:
  #
  # domains.work-fedora = {
  #   backend.type = "qubes-template";
  #   template = "fedora-44-xfce";
  #   label = "green";
  #   netvm = "sys-firewall";
  # };
  #
  domains = { };

  # Declare Nix-native domains here. The materializer creates a Qubes
  # StandaloneVM from a small Nixome root image plus a separate read-only Nix
  # store closure image mounted at /nix/store during early boot.
  #
  # nixNative.domains.work-nix = {
  #   label = "green";
  #   netvm = null;
  #   packages = with pkgs; [
  #     git
  #     jq
  #   ];
  #   configuration = {
  #     environment.etc."nixome/domain-role".text = "work-nix\n";
  #   };
  # };
  nixNative.domains = { };

  # Add or override raw Qubes topology entries here. These become normalized
  # nixome.topology JSON in config-validator before dom0 applies them.
  # Prefer top-level domains for normal additions; extraDomains is a low-level
  # escape hatch that overrides generated domain entries.
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
  #
  # sharedGuest customizes the shared nixome-nixos-config-system TemplateVM.
  # guests.<role> customizes the dedicated role TemplateVMs when templateMode =
  # "dedicated". Supported guest fields match normal template definitions:
  # hostName, packages, etc, systemd, configuration, and modules.
  configVms = {
    templateMode = "shared";
    sharedGuest = { };
    guests = {
      config-vault = { };
      config-staging = { };
      config-validator = { };
    };
    domains = { };
  };

  # Add dom0 host configuration here. These fields compile into normal NixOS
  # modules inside nixosConfigurations.nixome-dom0. Use overlays for dom0
  # package/version overrides.
  dom0.host = {
    # Automatic apply classification:
    # - packages, etc, and systemd fields default to an in-session switch
    # - overlays, configuration, modules, and dom0.extraModules default to the
    #   next boot generation because they can affect Xen/kernel/early services
    # Override only when you know the change class:
    # apply.mode = "switch"; # force live switch
    # apply.mode = "boot";   # force next boot generation
    apply.mode = "auto";

    overlays = [ ];
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

  # Low-level escape hatch. These values override dom0.host.apply classification
  # and are mainly for tests or unusual hand-authored plans.
  system.args = { };
}
