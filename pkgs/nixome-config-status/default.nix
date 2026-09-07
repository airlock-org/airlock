{
  coreutils,
  gawk,
  jq,
  lib,
  writeShellApplication,
}:

writeShellApplication {
  name = "nixome-config-status";

  runtimeInputs = [
    coreutils
    gawk
    jq
  ];

  text = builtins.readFile ../../scripts/config-status.sh;

  meta = {
    description = "Report Nixome config topology and apply status as JSON";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
