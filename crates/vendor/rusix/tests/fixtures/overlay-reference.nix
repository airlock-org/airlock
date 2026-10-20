# Ordinary nixpkgs overlay; the semantic reference, never a package rewrite.
{
  overlay = final: prev: {
    curl = prev.curl.overrideAttrs (old: {
      configureFlags = old.configureFlags ++ [ "--disable-dict" ];
    });
  };

  inspect = pkgs:
    let
      package = pkgs.curl;
      recipe = p: builtins.readFile (builtins.unsafeDiscardStringContext p.drvPath);
    in {
      inherit (package) configureFlags;
      derivationPath = package.drvPath;
      outputPath = package.outPath;
      outputPaths = pkgs.lib.genAttrs package.outputs (output: package.${output}.outPath);
      configureFlagsEnv = builtins.toString package.configureFlags;
      recipe = recipe package;
      source = package.src.drvPath;
      sourceRecipe = recipe package.src;
      downstream = {
        derivationPath = pkgs.curlpp.drvPath;
        recipe = recipe pkgs.curlpp;
        curl = (builtins.head pkgs.curlpp.buildInputs).drvPath;
      };
      # An unrelated ordinary nixpkgs package remains available.
      hello = pkgs.hello.drvPath;
    };

  reference = { nixpkgs, overlay ? null }:
    import nixpkgs {
      system = "x86_64-linux";
      config = {};
      overlays = if overlay == null then [] else [ overlay ];
    };
}
