{
  description = "Standalone Airlock application builds and previews";

  inputs = {
    self.submodules = false;
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" ];
    in {
      packages = nixpkgs.lib.genAttrs systems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          airlock-ctrl-gui = pkgs.callPackage ./airlock-ctrl-gui/package.nix { };
          airlock-nix = pkgs.callPackage ./airlock-nix/package.nix { };
          airlock-cli = pkgs.callPackage ./airlock-cli/package.nix { };
          airlock-nix-service = pkgs.callPackage ./airlock-nix-service/package.nix { };
          default = self.packages.${system}.airlock-ctrl-gui;
        });
    };
}
