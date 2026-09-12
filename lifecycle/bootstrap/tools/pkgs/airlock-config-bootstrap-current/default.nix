{
  coreutils,
  gawk,
  gnugrep,
  gnutar,
  jq,
  lib,
  python3,
  writeShellApplication,
}:

writeShellApplication {
  name = "airlock-config-bootstrap-current";

  runtimeInputs = [
    coreutils
    gawk
    gnugrep
    gnutar
    jq
    python3.pkgs.qubes-core-admin-client
    python3.pkgs.qubes-core-qrexec.dom0
  ];

  text = builtins.readFile ../../scripts/bootstrap-config-current.sh;

  meta = {
    description = "Bootstrap sys-config-staging from the current sys-config-vault baseline";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
