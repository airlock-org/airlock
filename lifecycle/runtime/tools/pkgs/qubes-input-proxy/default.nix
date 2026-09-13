{
  lib,
  stdenv,
  fetchFromGitHub,
}:
stdenv.mkDerivation {
  pname = "qubes-input-proxy";
  version = "1.0.46";

  src = fetchFromGitHub {
    owner = "QubesOS";
    repo = "qubes-app-linux-input-proxy";
    rev = "ea8710166c6ac29dd4183cdf155bc9a1da34cf4a";
    hash = "sha256-4yfdBcdAlahKnV76htdOojWgordZyM0H7JfvXzn8owE=";
  };

  # Keep the event protocol and filtering upstream. NixOS supplies service
  # activation and device selection instead of upstream's distro udev rules.
  buildPhase = ''
    runHook preBuild
    make -C src
    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall
    install -Dm755 src/input-proxy-sender "$out/bin/input-proxy-sender"
    install -Dm755 src/input-proxy-receiver "$out/bin/input-proxy-receiver"
    for service in qubes.InputKeyboard qubes.InputMouse qubes.InputTablet; do
      install -Dm755 "qubes-rpc/$service" "$out/etc/qubes-rpc/$service"
      substituteInPlace "$out/etc/qubes-rpc/$service" \
        --replace-fail /usr/bin/input-proxy-receiver "$out/bin/input-proxy-receiver"
    done
    runHook postInstall
  '';

  meta = {
    description = "Upstream Qubes evdev/uinput event proxy";
    homepage = "https://github.com/QubesOS/qubes-app-linux-input-proxy";
    license = lib.licenses.gpl2Plus;
    platforms = lib.platforms.linux;
  };
}
