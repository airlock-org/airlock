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
  name = "nixome-qube-gui-smoke";

  runtimeInputs = [
    coreutils
    gnugrep
    gnused
    systemd
    xdotool
  ];

  text = builtins.readFile ../../tests/qubes-gui-smoke.sh;

  meta = {
    description = "Run a Qubes GUI window smoke test on a Nixome dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
