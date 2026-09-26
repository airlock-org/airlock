{ pkgs, lib, config, guestStore }:
let
  engine = pkgs.python3.pkgs.callPackage ../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage { };
  python = pkgs.python3.withPackages (_: [ engine ]);
  system = config.system.build.toplevel;
  closure = pkgs.closureInfo { rootPaths = [ system ]; };
  choices = lib.mapAttrs' (name: value: lib.nameValuePair
    # Attribute names cannot carry Nix store dependency context. The system
    # and closure inputs below retain the dependency on the parent system;
    # each value also retains its specialisation's dependency context.
    (builtins.unsafeDiscardStringContext "${system}/specialisation/${name}")
    (toString value.configuration.system.build.toplevel)
  ) config.specialisation;
  seed = pkgs.runCommand "airlock-unified-store-seed" {
    __structuredAttrs = true;
    unsafeDiscardReferences.out = true;
    buildInputs = [ guestStore.builderInputClosure ];
    exportReferencesGraph.builder = guestStore.builderInputRoots;
    nativeBuildInputs = [ python pkgs.erofs-utils pkgs.gnutar ];
    AIRLOCK_CLOSURE_STORE_INDEX_SOURCE = "file";
    passthru = {
      inherit (guestStore) descriptors;
      # Guest descriptors stay unchanged. Builder successor descriptors are
      # checked by prepare-runtime against their new persistent physical pins.
      checkCommand = ''
        ${guestStore.checkCommand}
        ${python}/bin/python ${./tools/prepare-host-store.py} --check ${seed}
      '';
    };
  } ''
    python ${./tools/prepare-host-store.py} ${guestStore} "$out" \
      ${closure}/registration ${system} \
      ${pkgs.writeText "airlock-host-boot-choices.json" (builtins.toJSON choices)}
  '';
in seed
