{
  config,
  lib,
  pkgs,
  airlockUsbPreparedStore,
  airlockUsbGuestEntries,
  ...
}:
let
  # The full topology still names guest systems and kernelopts. Detach its Nix
  # references because guest source packages now live inside the seeded pool.
  # Explicit dependencies below keep every artifact dom0 actually consumes.
  topology =
    pkgs.runCommand "airlock-usb-prepared-topology.json"
      {
        __structuredAttrs = true;
        unsafeDiscardReferences.out = true;
      }
      ''
        cp ${pkgs.writeText "airlock-usb-topology.json" (builtins.toJSON config.airlock.qubes.topology)} "$out"
      '';
in
{
  options.airlock.image.preparedStoreMode = lib.mkOption {
    type = lib.types.str;
    default = "go-rwx";
    description = "Mode adjustment applied while installing the prepared guest store.";
  };

  config = {
    environment.etc."airlock/qubes-topology.json" = {
      text = lib.mkForce null;
      source = topology;
    };
    system.extraDependencies = [
      airlockUsbPreparedStore.descriptors
      airlockUsbPreparedStore.validation
    ]
    ++ map (kernel: kernel.source) (builtins.attrValues config.airlock.qubes.kernels);
    # Only the disk builder consumes this output, placing it at its final path.
    # Do not add the payload to extraDependencies: that would copy it twice.
    system.build.usbPreparedGuestStore = airlockUsbPreparedStore;
  };
}
