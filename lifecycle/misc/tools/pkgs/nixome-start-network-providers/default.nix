{
  coreutils,
  jq,
  lib,
  python3,
  writeShellApplication,
}:
writeShellApplication {
  name = "nixome-start-network-providers";

  runtimeInputs = [
    coreutils
    jq
    python3.pkgs.qubes-core-admin-client
  ];

  text = builtins.readFile ../../scripts/start-network-providers.sh;

  meta = {
    description = "Start declared Nixome root network-provider qubes after setup-config";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
