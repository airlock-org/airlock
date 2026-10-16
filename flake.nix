{
  description = "Airlock development, USB image, and integration tooling";

  inputs = {
    self.submodules = true;
    airlock-machine.url = "path:./airlock-machine";
    nixpkgs.follows = "airlock-machine/nixpkgs";
    nixos-qubes.follows = "airlock-machine/nixos-qubes";
  };

  outputs =
    inputs@{
      self,
      airlock-machine,
      nixpkgs,
      nixos-qubes,
      ...
    }:
    let
      inherit (nixpkgs) lib;
      supportedSystems = [ "x86_64-linux" ];
      forAllSystems = lib.genAttrs supportedSystems;

      mkUsbConfiguration = system:
        let
          seedPath = builtins.getEnv "AIRLOCK_MACHINE_SEED_ARCHIVE";
          offlineInputsPath = builtins.getEnv "AIRLOCK_MACHINE_OFFLINE_INPUTS";
          airlockMachineSeedArchive =
            if seedPath == "" then null else builtins.path { path = seedPath; name = "airlock-machine-seed.tar.gz"; };
          offlineInputs =
            if offlineInputsPath == "" then null
            else builtins.fromJSON (builtins.readFile offlineInputsPath);
          entries = builtins.removeAttrs (airlock-machine.lib.mkMachineGuestEntries system {
            inherit airlockMachineSeedArchive;
            airlockMachineOfflineInputs = offlineInputs;
          }) [ "sys-gui-hybrid" "sys-gui-standalone" ];
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              nixos-qubes.overlays.default
              airlock-machine.overlays.airlockQubesGuiWindowState
              airlock-machine.overlays.airlockImageBuildTools
            ];
            config.allowUnfreePredicate =
              pkg:
              builtins.elem (pkg.pname or (lib.getName pkg)) [
                "qubes-artwork"
                "qubes-vmm-stubdom-linux"
              ];
          };
          preparedStore = import ./lifecycle/image/prepared-guest-store.nix {
            inherit lib entries pkgs offlineInputs;
          };
          storeSources = lib.mapAttrs (name: _: "${preparedStore.descriptors}/${name}") entries;
        in
        airlock-machine.lib.mkMachineConfiguration {
          inherit system storeSources;
          includeRelease = false;
          includeAlternativeGuiProfiles = false;
          extraGuestSpecialArgs = {
            inherit airlockMachineSeedArchive;
            airlockMachineOfflineInputs = offlineInputs;
          };
          extraSpecialArgs = {
            airlockUsbPreparedStore = preparedStore;
            airlockMachineSource = if offlineInputs != null && offlineInputs ? revision then
              assert offlineInputs.lock_hash == builtins.hashFile "sha256" (airlock-machine + "/flake.lock");
              { revision = offlineInputs.revision; lockHash = offlineInputs.lock_hash; }
              else null;
            airlockSourceRevision = self.rev or "dirty";
            airlockSourceTimestamp = self.lastModified or 0;
            airlockUsbGuestEntries = entries;
            inherit airlockMachineSeedArchive;
          };
          # Keep the lab defaults; callers can construct another machine through
          # airlock-machine.lib.mkMachineConfiguration { hardwareModules = [ ... ]; }.
          extraModules = [ ./lifecycle/image/profiles/usb-image.nix ];
        };

      usbConfiguration = mkUsbConfiguration "x86_64-linux";
    in
    {
      lib = airlock-machine.lib;
      overlays = airlock-machine.overlays;
      nixosModules = airlock-machine.nixosModules;
      checks = airlock-machine.checks;
      airlock = airlock-machine.airlock;

      nixosConfigurations = airlock-machine.nixosConfigurations // {
        usb-dev = usbConfiguration;
        usb-dom0-console = lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            airlock-machine.nixosModules.dom0
            ./lifecycle/image/profiles/usb-dom0-console.nix
          ];
        };
      };

      packages = forAllSystems (
        system:
        removeAttrs airlock-machine.packages.${system} [
          "airlock-machine-release-usb-lab"
          "airlock-machine-release-usb-gui-lab"
        ]
        // {
          airlock-ctrl-gui = (import nixpkgs { inherit system; }).callPackage ./apps/airlock-ctrl-gui/package.nix { };
          airlock-prepare-runtime = import ./lifecycle/image/tools/prepare-runtime.nix { pkgs = usbConfiguration.pkgs; };
          airlock-usb-builder-input-closure = usbConfiguration.config.system.build.usbPreparedGuestStore.builderInputClosure;
          airlock-usb-builder-nix = usbConfiguration.config.system.build.usbPreparedGuestStore.builderNix;
          airlock-usb-dev-image = usbConfiguration.config.system.build.image;
          airlock-usb-dom0-console-image =
            self.nixosConfigurations.usb-dom0-console.config.system.build.image;
        }
        // lib.optionalAttrs (builtins.getEnv "AIRLOCK_HOST_REPAIR_SNAPSHOT" != "") {
          airlock-machine-host-repair =
            let snapshot = builtins.fromJSON (builtins.readFile (builtins.getEnv "AIRLOCK_HOST_REPAIR_SNAPSHOT"));
            in import ./lifecycle/image/host-repair.nix {
              inherit snapshot;
              baseline = builtins.getFlake snapshot.baselineFlake;
              repairModule = airlock-machine + "/lifecycle/misc/modules/machine-deployment.nix";
              repairRevision = snapshot.repairRevision;
            };
        }
      );
    };
}
