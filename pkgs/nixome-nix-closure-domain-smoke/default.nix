{
  coreutils,
  e2fsprogs,
  erofs-utils,
  gnugrep,
  gnused,
  jq,
  lib,
  python3,
  systemd,
  util-linux,
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
