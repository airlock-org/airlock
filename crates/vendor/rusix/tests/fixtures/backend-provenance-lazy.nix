{ pkgs, stdenv, earlyFailure ? false }:
let
  bad = builtins.addErrorContext "UNUSED-ORIGIN" (throw "unused provenance branch");
  mk = attrs: stdenv.mkDerivation ({ name = "lazy-backend-provenance"; dontUnpack = true; } // attrs);
  recursive = stdenv.mkDerivation (final: {
    pname = "lazy-final-attrs";
    version = "1";
    dontUnpack = true;
    buildInputs = pkgs.lib.optionals false [ bad ];
    passthru.seen = final.version;
    passthru.unused = final.passthru.unused;
  });
in if earlyFailure then (mk {
  hardeningDisable = [ "not-a-hardening-flag" ];
  buildInputs = [ bad ];
}).drvPath else {
  unusedDefault = ({ unused ? bad }: (mk {}).drvPath) {};
  excludedOptionalDependency = (mk { buildInputs = pkgs.lib.optionals false [ bad ]; }).drvPath;
  unusedBackendField = (mk { configureFlags = bad; passthru.unused = bad; }).name;
  finalAttrs = recursive.drvPath;
  finalAttrsOverride = (recursive.overrideAttrs (_: { version = "2"; })).seen;
}
