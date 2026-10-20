# Evaluation-only harness. Actual option declarations and merge/type machinery
# come from pinned upstream modules. Interop supplies pkgs; no system builder is loaded.
{ nixpkgs, module, selection, checkAssertions ? false, pkgs ? {}, packageSummary ? false }:
let
  lib = import (nixpkgs + "/lib");
  result = lib.evalModules {
    specialArgs = { inherit pkgs; }; # Empty on the original minimal evaluator.
    modules = [
      (nixpkgs + "/nixos/modules/misc/assertions.nix")
      ({ lib, ... }: {
        # Only support the unrelated side-effect namespaces referenced by sshd.
        # The real services.openssh options retain their upstream types. Unknown
        # services.* and unknown top-level paths are still checked normally.
        options = lib.genAttrs
          [ "environment" "systemd" "networking" "security" "users" "programs" "system" ]
          (_: lib.mkOption {
            type = lib.types.submodule { freeformType = lib.types.attrsOf lib.types.unspecified; };
            default = {};
          });
      })
      module
    ];
  };
  value = lib.foldl' (value: name: builtins.getAttr name value) result.config selection;
  selected = if packageSummary then map (package: {
    name = package.name;
    pname = package.pname or package.name;
    version = package.version or null;
    isDerivation = lib.isDerivation package;
  }) value else value;
  # Same failed-assertion filtering/message policy as pinned NixOS top-level.nix
  # lines 70-73; the system-building portion of that module is never imported.
  failedAssertions = map (x: x.message) (builtins.filter (x: !x.assertion) result.config.assertions);
in
if checkAssertions then
  if failedAssertions != [] then
    builtins.addErrorContext "rusix-stage:nixos-assertions" (
      throw "\nFailed assertions:\n${lib.concatStringsSep "\n" (map (x: "- ${x}") failedAssertions)}"
    )
  else selected
else selected
