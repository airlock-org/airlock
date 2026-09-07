{
  buildPythonPackage,
  lib,
  setuptools,
}:

buildPythonPackage {
  pname = "nixome-qubes-storage";
  version = "0.1.0";
  format = "pyproject";

  src = ./src;

  nativeBuildInputs = [
    setuptools
  ];

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
