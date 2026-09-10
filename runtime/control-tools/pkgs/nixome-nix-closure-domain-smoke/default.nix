{
  coreutils,
  e2fsprogs,
  erofs-utils,
  gnugrep,
  gnused,
  jq,
  lib,
  nixome-config-apply ? callPackage ../nixome-config-apply { },
  nixome-nix-domain-storage ? callPackage ../nixome-nix-domain-storage { },
  nixome-qube-gui-smoke ? callPackage ../nixome-qube-gui-smoke { },
  nixome-qubes-storage ? python3.pkgs.callPackage ../nixome-qubes-storage { },
  nixome-setup-config ? callPackage ../nixome-setup-config { },
  python3,
  systemd,
  util-linux,
  callPackage,
  writeShellApplication,
}:

writeShellApplication {
  name = "nixome-nix-closure-domain-smoke";
  excludeShellChecks = [ "SC2064" ];

  runtimeInputs = [
    coreutils
    e2fsprogs
    erofs-utils
    gnugrep
    gnused
    jq
    nixome-config-apply
    nixome-nix-domain-storage
    nixome-qube-gui-smoke
    nixome-qubes-storage
    nixome-setup-config
    python3
    python3.pkgs.qubes-core-admin-client
    python3.pkgs.qubes-core-qrexec.dom0
    systemd
    util-linux
  ];

  text = builtins.readFile ../../scripts/nix-closure-domain-smoke.sh;

  meta = {
    description = "Validate a Nixome nix-closure domain through a real Qubes VM lifecycle";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
