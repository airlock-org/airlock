{
  callPackage,
  coreutils,
  gawk,
  gnugrep,
  gnused,
  gnutar,
  jq,
  lib,
  nix,
  writeShellApplication,
}:
let
  configApply = callPackage ../airlock-config-apply { };
  configArchive = callPackage ../airlock-config-archive { };
  configStatus = callPackage ../airlock-config-status { };
in
writeShellApplication {
  name = "airlock-config-control";

  runtimeInputs = [
    configApply
    configArchive
    configStatus
    coreutils
    gawk
    gnugrep
    gnused
    gnutar
    jq
    nix
  ];

  text = builtins.readFile ../../scripts/config-control.sh;

  meta = {
    description = "Dom0 qrexec control entry point for Airlock config generations";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
