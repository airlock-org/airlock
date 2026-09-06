{
  callPackage,
  coreutils,
  gawk,
  git,
  gnugrep,
  gnutar,
  jq,
  lib,
  python3,
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
    python3.pkgs.qubes-core-qrexec.dom0
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
