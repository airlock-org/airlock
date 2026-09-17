{
  description = "Airlock development, USB image, and integration tooling";

  inputs = {
    self.submodules = true;
    airlock-os.url = "path:./os";
    nixpkgs.follows = "airlock-os/nixpkgs";
    nixos-qubes.follows = "airlock-os/nixos-qubes";
  };

  outputs =
    inputs@{
      self,
      airlock-os,
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
          entries = airlock-os.lib.machineGuestEntries system;
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              nixos-qubes.overlays.default
              airlock-os.overlays.airlockQubesGuiWindowState
              airlock-os.overlays.airlockImageBuildTools
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
        airlock-os.lib.mkMachineConfiguration {
          inherit system storeSources;
          includeRelease = false;
          extraSpecialArgs = {
            airlockUsbPreparedStore = preparedStore;
            airlockUsbGuestEntries = entries;
          };
          extraModules = [ ./lifecycle/image/profiles/usb-image.nix ];
        };

      usbConfiguration = mkUsbConfiguration "x86_64-linux";
    in
    {
      lib = airlock-os.lib;
      overlays = airlock-os.overlays;
      nixosModules = airlock-os.nixosModules;
      checks = airlock-os.checks;
      formatter = airlock-os.formatter;
      airlock = airlock-os.airlock;

      nixosConfigurations = airlock-os.nixosConfigurations // {
        usb-dev = usbConfiguration;
        usb-dom0-console = lib.nixosSystem {
          system = "x86_64-linux";
          modules = [
            airlock-os.nixosModules.dom0
            ./lifecycle/image/profiles/usb-dom0-console.nix
          ];
        };
      };

      packages = forAllSystems (
        system:
        removeAttrs airlock-os.packages.${system} [
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
