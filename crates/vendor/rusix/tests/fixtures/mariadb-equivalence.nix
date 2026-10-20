{
  compare = { factory, family, nixpkgs, release ? "mariadb_1011", version, hash, features ? {}, override ? {}, attrOverride ? {}, clientAttrOverride ? {}, localSystem ? "x86_64-linux", crossSystem ? null, hostFlags ? {}, project ? "both", probe ? "package", passthruDetail ? false, testName ? null }:
  let
    pkgs = import nixpkgs { inherit localSystem crossSystem; config = { allowUnsupportedSystem = true; allowBroken = true; }; };
    wiring = pkgs.lib.optionalAttrs (hostFlags != {}) { stdenv = pkgs.stdenv // { hostPlatform = pkgs.stdenv.hostPlatform // hostFlags; }; } // features;
    upstream = ((import (nixpkgs + "/pkgs/servers/sql/mariadb") pkgs).${release}).override wiring;
    candidate = pkgs.callPackage factory ({ inherit version hash; inherit (pkgs.darwin.apple_sdk.frameworks) CoreServices; } // wiring);
    identity = p: if !builtins.isAttrs p then { path = builtins.toString p; } else { inherit (p) name outputName; derivationPath = p.drvPath; outputPath = p.outPath; };
    recipe = p: builtins.readFile (builtins.unsafeDiscardStringContext p.drvPath);
    scripts = [ "prePatch" "postPatch" "preConfigure" "postInstall" "postFixup" ];
    projectTest = test: if pkgs.lib.isDerivation test then { package = identity test; recipe = recipe test; } else pkgs.lib.mapAttrs (_: projectTest) test;
    projection = p: {
      inherit (p) pname version outputs cmakeFlags;
      derivationPath = p.drvPath;
      outputPaths = pkgs.lib.genAttrs p.outputs (o: p.${o}.outPath);
      recipe = recipe p;
      source = identity p.src;
      sourceRecipe = recipe p.src;
      sourceUrls = p.src.urls;
      patches = builtins.map (p: { name = builtins.baseNameOf p; content = builtins.readFile p; storePath = builtins.toString (builtins.path { path = p; }); }) p.patches;
      nativeInputs = builtins.map identity p.nativeBuildInputs;
      buildInputs = builtins.map identity p.buildInputs;
      propagatedInputs = builtins.map identity (p.propagatedBuildInputs or []);
      scripts = pkgs.lib.genAttrs scripts (n: p.${n} or "");
      contexts = pkgs.lib.genAttrs scripts (n: builtins.getContext (p.${n} or ""));
      flagContexts = builtins.map builtins.getContext p.cmakeFlags;
      CXXFLAGS = p.CXXFLAGS or "";
      meta = builtins.removeAttrs p.meta [ "position" ];
      testNames = builtins.attrNames p.tests;
    } // pkgs.lib.optionalAttrs passthruDetail { tests = pkgs.lib.mapAttrs (_: projectTest) (if testName == null then builtins.removeAttrs p.tests [ "mysql-autobackup" ] else { ${testName} = p.tests.${testName}; }); };
    inspect = side:
      let base = (if side == "upstream" then upstream else candidate).override override;
          p = if attrOverride == {} then base else base.overrideAttrs (_: attrOverride);
      in if probe == "arguments" then (if side == "upstream" then pkgs.lib.functionArgs upstream.override else builtins.functionArgs factory)
      else if probe == "family" then pkgs.lib.genAttrs [ "mariadb_105" "mariadb_106" "mariadb_1011" "mariadb_114" ] (n: (if side == "upstream" then import (nixpkgs + "/pkgs/servers/sql/mariadb") pkgs else family).${n}.drvPath)
      else { server = projection p; client = projection (base.client.overrideAttrs (_: clientAttrOverride)); serverMember = identity base.server; hasClientAfterOverride = p ? client; hasServerAfterOverride = p ? server; };
  in if project == "both" then { upstream = inspect "upstream"; candidate = inspect "candidate"; } else inspect project;
}
