{ python3, writeShellApplication }:
let
  python = python3.withPackages (ps: [ ps.qubes-core-admin-client ]);
in
writeShellApplication {
  name = "airlock-gui-gpu-check";
  text = ''
    exec ${python}/bin/python3 ${./src/check.py} "$@"
  '';
}
