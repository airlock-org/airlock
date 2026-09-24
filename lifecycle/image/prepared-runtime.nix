{ pkgs, topology, preparedStore }:
let
  tool = import ./tools/prepare-runtime.nix { inherit pkgs; };
in
pkgs.runCommand "airlock-usb-prepared-runtime" {
  __structuredAttrs = true;
  # Paths in qubes.xml identify installed guest generations. They must not
  # pull a second copy of every guest's software into dom0's runtime store.
  unsafeDiscardReferences.out = true;
} ''
  ${preparedStore.checkCommand}
  ${tool}/bin/airlock-prepare-runtime \
    ${pkgs.writeText "airlock-prepared-runtime-topology.json" (builtins.toJSON topology)} \
    ${preparedStore}/pool "$TMPDIR/runtime"
  mv "$TMPDIR/runtime" "$out"
''
