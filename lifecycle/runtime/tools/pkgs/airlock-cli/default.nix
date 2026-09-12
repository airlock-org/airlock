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
  name = "airlock";

  runtimeInputs = [
    coreutils
    gitMinimal
    gnugrep
    gnutar
    jq
    python3.pkgs.qubes-core-qrexec.domU
  ];

  text = builtins.readFile ../../scripts/airlock-cli.sh;

  meta = {
    description = "Submit Airlock config candidates from an unprivileged Qubes VM";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
