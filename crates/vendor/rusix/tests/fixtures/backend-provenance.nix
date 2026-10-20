# Evaluation-only experiments. Context labels are observations, not Rusix IDs.
{ pkgs, scenario, placement, payload }:
let
  context = builtins.addErrorContext;
  childContext = value:
    if builtins.elem placement [ "child" "combined" ]
    then context "CHILD-ORIGIN" value else value;
  fieldContext = value:
    if builtins.elem placement [ "field" "combined" ]
    then context "FIELD-ORIGIN" value else value;
  callContext = value:
    if builtins.elem placement [ "call" "combined" ]
    then context "CALL-ORIGIN" value else value;
  value = if payload == "throw" then throw "deferred child failure"
    else if payload == "attrs" then { notAPackage = true; }
    else if payload == "nested-throw" then {
      type = "derivation";
      outPath = throw "deferred outPath failure";
    }
    else 1;
  child = childContext value;
  mk = attrs: callContext (pkgs.stdenv.mkDerivation ({
    name = "backend-provenance";
    dontUnpack = true;
  } // attrs));
  deps = fieldContext [ child ];
  drv =
    if scenario == "plain" then mk { custom = fieldContext child; }
    else if scenario == "nested-list" then mk { buildInputs = fieldContext [ [ child ] ]; }
    else if scenario == "propagated" then mk { propagatedBuildInputs = deps; }
    else if scenario == "copy" then
      let original = { inherit child; }; copied = { dep = original.child; };
      in mk { buildInputs = fieldContext [ copied.dep ]; }
    else if scenario == "lib-map" then mk { buildInputs = pkgs.lib.map (x: x) deps; }
    else if scenario == "lib-flatten" then mk { buildInputs = pkgs.lib.flatten (fieldContext [ [ child ] ]); }
    else if scenario == "reconstruct" then
      # Successful evaluation of the source ends its context before reconstruction.
      mk { buildInputs = pkgs.lib.map (x: if builtins.isInt x then x + 0 else x) deps; }
    else if scenario == "override" then
      ((pkgs.lib.makeOverridable ({ dep }: mk { buildInputs = fieldContext [ dep ]; }))
        { dep = pkgs.zlib; }).override { dep = child; }
    else if scenario == "overrideAttrs" then
      (mk { buildInputs = [ pkgs.zlib ]; }).overrideAttrs (_: { buildInputs = deps; })
    else if scenario == "finalAttrs" then
      callContext (pkgs.stdenv.mkDerivation (final: {
        name = "backend-provenance";
        dontUnpack = true;
        buildInputs = fieldContext [ final.passthru.dep ];
        passthru.dep = child;
        passthru.unused = throw "unused finalAttrs branch";
      }))
    else if scenario == "composed" then
      let producer = mk { buildInputs = deps; };
          middle = mk { propagatedBuildInputs = [ producer ]; };
      in mk { buildInputs = [ middle ]; }
    else mk { buildInputs = deps; };
in drv.drvPath
