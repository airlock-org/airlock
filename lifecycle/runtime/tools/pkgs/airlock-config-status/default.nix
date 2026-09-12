{
  coreutils,
  gawk,
  jq,
  lib,
  writeShellApplication,
}:

writeShellApplication {
  name = "airlock-config-status";

  runtimeInputs = [
    coreutils
    gawk
    jq
  ];

  text = builtins.readFile ../../scripts/config-status.sh;

  meta = {
    description = "Report Airlock config topology and apply status as JSON";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
