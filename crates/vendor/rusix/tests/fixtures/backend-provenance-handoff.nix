# Test-only stdenv adapter. The real backend remains authoritative and unchanged.
{ instrument = { stdenv, placement }:
let
  context = builtins.addErrorContext;
  selected = [ "buildInputs" "nativeBuildInputs" "propagatedBuildInputs" "configureFlags" "cmakeFlags" ];
  wrap = name: value:
    let
      children = if builtins.elem placement [ "child" "combined" ]
        then map (context "HANDOFF-CHILD:${name}") value else value;
    in if builtins.elem placement [ "field" "combined" ]
      then context "HANDOFF-FIELD:${name}" children else children;
  wrapAttrs = builtins.mapAttrs (name: value:
    if builtins.elem name selected then wrap name value else value);
  wrapCall = value: if builtins.elem placement [ "call" "combined" ]
    then context "HANDOFF-CALL" value else value;
in stdenv // {
  mkDerivation = attrs: wrapCall (stdenv.mkDerivation (
    if builtins.isFunction attrs then final: wrapAttrs (attrs final)
    else wrapAttrs attrs
  ));
}; }
