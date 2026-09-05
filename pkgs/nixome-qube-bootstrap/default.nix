{
  coreutils,
  curl,
  gnugrep,
  gnused,
  lib,
  systemd,
  writeShellApplication,
}:

writeShellApplication {
  name = "nixome-qube-bootstrap";

  runtimeInputs = [
    coreutils
    curl
    gnugrep
    gnused
    systemd
  ];

  text = builtins.readFile ../../scripts/bootstrap-qubes-assets.sh;

  meta = {
    description = "Bootstrap a Qubes VM kernel and TemplateVM on a Nixome dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
