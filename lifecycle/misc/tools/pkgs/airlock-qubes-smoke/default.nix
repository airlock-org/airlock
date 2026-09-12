{
  coreutils,
  gnugrep,
  gnused,
  lib,
  systemd,
  writeShellApplication,
}:

writeShellApplication {
  name = "airlock-qubes-smoke";

  runtimeInputs = [
    coreutils
    gnugrep
    gnused
    systemd
  ];

  text = builtins.readFile ./src/airlock-qubes-smoke.sh;

  meta = {
    description = "Run the Airlock qrexec smoke test on a booted Qubes dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
