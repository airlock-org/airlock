{ config, lib, pkgs, modulesPath, ... }:
{
  imports = [
    "${modulesPath}/virtualisation/disk-image.nix"
    ./usb-prepared-store.nix
  ];

  image = { baseName = "airlock-usb-dev"; format = "raw"; efiSupport = true; };
  virtualisation.diskSize = 32768;
  system.build.image = lib.mkForce (lib.overrideDerivation (import "${modulesPath}/../lib/make-disk-image.nix" {
    inherit config lib pkgs;
    inherit (config.image) baseName format;
    diskSize = config.virtualisation.diskSize;
    partitionTableType = "efi";
    bootSize = "1G";
    label = "airlock-usb";
    contents = [
      {
        source = pkgs.writeText "airlock-usb-image" "airlock-usb-dev-v1\n";
        target = "/airlock-usb-image";
      }
      {
        source = "${config.system.build.usbPreparedGuestStore}/pool";
        target = "/var/lib/airlock/closure-store";
        mode = config.airlock.image.preparedStoreMode;
      }
    ];
  }) (old: {
    preVM = ''
      echo "Validating prepared guest store before image assembly"
      ${config.system.build.usbPreparedGuestStore.checkCommand}
    '' + old.preVM;
  }));

  environment.etc."airlock/usb-dev".text = "airlock-usb-dev-v1\n";
  environment.systemPackages = with pkgs; [
    (writeShellScriptBin "airlock-usb" ''
      export PATH=${lib.makeBinPath [ coreutils util-linux cryptsetup e2fsprogs git openssh openssl ]}:$PATH
      exec ${python3}/bin/python3 ${../tools/usb-dev.py} "$@"
    '')
    (writeShellScriptBin "airlock-usb-amd-preflight" ''
      exec ${python3}/bin/python3 ${../tools/amd-preflight.py} "$@"
    '')
    (lib.hiPrio (writeShellScriptBin "codex" ''
      if ! ${util-linux}/bin/mountpoint -q /home/airlock/.codex \
        && ! test -f /var/lib/airlock/usb/insecure-codex-auth; then
        echo 'Unlock credentials with: sudo airlock-usb vault-unlock' >&2
        exit 1
      fi
      export CODEX_HOME=/home/airlock/.codex
      exec ${codex}/bin/codex "$@"
    ''))
  ];
}
