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
    "sys-config-vault"
    "sys-config-staging"
    "sys-config-validator"
  ];

  missingConfigDomains = builtins.filter (
    name: !(builtins.hasAttr name configDomains)
  ) requiredConfigDomains;
in
if missingConfigDomains != [ ] then
  throw "airlock.configVms: missing generated nix-closure domain(s): ${builtins.concatStringsSep ", " missingConfigDomains}"
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
        service = "airlock.PolicyProbe";
        source = "dom0";
        target = "sys-config-vault";
        action = "allow";
      }
    ]
    ++ extraQrexecRules;
  }
