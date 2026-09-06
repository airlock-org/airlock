{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.nixome.qubes;
  jsonScalar = lib.types.nullOr (
    lib.types.oneOf [
      lib.types.bool
      lib.types.int
      lib.types.str
    ]
  );

  templateType = lib.types.submodule {
    options = {
      import = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Import this template source if the TemplateVM is missing.";
      };

      source = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Qubes template source directory containing root.img and template.conf.";
      };

      skipStart = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Pass --skip-start while importing the template.";
      };
    };
  };

  kernelType = lib.types.submodule {
    options = {
      source = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Source directory containing a Qubes VM kernel vmlinuz and optional initramfs/modules.img.";
      };
    };
  };

  domainType = lib.types.submodule {
    options = {
      class = lib.mkOption {
        type = lib.types.enum [
          "AppVM"
          "StandaloneVM"
          "TemplateVM"
        ];
        default = "AppVM";
        description = "Qubes domain class to create when the domain is missing.";
      };

      template = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "TemplateVM used for AppVM creation.";
      };

      label = lib.mkOption {
        type = lib.types.str;
        default = "red";
        description = "Qubes label.";
      };

      netvm = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "NetVM to assign. Null means no network.";
      };

      prefs = lib.mkOption {
        type = lib.types.attrsOf jsonScalar;
        default = { };
        description = "Additional qvm-prefs values to set.";
      };

      features = lib.mkOption {
        type = lib.types.attrsOf jsonScalar;
        default = { };
        description = "qvm-features values to set.";
      };

      services = lib.mkOption {
        type = lib.types.attrsOf lib.types.bool;
        default = { };
        description = "qvm-service values to set.";
      };
    };
  };

  topology = {
    schema = "nixome.qubes-topology.v1";
    templates = lib.mapAttrs (_: template: {
      inherit (template) import source;
      skip_start = template.skipStart;
    }) cfg.templates;
    kernels = cfg.kernels;
    domains = cfg.domains;
    prefs = cfg.prefs;
  };

  setupConfig = pkgs.callPackage ../pkgs/nixome-setup-config { };
  configArchive = pkgs.callPackage ../pkgs/nixome-config-archive { };
  configVmsSmoke = pkgs.callPackage ../pkgs/nixome-config-vms-smoke { };

  configAccessPolicyPackage = pkgs.runCommand "nixome-config-access-policy" { } ''
        set -eu

        install -d "$out/etc/qubes/policy.d"
        cat > "$out/etc/qubes/policy.d/30-nixome-config.policy" <<'EOF'
    # Managed by Nixome. Only dom0 may fetch a validated candidate from
    # config-mod or archive a candidate into config-vault.
    ${cfg.configAccessPolicy.candidateExportService} * dom0 ${cfg.configAccessPolicy.modifier} allow
    ${cfg.configAccessPolicy.candidateExportService} * @anyvm @anyvm deny
    ${cfg.configAccessPolicy.archiveService} * dom0 ${cfg.configAccessPolicy.vault} allow
    ${cfg.configAccessPolicy.archiveService} * @anyvm @anyvm deny
    EOF
  '';
in
{
  options.nixome.qubes = {
    enable = lib.mkEnableOption "declarative Qubes config topology setup";

    templates = lib.mkOption {
      type = lib.types.attrsOf templateType;
      default = { };
      description = "TemplateVMs that Nixome can import before setting up domains.";
    };

    kernels = lib.mkOption {
      type = lib.types.attrsOf kernelType;
      default = { };
      description = "Qubes VM kernels Nixome should install into dom0's VM kernel pool.";
    };

    domains = lib.mkOption {
      type = lib.types.attrsOf domainType;
      default = { };
      description = "Qubes domains Nixome should create and configure.";
    };

    prefs = lib.mkOption {
      type = lib.types.attrsOf jsonScalar;
      default = { };
      description = "Global qubes-prefs values for the setup-config layer.";
    };

    configAccessPolicy = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Install qrexec policy for dom0-mediated config candidate archival.";
      };

      candidateExportService = lib.mkOption {
        type = lib.types.str;
        default = "nixome.Config.CandidateExport";
        description = "qrexec service used by dom0 to fetch a validated candidate from config-mod.";
      };

      archiveService = lib.mkOption {
        type = lib.types.str;
        default = "nixome.Config.ArchiveCandidate";
        description = "qrexec service used by dom0 to archive a candidate into config-vault.";
      };

      vault = lib.mkOption {
        type = lib.types.str;
        default = "config-vault";
        description = "Qube that owns the archived configuration history.";
      };

      modifier = lib.mkOption {
        type = lib.types.str;
        default = "config-mod";
        description = "Qube that owns the editable configuration candidate.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions =
      lib.mapAttrsToList (name: domain: {
        assertion = domain.class != "AppVM" || domain.template != null;
        message = "nixome.qubes.domains.${name}.template must be set for AppVM domains.";
      }) cfg.domains
      ++ lib.mapAttrsToList (name: template: {
        assertion = !template.import || template.source != null;
        message = "nixome.qubes.templates.${name}.source must be set when import is enabled.";
      }) cfg.templates
      ++ lib.mapAttrsToList (name: kernel: {
        assertion = kernel.source != null;
        message = "nixome.qubes.kernels.${name}.source must be set.";
      }) cfg.kernels;

    environment.etc."nixome/qubes-topology.json".text = builtins.toJSON topology + "\n";
    environment.systemPackages = [
      configArchive
      configVmsSmoke
      setupConfig
    ];

    virtualisation.qubes.packages = lib.optionals cfg.configAccessPolicy.enable [
      configAccessPolicyPackage
    ];
  };
}
