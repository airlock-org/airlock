{
  nixpkgs,
  nixosQubesSource,
  nixomeSupport,
  system ? "x86_64-linux",
}:
let
  lib = nixpkgs.lib;

  allowUnfreePredicate =
    pkg:
    builtins.elem (pkg.pname or (lib.getName pkg)) [
      "qubes-artwork"
      "qubes-vmm-stubdom-linux"
    ];

  imageBuildToolsOverlay = _final: prev: {
    lkl = prev.lkl.overrideAttrs (oldAttrs: {
      postPatch = (oldAttrs.postPatch or "") + ''
        substituteInPlace tools/lkl/cptofs.c \
          --replace-fail 'lkl_start_kernel("mem=100M")' 'lkl_start_kernel("mem=1024M")'
      '';
    });
  };

  qubesOverlays = [
    imageBuildToolsOverlay
    (import "${nixosQubesSource}/pkgs/top-level/overlay.nix")
    (import "${nixpkgs}/pkgs/top-level/by-name-overlay.nix" "${nixosQubesSource}/pkgs/by-name")
    (import "${nixosQubesSource}/nix/overlay.nix")
  ];

  pkgs = import nixpkgs {
    inherit system;
    overlays = qubesOverlays;
    config.allowUnfreePredicate = allowUnfreePredicate;
  };

  mkQubeKernelOpts =
    configuration:
    let
      cfg = configuration.config;
      kernelParams = builtins.filter (param: !(lib.hasPrefix "root=" param)) cfg.boot.kernelParams;
    in
    lib.concatStringsSep " " (
      kernelParams
      ++ [
        "init=/sbin/init"
      ]
    );

  mkQubeRootImage =
    {
      configuration,
      imageName,
      additionalSpace ? "1024M",
    }:
    let
      bootAnchors = pkgs.runCommand "${imageName}-qubes-boot-anchors" { } ''
        mkdir -p "$out/sbin" "$out/lib/modules"
        cat > "$out/sbin/init" <<'EOF'
        #!${pkgs.runtimeShell}
        set -eu

        export PATH=${lib.makeBinPath [ pkgs.coreutils ]}

        mkdir -p /etc /run/qubes /var/log/qubes
        if [ -L /var/run ] || [ ! -e /var/run ]; then
          mkdir -p /var
          ln -sfn /run /var/run
        elif [ -d /var/run ]; then
          mkdir -p /var/run/qubes
        else
          echo "cannot initialize /var/run for Qubes guest runtime" >&2
          exit 1
        fi

        ${configuration.config.system.build.toplevel}/activate
        exec ${configuration.config.system.build.toplevel}/init "$@"
        EOF
        chmod 0555 "$out/sbin/init"
      '';
    in
    import "${nixpkgs}/nixos/lib/make-disk-image.nix" {
      inherit
        additionalSpace
        lib
        pkgs
        ;
      config = configuration.config;
      name = "${imageName}-root-image";
      baseName = "root";
      format = "raw";
      partitionTableType = "none";
      installBootLoader = false;
      copyChannel = false;
      diskSize = "auto";
      contents = [
        {
          source = "${bootAnchors}/sbin/init";
          target = "/sbin/init";
        }
        {
          source = "${bootAnchors}/lib";
          target = "/lib";
        }
      ];
    };

  mkQubesTemplatePackage =
    {
      templateName,
      rootImageFile,
      templateConf ? {
        qrexec = "1";
        virt-mode = "pvh";
      },
      vmAppmenus ? [ ],
      templateAppmenus ? [ ],
      netvmAppmenus ? [ ],
    }:
    pkgs.callPackage "${nixomeSupport}/pkgs/nixome-qubes-template" {
      inherit
        netvmAppmenus
        rootImageFile
        templateAppmenus
        templateConf
        templateName
        vmAppmenus
        ;
    };

  mkNixosQubeTemplate =
    {
      templateName,
      imageName ? templateName,
      modules,
      additionalSpace ? "1024M",
      templateConf ? {
        qrexec = "1";
        virt-mode = "pvh";
      },
      vmAppmenus ? [ ],
      templateAppmenus ? [ ],
      netvmAppmenus ? [ ],
    }:
    let
      configuration = lib.nixosSystem {
        inherit system;
        specialArgs = {
          nixomeConfigSeedInputs = null;
        };
        modules = [
          "${nixpkgs}/nixos/modules/profiles/minimal.nix"
          "${nixosQubesSource}/nixos/modules/virtualisation/qubes"
          "${nixomeSupport}/modules/qube-guest.nix"
          {
            nixpkgs.pkgs = pkgs;
            nixome.qubeGuest.enable = true;
            documentation.enable = lib.mkDefault false;
            programs.command-not-found.enable = lib.mkDefault false;
            system.nixos.versionSuffix = lib.mkForce "";
          }
        ]
        ++ modules;
      };
      rootImage = mkQubeRootImage {
        inherit additionalSpace configuration imageName;
      };
      package = mkQubesTemplatePackage {
        inherit
          netvmAppmenus
          templateAppmenus
          templateConf
          templateName
          vmAppmenus
          ;
        rootImageFile = "${rootImage}/root.img";
      };
    in
    {
      inherit
        configuration
        package
        rootImage
        ;
      kernelopts = mkQubeKernelOpts configuration;
    };
in
{
  inherit
    mkNixosQubeTemplate
    mkQubeKernelOpts
    mkQubeRootImage
    mkQubesTemplatePackage
    pkgs
    qubesOverlays
    ;
}
