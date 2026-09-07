{
  coreutils,
  gawk,
  jq,
  lib,
  python3,
  writeShellApplication,
}:

writeShellApplication {
  name = "nixome-config-status";

  runtimeInputs = [
    coreutils
    gawk
    jq
    python3.pkgs.qubes-core-admin-client
  ];

  text = builtins.readFile ../../scripts/config-status.sh;

  meta = {
    description = "Report Nixome config topology and apply status as JSON";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
