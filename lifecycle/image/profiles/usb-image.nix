{ config, lib, pkgs, modulesPath, ... }:
let
  preparedRuntime = import ../prepared-runtime.nix {
    inherit pkgs;
    # Seed the normal boot profile, not the base console carrier or recovery.
    topology = if config.specialisation ? usb-lab
      then config.specialisation.usb-lab.configuration.airlock.qubes.topology
      else config.airlock.qubes.topology;
    preparedStore = config.system.build.usbPreparedGuestStore;
  };
  installRuntime = ''
    echo "Installing prepared Qubes runtime state"
    test ! -e /mnt/var/lib/qubes/qubes.xml
    mkdir -p /mnt/var/lib/qubes
    cp -a --sparse=always ${preparedRuntime}/qubes/. /mnt/var/lib/qubes/
    cp -a ${preparedRuntime}/pool-selections/. /mnt/var/lib/airlock/closure-store/
    chmod -R u+rwX /mnt/var/lib/qubes
    nixos-enter --root /mnt -- chown -R root:qubes /var/lib/qubes
    chmod 0660 /mnt/var/lib/qubes/qubes.xml
    cp ${preparedRuntime}/receipt.json /mnt/var/lib/airlock/prepared-runtime.json
  '';
in
{
  imports = [
    "${modulesPath}/virtualisation/disk-image.nix"
    ./usb-prepared-store.nix
  ];

  image = { baseName = "airlock-usb-dev"; format = "raw"; efiSupport = true; };
  virtualisation.diskSize = 32768;
  system.build.usbPreparedRuntime = preparedRuntime;
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
  }) (old: assert lib.hasInfix "umount -R /mnt" old.buildCommand; {
    # Sparse writable disks belong in their final filesystem, never in the
    # installed Nix store. Keep upstream assembly and bootloader installation.
    buildCommand = lib.replaceStrings [ "umount -R /mnt" ]
      [ (installRuntime + "\numount -R /mnt") ] old.buildCommand;
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
