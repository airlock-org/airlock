{
  buildPythonPackage,
  e2fsprogs,
  erofs-utils,
  lib,
  lvm2,
  makeWrapper,
  setuptools,
  util-linux,
}:

buildPythonPackage {
  pname = "nixome-qubes-storage";
  version = "0.1.0";
  format = "pyproject";

  src = ./src;

  nativeBuildInputs = [
    makeWrapper
    setuptools
  ];

  postFixup = ''
    wrapProgram "$out/bin/nixome-closure-store" \
      --prefix PATH : ${lib.makeBinPath [
        e2fsprogs
        erofs-utils
        lvm2
        util-linux
      ]}
  '';

  pythonImportsCheck = [
    "nixome_qubes_storage"
    "nixome_qubes_storage.engine"
  ];

  meta = {
    description = "Nixome closure-backed Qubes storage adapter and immutable store engine";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
