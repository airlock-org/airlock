# Both factories receive the same real callPackage dependency scope. No source is built.
{
  compare =
    { factory, nixpkgs, features ? {}, localSystem ? "x86_64-linux", crossSystem ? null
    , override ? {}, attrOverride ? {}, project ? "both", hostFlags ? {}
    , libOverrides ? {}, passthruDetail ? false, model ? null, probe ? "package", staticBuild ? false
    }:
    let
      basePkgs = import nixpkgs {
        inherit localSystem crossSystem;
        config = { allowUnsupportedSystem = true; allowBroken = true; };
      };
      pkgs = if staticBuild then basePkgs.pkgsStatic else basePkgs;
      lib = pkgs.lib // libOverrides;
      wiring = { inherit lib; } // pkgs.lib.optionalAttrs (hostFlags != {}) {
        # An explicit branch probe, distinct from a supported target evaluation.
        stdenv = pkgs.stdenv // { hostPlatform = pkgs.stdenv.hostPlatform // hostFlags; };
      } // features;
      original = import (nixpkgs + "/pkgs/by-name/cu/curlMinimal/package.nix");
      instantiate = implementation:
        let base = pkgs.callPackage implementation (wiring // (if model == null then {} else model));
        in (base.override override).overrideAttrs (_: attrOverride);
      identity = package: if !builtins.isAttrs package then { path = builtins.toString package; } else {
        inherit (package) name outputName;
        derivationPath = package.drvPath;
        outputPath = package.outPath;
      };
      recipe = package: builtins.readFile (builtins.unsafeDiscardStringContext package.drvPath);
      projectTest = test:
        if pkgs.lib.isDerivation test then { package = identity test; recipe = recipe test; }
        else pkgs.lib.mapAttrs (_: projectTest) test;
      scripts = [ "postPatch" "preConfigure" "preCheck" "postInstall" ];
      projection = package: {
        inherit (package) name pname version outputs separateDebugInfo enableParallelBuilding
          strictDeps configureFlags CXX CXXCPP doCheck;
        env = package.env or {};
        derivationPath = package.drvPath;
        outputPath = package.outPath;
        outputPaths = pkgs.lib.genAttrs package.outputs (output: package.${output}.outPath);
        recipe = recipe package;
        source = identity package.src;
        sourceRecipe = recipe package.src;
        sourceUrls = package.src.urls;
        nativeInputs = builtins.map identity package.nativeBuildInputs;
        buildInputs = builtins.map identity (package.buildInputs or []);
        propagatedInputs = builtins.map identity package.propagatedBuildInputs;
        scripts = pkgs.lib.genAttrs scripts (name: package.${name});
        scriptContexts = pkgs.lib.genAttrs scripts (name: builtins.getContext package.${name});
        flagContexts = builtins.map builtins.getContext package.configureFlags;
        meta = {
          inherit (package.meta) changelog description homepage license maintainers platforms
            broken pkgConfigModules mainProgram;
        };
        passthru = {
          inherit (package) opensslSupport;
          openssl = identity package.openssl;
          names = builtins.attrNames package.tests;
          withCheck = identity package.tests.withCheck;
          withCheckRecipe = recipe package.tests.withCheck;
          withCheckEnabled = package.tests.withCheck.doCheck;
        } // pkgs.lib.optionalAttrs passthruDetail {
          # Force real recursive consuming packages and tests, without running them.
          tests = pkgs.lib.mapAttrs (_: projectTest) (builtins.removeAttrs package.tests [ "withCheck" ]);
        };
      };
      inspect = implementation:
        if probe == "arguments" then builtins.functionArgs implementation
        else projection (instantiate implementation);
    in
      if project == "upstream" then inspect original
      else if project == "candidate" then inspect factory
      else { upstream = inspect original; candidate = inspect factory; };
}
