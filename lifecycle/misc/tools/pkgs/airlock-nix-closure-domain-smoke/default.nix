{
  coreutils,
  e2fsprogs,
  erofs-utils,
  gnugrep,
  gnused,
  jq,
  lib,
  airlock-config-apply ? callPackage ../../../../runtime/tools/pkgs/airlock-config-apply { },
  airlock-nix-domain-storage ? callPackage ../../../../runtime/tools/pkgs/airlock-nix-domain-storage { },
  airlock-qube-gui-smoke ? callPackage ../airlock-qube-gui-smoke { },
  airlock-qubes-storage ? python3.pkgs.callPackage ../../../../runtime/tools/pkgs/airlock-qubes-storage { },
  airlock-setup-config ? callPackage ../airlock-setup-config { },
  python3,
  systemd,
  util-linux,
  callPackage,
  writeShellApplication,
}:

writeShellApplication {
  name = "airlock-nix-closure-domain-smoke";
  excludeShellChecks = [ "SC2064" ];

  runtimeInputs = [
    coreutils
    e2fsprogs
    erofs-utils
    gnugrep
    gnused
    jq
    airlock-config-apply
    airlock-nix-domain-storage
    airlock-qube-gui-smoke
    airlock-qubes-storage
    airlock-setup-config
    python3
    python3.pkgs.qubes-core-admin-client
    python3.pkgs.qubes-core-qrexec.dom0
    systemd
    util-linux
  ];

  text = builtins.readFile ../../scripts/nix-closure-domain-smoke.sh;

  meta = {
    description = "Validate a Airlock nix-closure domain through a real Qubes VM lifecycle";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
