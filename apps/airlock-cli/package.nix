{ lib, rustPlatform, makeWrapper, machineCli ? null }:
rustPlatform.buildRustPackage {
  pname = "airlock-cli";
  version = "0.1.0";
  src = import ../package-source.nix { inherit lib; };
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "-p" "airlock-cli" "--bin" "airlock" ];
  doCheck = false;
  nativeBuildInputs = [ makeWrapper ];
  postInstall = lib.optionalString (machineCli != null) ''
    wrapProgram "$out/bin/airlock" --set AIRLOCK_MACHINE_CLI ${lib.getExe machineCli}
  '';
  meta = {
    description = "Airlock command line interface";
    license = lib.licenses.mit;
    mainProgram = "airlock";
    platforms = lib.platforms.linux;
  };
}
