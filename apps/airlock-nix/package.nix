{ lib, rustPlatform, role ? "inspect" }:
let
  executable = {
    inspect = "airlock-nix";
    client = "airlock-nix-client";
    controller = "airlock-nix-controller";
  }.${role};
in
rustPlatform.buildRustPackage {
  pname = executable;
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
  cargoBuildFlags = [ "-p" "airlock-nix" "--bin" executable ];
  doCheck = false;
  meta = {
    description = "Airlock Nix ${role} executable";
    license = lib.licenses.mit;
    mainProgram = executable;
    platforms = lib.platforms.linux;
  };
}
