{ lib, rustPlatform }:
rustPlatform.buildRustPackage {
  pname = "airlock-nix-service";
  version = "0.1.0";
  src = import ../package-source.nix { inherit lib; };
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "-p" "airlock-nix-service" "--bin" "airlock-nix-service" ];
  doCheck = false;
  meta = {
    description = "Airlock Nix review and deployment service";
    license = lib.licenses.mit;
    mainProgram = "airlock-nix-service";
    platforms = lib.platforms.linux;
  };
}
