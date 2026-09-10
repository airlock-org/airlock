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
  configApply = callPackage ../nixome-config-apply { };
  configArchive = callPackage ../nixome-config-archive { };
  configStatus = callPackage ../nixome-config-status { };
in
writeShellApplication {
  name = "nixome-config-control";

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
    description = "Dom0 qrexec control entry point for Nixome config generations";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
