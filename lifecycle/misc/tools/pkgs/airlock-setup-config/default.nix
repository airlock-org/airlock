{
  coreutils,
  diffutils,
  e2fsprogs,
  erofs-utils,
  gawk,
  gnugrep,
  gnutar,
  gnused,
  iproute2,
  jq,
  lib,
  lvm2,
  nix,
  airlock-nix-domain-storage ?
    callPackage ../../../../runtime/tools/pkgs/airlock-nix-domain-storage
      { },
  airlock-qubes-storage ?
    python3.pkgs.callPackage ../../../../runtime/tools/pkgs/airlock-qubes-storage
      { },
  python3,
  qubes-desktop-linux-common ? python3.pkgs.qubes-desktop-linux-common,
  qubes-core-admin ? python3.pkgs.qubes-core-admin,
  sudo,
  systemd,
  util-linux,
  callPackage,
  writeShellApplication,
}:
let
  qubesAdminPython = python3.withPackages (ps: [
    ps.qubes-core-admin-client
  ]);
in
writeShellApplication {
  name = "airlock-setup-config";
  excludeShellChecks = [ "SC2016" ];

  runtimeInputs = [
    coreutils
    diffutils
    e2fsprogs
    erofs-utils
    gawk
    gnugrep
    gnutar
    gnused
    iproute2
    jq
    lvm2
    nix
    airlock-nix-domain-storage
    airlock-qubes-storage
    (callPackage ../airlock-gui-gpu-check { })
    python3
    qubes-core-admin
    qubes-desktop-linux-common
    sudo
    python3.pkgs.qubes-core-admin-client
    systemd
    util-linux
  ];

  text = ''
    export AIRLOCK_NIX_DOMAIN_STORAGE_HELPER=''${AIRLOCK_NIX_DOMAIN_STORAGE_HELPER:-airlock-nix-domain-storage}
    export AIRLOCK_QUBESADMIN_PYTHON=''${AIRLOCK_QUBESADMIN_PYTHON:-${qubesAdminPython}/bin/python3}
  ''
  + builtins.readFile ../../scripts/setup-config.sh;

  meta = {
    description = "Set up Airlock's declared Qubes config topology on dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
