{
  lib,
  rustPlatform,
  makeWrapper,
  libGL,
  libxkbcommon,
  wayland,
  libx11,
  libxcursor,
  libxi,
  libxrandr,
  git,
}:
rustPlatform.buildRustPackage {
  pname = "airlock-ctrl";
  version = "0.1.0";
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [ ../Cargo.toml ../Cargo.lock ./Cargo.toml ./src ];
  };
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "-p" "airlock-ctrl" ];
  # Behavioural coverage is pytest, run against the explicitly built binary.
  doCheck = false;
  nativeBuildInputs = [ makeWrapper ];
  postInstall = ''
    wrapProgram "$out/bin/airlock-ctrl" \
      --prefix PATH : ${lib.makeBinPath [ git ]} \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [
        libGL libxkbcommon wayland libx11 libxcursor libxi libxrandr
      ]}
  '';
  meta = {
    description = "Airlock source and deployment review desktop application";
    license = lib.licenses.mit;
    mainProgram = "airlock-ctrl";
    platforms = lib.platforms.linux;
  };
}
