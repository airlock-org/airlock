{
  coreutils,
  gitMinimal,
  gnugrep,
  gnutar,
  jq,
  lib,
  python3,
  writeShellApplication,
}:

writeShellApplication {
  name = "nixome";

  runtimeInputs = [
    coreutils
    gitMinimal
    gnugrep
    gnutar
    jq
    python3.pkgs.qubes-core-qrexec.domU
  ];

  text = builtins.readFile ../../scripts/nixome-cli.sh;

  meta = {
    description = "Submit Nixome config candidates from an unprivileged Qubes VM";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
