{
  pkgs,
  lib,
  entries,
}:
let
  engine = pkgs.python3.pkgs.callPackage ../runtime/tools/pkgs/airlock-qubes-storage { };
  python = pkgs.python3.withPackages (_: [ engine ]);
  specs = lib.mapAttrsToList (name: entry: {
    inherit name;
    domain = "sys-gui";
    system = toString entry.configuration.config.system.build.toplevel;
    root_source = "${entry.rootImage}/root.img";
    kernel_name = entry.domain.backend.kernel.name;
    closure_source = toString entry.storeArtifact;
  }) entries;
  seed =
    pkgs.runCommand "airlock-usb-prepared-guest-store"
      {
        __structuredAttrs = true;
        outputs = [
          "out"
          "descriptors"
        ];
        nativeBuildInputs = [
          python
          pkgs.erofs-utils
          pkgs.gnutar
        ];
        # The payload contains its own guest store objects. Their original Nix paths
        # are identities inside the guest, not dependencies of the installed dom0.
        # Like NixOS image artifacts, neither output should retain those sources.
        unsafeDiscardReferences.out = true;
        unsafeDiscardReferences.descriptors = true;
        AIRLOCK_CLOSURE_STORE_INDEX_SOURCE = "file";
        passthru.validation =
          pkgs.runCommand "airlock-usb-prepared-store-validation"
            {
              nativeBuildInputs = [ python ];
            }
            ''
              # Check finalized outputs: descriptors can survive while Nix rebuilds a
              # collected payload. The receipt contains no paths retaining that payload.
              python -m airlock_qubes_storage._engine.prepared --check \
                ${seed}/pool ${seed.descriptors} > "$out"
            '';
      }
      ''
        python -m airlock_qubes_storage._engine.prepared \
          "$out/pool" "$descriptors" ${pkgs.writeText "prepared-guest-specs.json" (builtins.toJSON specs)} \
          /var/lib/airlock/closure-store
      '';
in
seed
