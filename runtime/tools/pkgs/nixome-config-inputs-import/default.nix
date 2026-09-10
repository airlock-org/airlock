{
  coreutils,
  gawk,
  gnugrep,
  gnutar,
  gzip,
  jq,
  lib,
  nix,
  python3,
  writeShellApplication,
}:

writeShellApplication {
  name = "nixome-config-inputs-import";

  runtimeInputs = [
    coreutils
    gawk
    gnugrep
    gnutar
    gzip
    jq
    nix
    python3.pkgs.qubes-core-admin-client
    python3.pkgs.qubes-core-qrexec.dom0
  ];

  text = builtins.readFile ../../scripts/import-config-inputs.sh;

  meta = {
    description = "Import Nixome config inputs into closure-backed config VMs";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
