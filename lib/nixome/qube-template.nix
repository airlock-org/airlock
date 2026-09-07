{
  nixpkgs,
  nixpkgsLibvirt105Source,
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

  libvirt105Overlay = self: _: {
    libvirt_10_5 = self.callPackage "${nixpkgsLibvirt105Source}/pkgs/development/libraries/libvirt" {
      substituteAll = args: self.replaceVars args.src (builtins.removeAttrs args [ "src" ]);
      xhtml1 = null;
      AppKit = null;
      Carbon = null;
    };
  };

  qubesOverlays = [
    imageBuildToolsOverlay
    (import "${nixosQubesSource}/pkgs/top-level/overlay.nix")
    (import "${nixpkgs}/pkgs/top-level/by-name-overlay.nix" "${nixosQubesSource}/pkgs/by-name")
    libvirt105Overlay
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
          nixomePrebuiltTemplateSources = { };
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

  mkNixStoreClosureImage =
    {
      name,
      rootPaths,
      additionalSpaceMiB ? 256,
    }:
    let
      closure = pkgs.closureInfo {
        inherit rootPaths;
      };
    in
    pkgs.runCommand "${name}-store-closure-image"
      {
        nativeBuildInputs = [
          pkgs.coreutils
          pkgs.e2fsprogs
          pkgs.gawk
        ];
        preferLocalBuild = true;
        allowSubstitutes = false;
      }
      ''
        set -eu

        root="$PWD/root"
        mkdir -p "$root/nix/store" "$out"

        while IFS= read -r store_path; do
          mkdir -p "$root$(dirname "$store_path")"
          cp -a "$store_path" "$root$store_path"
        done < ${closure}/store-paths

        used_bytes="$(du -sb "$root" | awk '{ print $1 }')"
        image_bytes="$((used_bytes + (${toString additionalSpaceMiB} * 1024 * 1024)))"
        truncate -s "$image_bytes" "$out/store.img"
        mkfs.ext4 -q -F -L nixstore -d "$root" "$out/store.img"

        cp ${closure}/store-paths "$out/store-paths"
        cat > "$out/manifest.json" <<EOF
        {"schema":"nixome.store-closure-image.v1","name":"${name}","image":"store.img","mountpoint":"/nix/store","read_only":true}
        EOF
      '';

  mkNixNativeDomain =
    {
      name,
      modules,
      additionalRootSpace ? "1024M",
      additionalStoreSpaceMiB ? 256,
      configurationName ? "nixome-domain-${builtins.replaceStrings [ "." ] [ "-" ] name}",
      rootImageName ? "${configurationName}-root-image",
      storeImageName ? "${configurationName}-store-image",
      kernelName ? "6.12.64-1.fc41",
      label ? "green",
      netvm ? null,
      prefs ? { },
      features ? { },
      services ? { },
    }:
    let
      configuration = lib.nixosSystem {
        inherit system;
        specialArgs = {
          nixomeConfigSeedInputs = null;
          nixomePrebuiltTemplateSources = { };
        };
        modules = [
          "${nixpkgs}/nixos/modules/profiles/minimal.nix"
          "${nixosQubesSource}/nixos/modules/virtualisation/qubes"
          "${nixomeSupport}/modules/qube-guest.nix"
          {
            nixpkgs.pkgs = pkgs;
            nixome.qubeGuest.enable = true;
            nixome.qubeGuest.role = "base";
            documentation.enable = lib.mkDefault false;
            programs.command-not-found.enable = lib.mkDefault false;
            system.nixos.versionSuffix = lib.mkForce "";
          }
        ]
        ++ modules;
      };
      rootImage = mkQubeRootImage {
        configuration = configuration;
        imageName = rootImageName;
        additionalSpace = additionalRootSpace;
      };
      storeImage = mkNixStoreClosureImage {
        name = storeImageName;
        rootPaths = [ configuration.config.system.build.toplevel ];
        additionalSpaceMiB = additionalStoreSpaceMiB;
      };
      defaultPrefs = {
        autostart = false;
        default_user = "user";
        include_in_backups = true;
        kernel = kernelName;
        kernelopts = mkQubeKernelOpts configuration;
        memory = 1024;
        maxmem = 2048;
        qrexec_timeout = 120;
        vcpus = 1;
        virt_mode = "pvh";
      };
      defaultFeatures = {
        "nixome-backend" = "nix-closure";
        "nixome-managed" = "1";
        "nixome-role" = "nix-native";
        os = "Linux";
        qrexec = "1";
      };
      domain = {
        inherit label netvm services;
        class = "StandaloneVM";
        template = null;
        prefs = defaultPrefs // prefs;
        features = defaultFeatures // features;
        backend = {
          type = "nix-closure";
          system.sourceRef.build = "nixosConfigurations.${configurationName}.config.system.build.toplevel";
          kernel.name = kernelName;
          root = {
            mode = "qubes-root-image";
            sourceRef = {
              build = "packages.${system}.${rootImageName}";
              subpath = "root.img";
            };
          };
          store = {
            mode = "closure-image";
            sourceRef = {
              build = "packages.${system}.${storeImageName}";
              subpath = "store.img";
            };
            mountpoint = "/nix/store";
            readOnly = true;
          };
        };
      };
    in
    {
      inherit
        configuration
        domain
        rootImage
        rootImageName
        storeImage
        storeImageName
        ;
      kernelopts = mkQubeKernelOpts configuration;
      configurationName = configurationName;
    };
in
{
  inherit
    mkNixNativeDomain
    mkNixStoreClosureImage
    mkNixosQubeTemplate
    mkQubeKernelOpts
    mkQubeRootImage
    mkQubesTemplatePackage
    pkgs
    qubesOverlays
    ;
}
