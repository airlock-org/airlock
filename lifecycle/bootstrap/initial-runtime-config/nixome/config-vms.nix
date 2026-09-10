{
  configDomains ? { },
  extraTemplates ? { },
  extraKernels ? { },
  extraPrefs ? { },
  extraDomains ? { },
  extraQrexecRules ? [ ],
  ...
}:
let
  requiredConfigDomains = [
    "config-vault"
    "config-staging"
    "config-validator"
  ];

  missingConfigDomains = builtins.filter (
    name: !(builtins.hasAttr name configDomains)
  ) requiredConfigDomains;
in
if missingConfigDomains != [ ] then
  throw "nixome.configVms: missing generated nix-closure domain(s): ${builtins.concatStringsSep ", " missingConfigDomains}"
else
  {
    templates = extraTemplates;
    kernels = extraKernels;
    prefs = {
      default_kernel = "";
    }
    // extraPrefs;
    domains = configDomains // extraDomains;
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
