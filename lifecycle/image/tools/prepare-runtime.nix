{ pkgs }:
let
  storage = pkgs.python3.pkgs.callPackage ../../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage { };
  python = pkgs.python3.withPackages (ps: [ ps.qubes-core-admin storage ]);
in
pkgs.writeShellScriptBin "airlock-prepare-runtime" ''
  export PATH=${pkgs.lib.makeBinPath [ pkgs.coreutils pkgs.e2fsprogs ]}:$PATH
  exec ${python}/bin/python3 ${./prepare-runtime.py} "$@" \
    ${../../../airlock-machine/lifecycle/runtime/tools/scripts/nix-domain-storage.py}
''
