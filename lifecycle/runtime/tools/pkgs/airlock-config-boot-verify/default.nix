{
  callPackage,
  coreutils,
  jq,
  lib,
  writeShellApplication,
}:

let
  configStatus = callPackage ../airlock-config-status { };
in
writeShellApplication {
  name = "airlock-config-boot-verify";

  runtimeInputs = [
    configStatus
    coreutils
    jq
  ];

  text = builtins.readFile ../../scripts/config-boot-verify.sh;

  meta = {
    description = "Verify dom0 has booted into the staged Airlock NixOS generation";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
