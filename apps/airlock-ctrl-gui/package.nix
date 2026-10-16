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
  dbus,
  glib,
  fontconfig,
}:
rustPlatform.buildRustPackage {
  pname = "airlock-ctrl-gui";
  version = "0.1.0";
  src = lib.fileset.toSource {
    root = ../.;
    # Cargo discovers sibling targets even when building only this app.
    fileset = lib.fileset.unions [
      ../Cargo.toml ../Cargo.lock ./Cargo.toml ./src ./assets ./examples
      ../airlock-nix/Cargo.toml ../airlock-nix/src/lib.rs ../airlock-nix/src/main.rs
      ../airlock-cli/Cargo.toml ../airlock-cli/src/main.rs
      ../airlock-nix-service/Cargo.toml ../airlock-nix-service/src/main.rs
    ];
  };
  cargoLock.lockFile = ../Cargo.lock;
  cargoBuildFlags = [ "-p" "airlock-ctrl-gui" ];
  # Behavioural coverage is pytest, run against the explicitly built binary.
  doCheck = false;
  nativeBuildInputs = [ makeWrapper ];
  postInstall = ''
    wrapProgram "$out/bin/airlock-ctrl-gui" \
      --prefix PATH : ${lib.makeBinPath [ git dbus glib fontconfig ]} \
      --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath [
        libGL libxkbcommon wayland libx11 libxcursor libxi libxrandr
      ]}
  '';
  meta = {
    description = "Airlock source and deployment review desktop application";
    license = lib.licenses.mit;
    mainProgram = "airlock-ctrl-gui";
    platforms = lib.platforms.linux;
  };
}
