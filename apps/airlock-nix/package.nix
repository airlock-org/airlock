{ lib, rustPlatform }:
rustPlatform.buildRustPackage {
  pname = "airlock-nix";
  version = "0.1.0";
  src = lib.fileset.toSource {
    root = ../.;
    # Cargo resolves every workspace manifest, even for a single selected app.
    # The sibling entry point is included for target discovery, not compilation.
    fileset = lib.fileset.unions [
      ../Cargo.toml ../Cargo.lock ./Cargo.toml ./src
      ../airlock-ctrl/Cargo.toml ../airlock-ctrl/src/main.rs
    ];
  };
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "-p" "airlock-nix" ];
  doCheck = false;
  meta = {
    description = "Typed foundations and local record inspection for Airlock Nix management";
    license = lib.licenses.mit;
    mainProgram = "airlock-nix";
    platforms = lib.platforms.linux;
  };
}
