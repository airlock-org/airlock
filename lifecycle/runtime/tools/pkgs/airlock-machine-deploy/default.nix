{
  lib,
  writeShellApplication,
  python3,
  nix,
  systemd,
  ...
}:
let
  storage = python3.pkgs.callPackage ../airlock-qubes-storage { };
in
writeShellApplication {
  name = "airlock-machine-deploy";
  runtimeInputs = [
    python3
    nix
    systemd
    storage
    python3.pkgs.qubes-core-admin-client
    python3.pkgs.qubes-core-qrexec.dom0
  ];
  text = ''
    exec ${python3}/bin/python3 ${../../machine-deploy.py} "$@"
  '';
  meta.description = "Repository-owned whole-machine Airlock deployment coordinator";
  meta.license = lib.licenses.mit;
}
