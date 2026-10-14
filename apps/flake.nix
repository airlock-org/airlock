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
          airlock-ctrl = pkgs.callPackage ./airlock-ctrl/package.nix { };
          default = self.packages.${system}.airlock-ctrl;
        });
    };
}
