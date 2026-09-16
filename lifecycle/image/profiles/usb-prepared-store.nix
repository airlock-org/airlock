{
  config,
  lib,
  pkgs,
  airlockUsbPreparedStore,
  airlockUsbGuestEntries,
  airlockSourceRevision,
  airlockSourceLockHash,
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
  # Deployment and image use the same evaluated systems. Only their transport
  # differs: seeded pool descriptors on USB, ordinary closures for deployment.
  deploymentTopology = config.airlock.qubes.topology // {
    domains = lib.mapAttrs (
      _: domain:
      if domain.backend.type != "nix-closure" then
        domain
      else
        let
          matches = builtins.filter (
            entry: toString entry.configuration.config.system.build.toplevel == domain.backend.system.source
          ) (builtins.attrValues airlockUsbGuestEntries);
          entry =
            if matches == [ ] then
              throw "No closure artifact for machine deployment domain"
            else
              builtins.head matches;
        in
        domain
        // {
          backend = domain.backend // {
            store = domain.backend.store // {
              source = toString entry.storeArtifact;
            };
          };
        }
    ) config.airlock.qubes.topology.domains;
  };
  setup = pkgs.callPackage ../../misc/tools/pkgs/airlock-setup-config { };
  engine = pkgs.callPackage ../../runtime/tools/pkgs/airlock-machine-deploy { };
  releaseManifest = pkgs.writeText "airlock-machine-release.json" (
    builtins.toJSON {
      schema = "airlock.machine-release.v1";
      profile = config.airlock.machineDeployment.profile;
      host = toString config.system.build.toplevel;
      setup = "${setup}/bin/airlock-setup-config";
      coordinator = "${engine}/bin/airlock-machine-deploy";
      revision = airlockSourceRevision;
      lockHash = airlockSourceLockHash;
    }
  );
  releaseTopology = pkgs.writeText "airlock-machine-topology.json" (
    builtins.toJSON deploymentTopology
  );
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
    ++ map (entry: entry.rootImage) (builtins.attrValues airlockUsbGuestEntries)
    ++ map (kernel: kernel.source) (builtins.attrValues config.airlock.qubes.kernels);
    # Only the disk builder consumes this output, placing it at its final path.
    # Do not add the payload to extraDependencies: that would copy it twice.
    system.build.usbPreparedGuestStore = airlockUsbPreparedStore;
    system.build.airlockMachineRelease = pkgs.runCommand "airlock-machine-release" { } ''
      mkdir -p "$out"
      cp ${releaseManifest} "$out/release.json"
      cp ${releaseTopology} "$out/topology.json"
    '';
  };
}
