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
    if test -e /mnt/var/lib/qubes/qubes.xml; then
      echo "Refusing to replace an existing Qubes database during image preparation" >&2
      exit 1
    fi
    # NAR substitution can materialize holes in seed files. Always reconstruct
    # sparse allocation in the installed pool, even from a dense cached seed.
    mkdir -p /mnt/var/lib/airlock/closure-store
    cp -a --sparse=always ${config.system.build.usbPreparedGuestStore}/pool/. \
      /mnt/var/lib/airlock/closure-store/
    chmod -R ${lib.escapeShellArg config.airlock.image.preparedStoreMode} \
      /mnt/var/lib/airlock/closure-store
    mkdir -p /mnt/var/lib/qubes
    cp -a --sparse=always ${preparedRuntime}/qubes/. /mnt/var/lib/qubes/
    cp -a ${preparedRuntime}/pool-selections/. /mnt/var/lib/airlock/closure-store/
    chmod -R u+rwX /mnt/var/lib/qubes
    cp ${preparedRuntime}/receipt.json /mnt/var/lib/airlock/prepared-runtime.json
  '';
  ownRuntime = ''
    # Activation creates the qubes group; only then resolve its installed GID.
    nixos-enter --root /mnt -- chown -R root:qubes /var/lib/qubes
    chmod 0660 /mnt/var/lib/qubes/qubes.xml
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
        source = pkgs.writeText "airlock-usb-image" "airlock-usb-dev-v2\n";
        target = "/airlock-usb-image";
      }
    ];
  }) (old:
    assert lib.hasInfix "mount $rootDisk $mountPoint" old.buildCommand;
    assert lib.hasInfix "umount -R /mnt" old.buildCommand;
  {
    # Sparse writable disks belong in their final filesystem, never in the
    # installed Nix store. Keep upstream assembly and bootloader installation.
    # Seed before activation: upstream qubes-create initializes an empty
    # database when none exists. Set ownership after activation creates users.
    buildCommand = lib.replaceStrings
      [ "mount $rootDisk $mountPoint" "umount -R /mnt" ]
      [ ("mount $rootDisk $mountPoint\n" + installRuntime)
        (ownRuntime + "\numount -R /mnt") ] old.buildCommand;
    preVM = ''
      echo "Validating prepared guest store before image assembly"
      ${config.system.build.usbPreparedGuestStore.checkCommand}
    '' + old.preVM;
  }));

  environment.etc."airlock/usb-dev".text = "airlock-usb-dev-v2\n";
  environment.systemPackages = with pkgs; [
    (writeShellScriptBin "airlock-usb" ''
      export PATH=${lib.makeBinPath [ coreutils util-linux cryptsetup e2fsprogs git openssh openssl ]}:$PATH
      exec ${python3}/bin/python3 ${../tools/usb-dev.py} "$@"
    '')
    (writeShellScriptBin "airlock-usb-amd-preflight" ''
      exec ${python3}/bin/python3 ${../tools/amd-preflight.py} "$@"
    '')
    (lib.hiPrio (writeShellScriptBin "codex" ''
      if ! ${util-linux}/bin/mountpoint -q /home/user/.codex \
        && ! test -f /var/lib/airlock/usb/insecure-codex-auth; then
        echo 'Unlock credentials with: sudo airlock-usb vault-unlock' >&2
        exit 1
      fi
      export CODEX_HOME=/home/user/.codex
      exec ${codex}/bin/codex "$@"
    ''))
  ];
}
