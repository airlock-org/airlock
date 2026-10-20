# Git factories use the same callPackage scope, including dependency splicing.
{
  compare =
    { factory, nixpkgs, features ? {}, localSystem ? "x86_64-linux", crossSystem ? null
    , override ? {}, attrOverride ? {}, project ? "both", hostFlags ? {}
    }:
    let
      pkgs = import nixpkgs {
        inherit localSystem crossSystem;
        config.allowUnsupportedSystem = true;
      };
      wiring = {
        inherit (pkgs.darwin.apple_sdk.frameworks) CoreServices Security;
        perlLibs = with pkgs.perlPackages; [ LWP URI TermReadKey ];
        smtpPerlLibs = with pkgs.perlPackages; [
          libnet NetSMTPSSL IOSocketSSL NetSSLeay AuthenSASL DigestHMAC
        ];
      } // pkgs.lib.optionalAttrs (hostFlags != {}) {
        # An explicit branch probe, not a claim that nixpkgs supports this target.
        stdenv = pkgs.stdenv // { hostPlatform = pkgs.stdenv.hostPlatform // hostFlags; };
      } // features;
      instantiate = implementation:
        let base = pkgs.callPackage implementation wiring;
        in (base.override override).overrideAttrs (_: attrOverride);
      upstream = instantiate (nixpkgs + "/pkgs/applications/version-management/git");
      candidate = instantiate factory;
      identity = package: if builtins.isPath package then { path = builtins.toString package; } else {
        inherit (package) name outputName;
        derivationPath = package.drvPath;
        outputPath = package.outPath;
      };
      # The exact ATerm recipe and output paths are hash-relevant, not snapshots of source.
      projection = package: {
        inherit (package) name pname version outputs;
        recipe = builtins.readFile (builtins.unsafeDiscardStringContext package.drvPath);
        derivationPath = package.drvPath;
        outputPath = package.outPath;
        dependencies = builtins.map identity (package.nativeBuildInputs ++ package.buildInputs);
        patches = builtins.map (p: "${p}") package.patches;
        patchContexts = builtins.map (p: builtins.getContext "${p}") package.patches;
        source = identity package.src;
        sourceRecipe = builtins.readFile (builtins.unsafeDiscardStringContext package.src.drvPath);
        passthru = {
          inherit (package) shellPath;
          updateScript = "${package.updateScript}";
          updateScriptContents = builtins.readFile package.updateScript;
          testNames = builtins.attrNames package.tests;
          # Forces the finalAttrs.finalPackage callback and checks overrideAttrs compatibility.
          installedTest = identity package.tests.withInstallCheck;
          externalTests = pkgs.lib.mapAttrs (_: test: {
            category = builtins.typeOf test;
            names = if builtins.isAttrs test then builtins.attrNames test else [];
          }) (builtins.removeAttrs package.tests [ "withInstallCheck" ]);
          updateScriptContext = builtins.getContext "${package.updateScript}";
          installedTestRecipe = builtins.readFile (builtins.unsafeDiscardStringContext package.tests.withInstallCheck.drvPath);
        };
        meta = {
          inherit (package.meta) homepage description license changelog longDescription platforms maintainers mainProgram;
        };
        scripts = {
          inherit (package) postPatch preBuild postBuild preInstall postInstall preInstallCheck;
        };
        scriptContexts = pkgs.lib.genAttrs [ "postPatch" "preBuild" "postBuild" "preInstall" "postInstall" "preInstallCheck" ]
          (name: builtins.getContext package.${name});
        flags = {
          inherit (package) configureFlags makeFlags installFlags installCheckFlags
            installCheckTarget doInstallCheck doCheck
            disallowedReferences stripDebugList NIX_LDFLAGS;
          nativeInstallCheckInputs = package.nativeInstallCheckInputs or [];
        };
      };
    in
      if project == "upstream" then projection upstream
      else if project == "candidate" then projection candidate
      else {
        upstream = projection upstream // { arguments = builtins.functionArgs (import (nixpkgs + "/pkgs/applications/version-management/git")); };
        candidate = projection candidate // { arguments = builtins.functionArgs factory; };
      };
}
