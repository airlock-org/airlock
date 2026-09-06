{
  callPackage,
  coreutils,
  gawk,
  git,
  gnugrep,
  gnutar,
  jq,
  lib,
  systemd,
  writeShellApplication,
}:
let
  configArchive = callPackage ../nixome-config-archive { };
  setupConfig = callPackage ../nixome-setup-config { };
in
writeShellApplication {
  name = "nixome-config-apply";

  runtimeInputs = [
    configArchive
    coreutils
    gawk
    git
    gnugrep
    gnutar
    jq
    setupConfig
    systemd
  ];

  text = builtins.readFile ../../scripts/apply-config-candidate.sh;

  meta = {
    description = "Apply a validator-approved Nixome config candidate from dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
