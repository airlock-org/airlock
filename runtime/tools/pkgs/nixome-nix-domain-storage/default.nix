{
  lib,
  nixome-qubes-storage ? python3.pkgs.callPackage ../nixome-qubes-storage { },
  python3,
  qubes-core-admin ? python3.pkgs.qubes-core-admin,
  writeTextFile,
}:

let
  storageHelperPython = python3.withPackages (_: [
    qubes-core-admin
    nixome-qubes-storage
  ]);
in
writeTextFile {
  name = "nixome-nix-domain-storage";
  destination = "/bin/nixome-nix-domain-storage";
  executable = true;
  text =
    "#!${storageHelperPython}/bin/python3\n" + builtins.readFile ../../scripts/nix-domain-storage.py;

  meta = {
    description = "Manage Nixome nix-closure Qubes storage volumes";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
