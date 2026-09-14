{ buildPythonPackage, setuptools }:
buildPythonPackage {
  pname = "airlock-amd-vbios";
  version = "0.1.0";
  format = "pyproject";
  src = ./src;
  nativeBuildInputs = [ setuptools ];
  pythonImportsCheck = [
    "airlock_amd_vbios.firmware"
    "airlock_amd_vbios.runtime"
  ];
}
