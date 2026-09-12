{
  coreutils,
  gnugrep,
  gnused,
  lib,
  systemd,
  writeShellApplication,
  xdotool,
}:

writeShellApplication {
  name = "airlock-qube-gui-smoke";

  runtimeInputs = [
    coreutils
    gnugrep
    gnused
    systemd
    xdotool
  ];

  text = builtins.readFile ./src/airlock-qube-gui-smoke.sh;

  meta = {
    description = "Run a Qubes GUI window smoke test on a Airlock dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
