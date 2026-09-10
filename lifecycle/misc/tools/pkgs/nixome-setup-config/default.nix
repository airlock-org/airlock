{
  coreutils,
  diffutils,
  e2fsprogs,
  erofs-utils,
  gawk,
  gnugrep,
  gnutar,
  gnused,
  jq,
  lib,
  lvm2,
  nix,
  nixome-nix-domain-storage ? callPackage ../../../../runtime/tools/pkgs/nixome-nix-domain-storage { },
  nixome-qubes-storage ? python3.pkgs.callPackage ../../../../runtime/tools/pkgs/nixome-qubes-storage { },
  python3,
  qubes-desktop-linux-common ? python3.pkgs.qubes-desktop-linux-common,
  qubes-core-admin ? python3.pkgs.qubes-core-admin,
  sudo,
  systemd,
  util-linux,
  callPackage,
  writeShellApplication,
}:
writeShellApplication {
  name = "nixome-setup-config";
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
    jq
    lvm2
    nix
    nixome-nix-domain-storage
    nixome-qubes-storage
    python3
    qubes-core-admin
    qubes-desktop-linux-common
    sudo
    python3.pkgs.qubes-core-admin-client
    systemd
    util-linux
  ];

  text = ''
    export NIXOME_NIX_DOMAIN_STORAGE_HELPER=''${NIXOME_NIX_DOMAIN_STORAGE_HELPER:-nixome-nix-domain-storage}
  ''
  + builtins.readFile ../../scripts/setup-config.sh;

  meta = {
    description = "Set up Nixome's declared Qubes config topology on dom0";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
