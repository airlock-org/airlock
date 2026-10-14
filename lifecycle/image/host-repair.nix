{ snapshot, baseline, repairModule, repairRevision }:
let
  lib = baseline.inputs.nixpkgs.lib;
  # Runtime topology includes normalized aliases that are not Nix options.
  declarative = value:
    if builtins.isList value then map declarative value
    else if builtins.isAttrs value then lib.mapAttrs' (name: item: {
      name = { source_ref = "sourceRef"; read_only = "readOnly";
               persistent_size = "persistentSize"; }.${name} or name;
      value = declarative item;
    }) (builtins.removeAttrs value [ "blob_device" "view_backend" ])
    else value;
  configuration = baseline.lib.mkMachineConfiguration {
    system = "x86_64-linux";
    includeRelease = false;
    includeAlternativeGuiProfiles = false;
    extraModules = [ ({ lib, ... }: {
      disabledModules = [ "${baseline}/lifecycle/misc/modules/machine-deployment.nix" ];
      imports = [ repairModule ];
      airlock.machineDeployment.initialSource = {
        revision = snapshot.revision;
        lockHash = snapshot.lockHash;
      };
      airlock.qubes.domains = lib.mkOverride 40 (declarative snapshot.topology.domains);
      airlock.qubes.kernels = lib.mkOverride 40 snapshot.topology.kernels;
      airlock.qubes.prefs = lib.mkOverride 40 snapshot.topology.prefs;
      airlock.qubes.qrexecPolicy = lib.mkOverride 40 snapshot.topology.qrexec_policy;
      environment.etc."airlock/host-repair.json".text = builtins.toJSON {
        inherit repairRevision;
        baselineHost = snapshot.host;
        baselineRevision = snapshot.revision;
      };
    }) ];
  };
  config = configuration.config.specialisation.usb-lab.configuration;
  pkgs = configuration.pkgs;
  host = config.system.build.toplevel;
  engine = config.system.build.machineDeploymentCoordinator;
  setup = pkgs.callPackage ../../airlock-machine/lifecycle/misc/tools/pkgs/airlock-setup-config { };
  manifest = {
    schema = "airlock.machine-release.v1";
    profile = snapshot.profile;
    revision = snapshot.revision;
    lockHash = snapshot.lockHash;
    inherit host;
    coordinator = "${engine}/bin/airlock-machine-deploy";
    setup = "${setup}/bin/airlock-setup-config";
    retireDomains = [ ];
    hostRepair = {
      baselineHost = snapshot.host;
      topologyHash = snapshot.topologyHash;
      inherit repairRevision;
    };
  };
in
assert baseline.rev == snapshot.revision;
assert snapshot.lockHash == builtins.hashFile "sha256" (baseline + "/flake.lock");
assert config.airlock.qubes.topology == snapshot.topology;
pkgs.runCommand "airlock-machine-host-repair" {
  releaseJson = pkgs.writeText "airlock-host-repair-release.json" (builtins.toJSON manifest);
  topologyJson = pkgs.writeText "airlock-host-repair-topology.json" (builtins.toJSON snapshot.topology);
} ''
  mkdir -p "$out"
  ln -s "$releaseJson" "$out/release.json"
  ln -s "$topologyJson" "$out/topology.json"
''
