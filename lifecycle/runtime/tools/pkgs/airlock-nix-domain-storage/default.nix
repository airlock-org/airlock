{
  lib,
  airlock-qubes-storage ? python3.pkgs.callPackage ../airlock-qubes-storage { },
  python3,
  qubes-core-admin ? python3.pkgs.qubes-core-admin,
  writeTextFile,
}:

let
  storageHelperPython = python3.withPackages (_: [
    qubes-core-admin
    airlock-qubes-storage
  ]);
in
writeTextFile {
  name = "airlock-nix-domain-storage";
  destination = "/bin/airlock-nix-domain-storage";
  executable = true;
  text =
    "#!${storageHelperPython}/bin/python3\n" + builtins.readFile ../../scripts/nix-domain-storage.py;

  meta = {
    description = "Manage Airlock nix-closure Qubes storage volumes";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
