{
  pkgs,
  lib,
  entries,
  offlineInputs ? null,
}:
let
  engine = pkgs.python3.pkgs.callPackage ../../airlock-machine/lifecycle/runtime/tools/pkgs/airlock-qubes-storage { };
  python = pkgs.python3.withPackages (_: [ engine ]);
  guestSpecs = lib.mapAttrsToList (name: entry: {
    inherit name;
    domain = entry.preparedDomainName or "sys-gui";
    system = toString entry.configuration.config.system.build.toplevel;
    root_source = "${entry.rootImage}/root.img";
    kernel_name = entry.domain.backend.kernel.name;
    closure_source = toString entry.storeArtifact;
  }) entries;
  # These roots cover the build hooks and tools missing from the first sys-dev
  # guest change. The view shares objects already present in guest closures.
  routineBuildInputs = with pkgs; [
    composefs
    composefs.lib
    composefs.dev
    dieHook
    file
    gnu-config
    libcap
    pkgs."libcap-text-verifier"
    makeShellWrapper
    patchelf
    stdenvNoCC
    updateAutotoolsGnuConfigScriptsHook
  ];
  sourcePaths =
    if offlineInputs == null then [ ]
    else
      map builtins.storePath (
        lib.unique (
          lib.mapAttrsToList (
            _: source: builtins.elemAt (builtins.match "(/nix/store/[^/]+)(/.*)?" source) 0
          ) offlineInputs.inputs
        )
      );
  # A stable builder view shares the already admitted objects, including other
  # guests' software. It is independent of sys-dev's own boot generation.
  buildClosure = pkgs.closureInfo {
    rootPaths =
      map (entry: entry.configuration.config.system.build.toplevel) (builtins.attrValues entries)
      ++ lib.optionals (entries ? sys-dev) (routineBuildInputs ++ sourcePaths);
  };
  buildManifest = pkgs.runCommand "sys-dev-builder-closure" { } ''
    mkdir -p "$out"
    cp ${buildClosure}/store-paths ${buildClosure}/registration "$out/"
    echo '{}' > "$out/manifest.json"
    ${lib.optionalString (offlineInputs != null) ''
      cp ${pkgs.writeText "airlock-offline-inputs.json" (builtins.toJSON offlineInputs)} "$out/flake-inputs.json"
    ''}
  '';
  specs = guestSpecs ++ lib.map (role: {
    name = "${role}-builder";
    domain = "${role}-builder";
    system = toString entries.${role}.configuration.config.system.build.toplevel;
    root_source = "";
    kernel_name = "";
    closure_source = toString buildManifest;
    builder = true;
  }) (builtins.filter (role: builtins.hasAttr role entries) [ "sys-dev" "sys-ctrl" ]);
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
        passthru.builderInputClosure = buildClosure;
        passthru.builderNix = entries.sys-dev.configuration.pkgs.nix;
        AIRLOCK_CLOSURE_STORE_INDEX_SOURCE = "file";
        # Execute this in the consuming image derivation too: a cached receipt
        # cannot attest to payload bytes rebuilt after that receipt was made.
        passthru.checkCommand = "${python}/bin/python -m airlock_qubes_storage._engine.prepared --check ${seed}/pool ${seed.descriptors}";
        passthru.validation =
          pkgs.runCommand "airlock-usb-prepared-store-validation"
            {
              nativeBuildInputs = [ python ];
            }
            ''
              # Check finalized outputs: descriptors can survive while Nix rebuilds a
              # collected payload. The receipt contains no paths retaining that payload.
              ${seed.checkCommand} > "$out"
            '';
      }
      ''
        python -m airlock_qubes_storage._engine.prepared \
          "$out/pool" "$descriptors" ${pkgs.writeText "prepared-guest-specs.json" (builtins.toJSON specs)} \
          /var/lib/airlock/closure-store
      '';
in
seed
