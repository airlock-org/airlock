{
  coreutils,
  curl,
  gnugrep,
  gnused,
  lib,
  rpm,
  systemd,
  writeShellApplication,
}:

writeShellApplication {
  name = "airlock-qube-bootstrap";

  runtimeInputs = [
    coreutils
    curl
    gnugrep
    gnused
    rpm
    systemd
  ];

  text = builtins.readFile ../../scripts/bootstrap-qubes-assets.sh;

  meta = {
    description = "Bootstrap a Qubes VM kernel and TemplateVM on a Airlock dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
