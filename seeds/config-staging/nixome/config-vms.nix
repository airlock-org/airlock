{
  appliedMessage ? "initial",
  configRoleKernelOpts ? { },
  configVms ? { },
  extraTemplates ? { },
  extraKernels ? { },
  extraPrefs ? { },
  extraDomains ? { },
  extraQrexecRules ? [ ],
  system ? "x86_64-linux",
}:
let
  configSystemTemplate = "nixome-nixos-config-system";
  configTemplateMode = configVms.templateMode or "shared";
  domainOverrides = configVms.domains or { };
  roleTemplateNames = {
    config-vault = "nixome-nixos-config-vault";
    config-staging = "nixome-nixos-config-staging";
    config-validator = "nixome-nixos-config-validator";
  };
  roleLabels = {
    config-vault = "black";
    config-staging = "gray";
    config-validator = "blue";
  };
  roleDescriptions = {
    config-vault = "config-vault-template";
    config-staging = "config-staging-template";
    config-validator = "config-validator-template";
  };

  get =
    attrs: name: default:
    if builtins.hasAttr name attrs then builtins.getAttr name attrs else default;

  mergeDomain =
    base: override:
    base
    // override
    // {
      prefs = (base.prefs or { }) // (override.prefs or { });
      features = (base.features or { }) // (override.features or { });
      services = (base.services or { }) // (override.services or { });
    };

  configRoleTemplateFor =
    role:
    if configTemplateMode == "shared" then
      configSystemTemplate
    else if configTemplateMode == "dedicated" then
      builtins.getAttr role roleTemplateNames
    else
      throw "nixome.configVms.templateMode must be \"shared\" or \"dedicated\", got ${builtins.toJSON configTemplateMode}";

  mkConfigRoleTemplate =
    role:
    let
      templateName = builtins.getAttr role roleTemplateNames;
      label = builtins.getAttr role roleLabels;
      roleDescription = builtins.getAttr role roleDescriptions;
      kernelopts = get configRoleKernelOpts role "";
    in
    {
      name = templateName;
      template = {
        import = true;
        source = {
          build = "packages.${system}.${templateName}-template";
          subpath = templateName;
        };
        skipStart = true;
      };
      domain = {
        class = "TemplateVM";
        label = label;
        netvm = null;
        prefs = {
          default_user = "user";
          kernel = "6.12.64-1.fc41";
          kernelopts = kernelopts;
          qrexec_timeout = 120;
          virt_mode = "pvh";
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = roleDescription;
          os = "Linux";
          qrexec = "1";
        };
        services = { };
      };
    };

  dedicatedRoleTemplates =
    if configTemplateMode == "dedicated" then
      map mkConfigRoleTemplate [
        "config-vault"
        "config-staging"
        "config-validator"
      ]
    else
      [ ];

  dedicatedTemplateEntries = builtins.listToAttrs (
    map (entry: {
      inherit (entry) name;
      value = entry.template;
    }) dedicatedRoleTemplates
  );
  dedicatedTemplateDomains = builtins.listToAttrs (
    map (entry: {
      inherit (entry) name;
      value = entry.domain;
    }) dedicatedRoleTemplates
  );

  configDomains = {
    config-vault = {
      template = configRoleTemplateFor "config-vault";
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
      template = configRoleTemplateFor "config-staging";
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
      template = configRoleTemplateFor "config-validator";
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
  mergedConfigDomains = builtins.mapAttrs (
    name: domain: mergeDomain domain (get domainOverrides name { })
  ) configDomains;
in
{
  templates = dedicatedTemplateEntries // extraTemplates;
  kernels = extraKernels;
  prefs = {
    default_kernel = "";
  }
  // extraPrefs;
  domains = dedicatedTemplateDomains // mergedConfigDomains // extraDomains;
  qrexecPolicy.rules = [
    {
      service = "nixome.PolicyProbe";
      source = "dom0";
      target = "config-vault";
      action = "allow";
    }
  ]
  ++ extraQrexecRules;
}
