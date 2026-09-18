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
          airlockMachineSeedArchive =
            if seedPath == "" then null else builtins.path { path = seedPath; name = "airlock-machine-seed.tar.gz"; };
          entries = builtins.removeAttrs (airlock-machine.lib.mkMachineGuestEntries system {
            inherit airlockMachineSeedArchive;
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
            inherit lib entries pkgs;
          };
          storeSources = lib.mapAttrs (name: _: "${preparedStore.descriptors}/${name}") entries;
        in
        airlock-machine.lib.mkMachineConfiguration {
          inherit system storeSources;
          includeRelease = false;
          includeAlternativeGuiProfiles = false;
          extraGuestSpecialArgs = {
            inherit airlockMachineSeedArchive;
          };
          extraSpecialArgs = {
            airlockUsbPreparedStore = preparedStore;
            airlockUsbGuestEntries = entries;
            inherit airlockMachineSeedArchive;
          };
          extraModules = [ ./lifecycle/image/profiles/usb-image.nix ];
        };

      usbConfiguration = mkUsbConfiguration "x86_64-linux";
    in
    {
      lib = airlock-machine.lib;
      overlays = airlock-machine.overlays;
      nixosModules = airlock-machine.nixosModules;
      checks = airlock-machine.checks;
      formatter = airlock-machine.formatter;
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
          airlock-usb-dev-image = usbConfiguration.config.system.build.image;
          airlock-usb-dom0-console-image =
            self.nixosConfigurations.usb-dom0-console.config.system.build.image;
        }
      );
    };
}
