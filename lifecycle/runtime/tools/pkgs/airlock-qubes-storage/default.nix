{
  buildPythonPackage,
  e2fsprogs,
  erofs-utils,
  gnutar,
  lib,
  lvm2,
  makeWrapper,
  setuptools,
  util-linux,
}:

buildPythonPackage {
  pname = "airlock-qubes-storage";
  version = "0.1.0";
  format = "pyproject";

  src = ./src;

  nativeBuildInputs = [
    makeWrapper
    setuptools
  ];

  postFixup = ''
    wrapProgram "$out/bin/airlock-closure-store" \
      --prefix PATH : ${lib.makeBinPath [
        e2fsprogs
        erofs-utils
        gnutar
        lvm2
        util-linux
      ]}
  '';

  pythonImportsCheck = [
    "airlock_qubes_storage"
    "airlock_qubes_storage.engine"
  ];

  meta = {
    description = "Airlock closure-backed Qubes storage adapter and immutable store engine";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
