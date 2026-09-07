{
  appliedMessage,
  pkgs ? null,
  system,
  templateBuilders ? null,
  templateSet,
  ...
}:
{
  templates.definitions = { };

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
