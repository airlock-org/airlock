{
  appliedMessage,
  system,
  templateSet,
  ...
}:
{
  topology = {
    extraTemplates = { };
    extraKernels = { };
    extraPrefs = { };
    extraDomains = { };
    extraQrexecRules = [ ];
  };

  dom0.extraModules = [ ];

  system.args = { };
}
