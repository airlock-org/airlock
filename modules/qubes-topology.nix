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

  domainType = lib.types.submodule {
    options = {
      class = lib.mkOption {
        type = lib.types.enum [
          "AppVM"
          "StandaloneVM"
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
    domains = cfg.domains;
    prefs = cfg.prefs;
  };

  reconciler = pkgs.callPackage ../pkgs/nixome-qubes-reconcile { };

  configAccessPolicyPackage = pkgs.runCommand "nixome-config-access-policy" { } ''
        set -eu

        install -d "$out/etc/qubes/policy.d"
        cat > "$out/etc/qubes/policy.d/30-nixome-config.policy" <<'EOF'
    # Managed by Nixome. This grants sys-agent read-only access to the
    # config-vault export service and denies the service everywhere else.
    ${cfg.configAccessPolicy.service} * ${cfg.configAccessPolicy.reader} ${cfg.configAccessPolicy.vault} allow
    ${cfg.configAccessPolicy.service} * @anyvm @anyvm deny
    EOF
  '';
in
{
  options.nixome.qubes = {
    enable = lib.mkEnableOption "declarative Qubes topology reconciliation";

    templates = lib.mkOption {
      type = lib.types.attrsOf templateType;
      default = { };
      description = "TemplateVMs that Nixome can import before reconciling domains.";
    };

    domains = lib.mkOption {
      type = lib.types.attrsOf domainType;
      default = { };
      description = "Qubes domains Nixome should create and configure.";
    };

    prefs = lib.mkOption {
      type = lib.types.attrsOf jsonScalar;
      default = { };
      description = "Global qubes-prefs values. Reserved for the next reconciler layer.";
    };

    configAccessPolicy = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Install qrexec policy for sys-agent read access to sys-config-vault.";
      };

      service = lib.mkOption {
        type = lib.types.str;
        default = "nixome.Config.Export";
        description = "qrexec service used for read-only config export.";
      };

      vault = lib.mkOption {
        type = lib.types.str;
        default = "sys-config-vault";
        description = "Qube that owns the authoritative configuration repository.";
      };

      reader = lib.mkOption {
        type = lib.types.str;
        default = "sys-agent";
        description = "Qube allowed to read exported configuration snapshots.";
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
      }) cfg.templates;

    environment.etc."nixome/qubes-topology.json".text = builtins.toJSON topology + "\n";
    environment.systemPackages = [ reconciler ];

    virtualisation.qubes.packages = lib.optionals cfg.configAccessPolicy.enable [
      configAccessPolicyPackage
    ];
  };
}
