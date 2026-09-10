{
  callPackage,
  coreutils,
  jq,
  lib,
  writeShellApplication,
}:

let
  configStatus = callPackage ../nixome-config-status { };
in
writeShellApplication {
  name = "nixome-config-boot-verify";

  runtimeInputs = [
    configStatus
    coreutils
    jq
  ];

  text = builtins.readFile ../../scripts/config-boot-verify.sh;

  meta = {
    description = "Verify dom0 has booted into the staged Nixome NixOS generation";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
