{
  compare = { factory, family, nixpkgs, release ? "openssl_3_3", features ? {}, override ? {}, attrOverride ? {}, localSystem ? "x86_64-linux", crossSystem ? null, staticBuild ? false, hostFlags ? {}, project ? "both", probe ? "package", passthruDetail ? false }:
  let
    base = import nixpkgs { inherit localSystem crossSystem; config = { allowUnsupportedSystem = true; allowBroken = true; permittedInsecurePackages = [ "openssl-1.1.1w" "openssl-99.0" ]; }; };
    pkgs = if staticBuild then base.pkgsStatic else base;
    original = import (nixpkgs + "/pkgs/development/libraries/openssl");
    wiring = pkgs.lib.optionalAttrs (hostFlags != {}) { stdenv = pkgs.stdenv // { hostPlatform = pkgs.stdenv.hostPlatform // hostFlags; }; } // features;
    upstream = (pkgs.callPackages original wiring).${release};
    candidate = pkgs.callPackage factory wiring;
    identity = p: if !builtins.isAttrs p then { path = builtins.toString p; } else { inherit (p) name outputName; derivationPath = p.drvPath; outputPath = p.outPath; };
    recipe = p: builtins.readFile (builtins.unsafeDiscardStringContext p.drvPath);
    scripts = [ "postPatch" "postInstall" "postFixup" ];
    projection = p: {
      inherit (p) pname version outputs setOutputFlags separateDebugInfo configurePlatforms configureScript dontAddStaticConfigureFlags configureFlags makeFlags enableParallelBuilding;
      derivationPath = p.drvPath;
      outputPaths = pkgs.lib.genAttrs p.outputs (o: p.${o}.outPath);
      recipe = recipe p;
      source = identity p.src;
      sourceRecipe = recipe p.src;
      sourceUrls = p.src.urls;
      patches = builtins.map (p: { name = builtins.baseNameOf p; content = builtins.readFile p; storePath = builtins.toString (builtins.path { path = p; }); }) p.patches;
      nativeInputs = builtins.map identity p.nativeBuildInputs;
      buildInputs = builtins.map identity p.buildInputs;
      scripts = pkgs.lib.genAttrs scripts (n: p.${n});
      contexts = pkgs.lib.genAttrs scripts (n: builtins.getContext p.${n});
      meta = builtins.removeAttrs p.meta [ "position" ];
      testNames = builtins.attrNames p.tests;
    } // pkgs.lib.optionalAttrs passthruDetail { test = identity p.tests.pkg-config; testRecipe = recipe p.tests.pkg-config; };
    inspect = p: if probe == "arguments" then builtins.functionArgs (if p == "upstream" then original else factory)
      else if probe == "family" then pkgs.lib.genAttrs [ "openssl_1_1" "openssl_3" "openssl_3_3" ] (n: (if p == "upstream" then pkgs.callPackages original wiring else pkgs.callPackage family wiring).${n}.drvPath)
      else projection (((if p == "upstream" then upstream else candidate).override override).overrideAttrs (_: attrOverride));
  in if project == "both" then { upstream = inspect "upstream"; candidate = inspect "candidate"; } else inspect project;
}
