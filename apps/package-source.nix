{ lib }:
lib.fileset.toSource {
  root = ./.;
  fileset = lib.fileset.unions [
    ./Cargo.toml ./Cargo.lock
    ./airlock-cli/Cargo.toml ./airlock-cli/src
    ./airlock-nix/Cargo.toml ./airlock-nix/src
    ./airlock-nix-service/Cargo.toml ./airlock-nix-service/src
    ./airlock-ctrl-gui/Cargo.toml ./airlock-ctrl-gui/src/main.rs
  ];
}
