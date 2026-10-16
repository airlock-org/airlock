{ lib, rustPlatform }:
rustPlatform.buildRustPackage {
  pname = "airlock-nix";
  version = "0.1.0";
  src = import ../package-source.nix { inherit lib; };
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "-p" "airlock-nix" "--bin" "airlock-nix" ];
  doCheck = false;
  meta = {
    description = "Inspect Airlock Nix records locally";
    license = lib.licenses.mit;
    mainProgram = "airlock-nix";
    platforms = lib.platforms.linux;
  };
}
