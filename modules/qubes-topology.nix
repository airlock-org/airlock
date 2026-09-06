{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.nixome.qubes;
  access = cfg.configAccessPolicy;
  topologyLib = import ../lib/nixome/topology.nix;
  sourcePath = lib.types.oneOf [
    lib.types.path
    lib.types.str
  ];
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
        type = lib.types.nullOr sourcePath;
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
        type = lib.types.nullOr sourcePath;
        default = null;
        description = "Source directory containing Qubes VM kernel files.";
      };
    };
  };

  domainType = lib.types.submodule {
    options = {
      class = lib.mkOption {
        type = lib.types.enum topologyLib.domainClasses;
        default = "AppVM";
        description = "Qubes domain class to create when the domain is missing.";
      };

      template = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "TemplateVM used for AppVM creation.";
      };

      label = lib.mkOption {
        type = lib.types.enum topologyLib.labels;
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
    inherit (cfg)
      templates
      kernels
      domains
      prefs
      ;
  };

  setupConfig = pkgs.callPackage ../pkgs/nixome-setup-config { };
  configApply = pkgs.callPackage ../pkgs/nixome-config-apply { };
  configArchive = pkgs.callPackage ../pkgs/nixome-config-archive { };
  configVmsSmoke = pkgs.callPackage ../pkgs/nixome-config-vms-smoke { };

  configAccessPolicyPackage = pkgs.runCommand "nixome-config-access-policy" { } ''
        set -eu

        install -d "$out/etc/qubes/policy.d"
        cat > "$out/etc/qubes/policy.d/30-nixome-config.policy" <<'EOF'
    # Managed by Nixome. Only dom0 may fetch a candidate from config-staging,
    # ask config-validator to validate it, or archive it into config-vault.
    ${access.candidateExportService} * dom0 ${access.staging} allow
    ${access.candidateExportService} * @anyvm @anyvm deny
    ${access.candidateValidateService} * dom0 ${access.validator} allow
    ${access.candidateValidateService} * @anyvm @anyvm deny
    ${access.archiveService} * dom0 ${access.vault} allow
    ${access.archiveService} * @anyvm @anyvm deny
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

    topology = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      description = "Normalized Nixome Qubes topology JSON payload.";
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
        description = "qrexec service used by dom0 to fetch a candidate from config-staging.";
      };

      candidateValidateService = lib.mkOption {
        type = lib.types.str;
        default = "nixome.Config.ValidateCandidate";
        description = "qrexec service used by dom0 to validate a candidate in config-validator.";
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

      staging = lib.mkOption {
        type = lib.types.str;
        default = "config-staging";
        description = "Qube that owns the editable configuration candidate.";
      };

      validator = lib.mkOption {
        type = lib.types.str;
        default = "config-validator";
        description = "Qube that validates staged configuration candidates.";
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

    nixome.qubes.topology = topologyLib.mkTopology topology;

    environment.etc."nixome/qubes-topology.json".text =
      builtins.toJSON config.nixome.qubes.topology + "\n";
    environment.systemPackages = [
      configApply
      configArchive
      configVmsSmoke
      setupConfig
    ];

    virtualisation.qubes.packages = lib.optionals cfg.configAccessPolicy.enable [
      configAccessPolicyPackage
    ];
  };
}
