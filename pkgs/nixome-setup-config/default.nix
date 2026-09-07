{
  coreutils,
  diffutils,
  gawk,
  gnugrep,
  gnused,
  jq,
  lib,
  python3,
  qubes-desktop-linux-common ? python3.pkgs.qubes-desktop-linux-common,
  qubes-core-admin ? python3.pkgs.qubes-core-admin,
  sudo,
  systemd,
  util-linux,
  writeShellApplication,
  writeTextFile,
}:

let
  storageHelperPython = python3.withPackages (_: [ qubes-core-admin ]);
  storageHelper = writeTextFile {
    name = "nixome-nix-domain-storage";
    destination = "/bin/nixome-nix-domain-storage";
    executable = true;
    text = "#!${storageHelperPython}/bin/python3\n" + builtins.readFile ../../scripts/nix-domain-storage.py;
  };
in
writeShellApplication {
  name = "nixome-setup-config";
  excludeShellChecks = [ "SC2016" ];

  runtimeInputs = [
    coreutils
    diffutils
    gawk
    gnugrep
    gnused
    jq
    python3
    qubes-core-admin
    qubes-desktop-linux-common
    storageHelper
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
