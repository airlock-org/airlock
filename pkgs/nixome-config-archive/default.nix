{
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

writeShellApplication {
  name = "nixome-config-archive-candidate";

  runtimeInputs = [
    coreutils
    gawk
    git
    gnugrep
    gnutar
    jq
    python3.pkgs.qubes-core-qrexec.dom0
    systemd
  ];

  text = builtins.readFile ../../scripts/archive-config-candidate.sh;

  meta = {
    description = "Validate a staged Nixome config candidate and archive it into config-vault";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
