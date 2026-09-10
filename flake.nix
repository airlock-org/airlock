{
  description = "Nixome: Qubes control stack on NixOS Xen dom0";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-libvirt_10_5.url = "github:NixOS/nixpkgs/e0464e47880a69896f0fb1810f00e0de469f770a";

    nixos-qubes = {
      url = "path:./vendor/nixos-qubes";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.nixpkgs-libvirt_10_5.follows = "nixpkgs-libvirt_10_5";
    };
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      nixpkgs-libvirt_10_5,
      nixos-qubes,
      ...
    }:
    let
      inherit (nixpkgs) lib;

      supportedSystems = [ "x86_64-linux" ];

      forAllSystems = lib.genAttrs supportedSystems;

      nixomeImageBuildToolsOverlay = _final: prev: {
        lkl = prev.lkl.overrideAttrs (oldAttrs: {
          postPatch = (oldAttrs.postPatch or "") + ''
            substituteInPlace tools/lkl/cptofs.c \
              --replace-fail 'lkl_start_kernel("mem=100M")' 'lkl_start_kernel("mem=1024M")'
          '';
        });
      };

      pkgsFor =
        system:
        import nixpkgs {
          inherit system;
          overlays = [
            nixomeImageBuildToolsOverlay
            nixos-qubes.overlays.default
          ];
          config.allowUnfreePredicate =
            pkg:
            builtins.elem (pkg.pname or (lib.getName pkg)) [
              "qubes-artwork"
              "qubes-vmm-stubdom-linux"
            ];
        };

      boolString = value: if value then "true" else "false";

      nixomeQubesVmKernelName = "6.12.64-1.fc41";

      nixomeSupportSeedSource =
        let
          root = toString ./.;
          keep =
            path: _type:
            let
              rel = lib.removePrefix "${root}/" (toString path);
              keepTree = name: rel == name || lib.hasPrefix "${name}/" rel;
            in
            rel == "flake.nix"
            || rel == "flake.lock"
            || keepTree "lib"
            || keepTree "machines"
            || keepTree "modules"
            || keepTree "pkgs"
            || keepTree "scripts"
            || keepTree "seeds"
            || rel == "tests"
            || rel == "tests/qubes-gui-smoke.sh"
            || rel == "tests/qubes-smoke.sh";
        in
        lib.cleanSourceWith {
          name = "nixome-support-seed-source";
          src = ./.;
          filter = keep;
        };

      nixomeConfigSeedInputs = {
        nixpkgs = "${nixpkgs}";
        nixpkgsLibvirt105 = "${nixpkgs-libvirt_10_5}";
        nixosQubes = "${nixos-qubes}";
        nixomeSupport = "${nixomeSupportSeedSource}";
      };

      mkNixomeQubeBuilders =
        system:
        import ./lib/nixome/qube-template.nix {
          inherit nixpkgs system;
          nixpkgsLibvirt105Source = "${nixpkgs-libvirt_10_5}";
          nixomeSupport = "${self}";
          nixosQubesSource = "${nixos-qubes}";
        };

      configVmRoles = [
        "config-vault"
        "config-staging"
        "config-validator"
      ];

      configNixPrivateSizeBytes = 8589934592;
      devPrivateSizeBytes = 34359738368;
      sysNetPrivateSizeBytes = 2147483648;

      configRoleDefaults = {
        config-vault = {
          guestHostName = "config-vault";
          label = "black";
          rootImageSizeMiB = 128;
          prefs = {
            autostart = false;
            include_in_backups = true;
            memory = 1024;
            maxmem = 2048;
            vcpus = 1;
          };
          features = {
            "nixome-managed" = "1";
            "nixome-role" = "config-vault";
            "nixome-config-authority" = "1";
          };
          services = {
            "network-manager" = false;
          };
        };
        config-staging = {
          guestHostName = "config-staging";
          label = "gray";
          rootImageSizeMiB = 128;
          state.persistentSize = configNixPrivateSizeBytes;
          prefs = {
            autostart = false;
            include_in_backups = true;
            memory = 2048;
            maxmem = 4096;
            vcpus = 2;
          };
          features = {
            "nixome-managed" = "1";
            "nixome-role" = "config-staging";
            "nixome-config-staging" = "1";
            "nixome-config-applied-message" = "initial";
          };
          services = {
            "network-manager" = false;
          };
        };
        config-validator = {
          guestHostName = "config-validator";
          label = "blue";
          rootImageSizeMiB = 128;
          state.persistentSize = configNixPrivateSizeBytes;
          prefs = {
            autostart = false;
            include_in_backups = true;
            memory = 2048;
            maxmem = 4096;
            vcpus = 2;
          };
          features = {
            "nixome-managed" = "1";
            "nixome-role" = "config-validator";
            "nixome-config-validator" = "1";
          };
          services = {
            "network-manager" = false;
          };
        };
      };

      devRoleDefaults = {
        guestHostName = "dev";
        label = "green";
        rootImageSizeMiB = 512;
        state.persistentSize = devPrivateSizeBytes;
        prefs = {
          autostart = false;
          include_in_backups = true;
          memory = 4096;
          maxmem = 8192;
          vcpus = 4;
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "dev";
          "nixome-dev" = "1";
        };
      };

      sysNetRoleDefaults = {
        guestHostName = "sys-net";
        label = "red";
        rootImageSizeMiB = 512;
        netvm = null;
        state.persistentSize = sysNetPrivateSizeBytes;
        prefs = {
          autostart = false;
          include_in_backups = true;
          memory = 768;
          maxmem = 1024;
          provides_network = true;
          vcpus = 2;
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "sys-net";
          "nixome-network-provider" = "sys-net";
        };
        services = {
          "network-manager" = true;
          "qubes-firewall" = true;
          "qubes-network" = true;
        };
        devices.pci = [ ];
      };

      mkConfigDomainEntries =
        system:
        let
          qubeBuilders = mkNixomeQubeBuilders system;
          mkGuestModule =
            role:
            let
              defaults = configRoleDefaults.${role};
            in
            { ... }:
            {
              networking.hostName = defaults.guestHostName;
            };
          mkEntry =
            role:
            let
              defaults = configRoleDefaults.${role};
            in
            qubeBuilders.mkNixNativeDomain {
              name = role;
              guestRole = role;
              inherit nixomeConfigSeedInputs;
              nixomePrebuiltTemplateSources = { };
              modules = [ (mkGuestModule role) ];
              rootImageSizeMiB = defaults.rootImageSizeMiB;
              kernelName = nixomeQubesVmKernelName;
              label = defaults.label;
              netvm = null;
              prefs = defaults.prefs;
              features = defaults.features;
              services = defaults.services;
              state = defaults.state or { };
            };
        in
        lib.listToAttrs (
          builtins.map (role: {
            name = role;
            value = mkEntry role;
          }) configVmRoles
        );

      mkDevDomainEntry =
        system:
        let
          qubeBuilders = mkNixomeQubeBuilders system;
        in
        qubeBuilders.mkNixNativeDomain {
          name = "dev";
          guestRole = "dev";
          modules = [
            ({ ... }: {
              networking.hostName = devRoleDefaults.guestHostName;
            })
          ];
          rootImageSizeMiB = devRoleDefaults.rootImageSizeMiB;
          kernelName = nixomeQubesVmKernelName;
          label = devRoleDefaults.label;
          netvm = "sys-net";
          prefs = devRoleDefaults.prefs;
          features = devRoleDefaults.features;
          services = { };
          state = devRoleDefaults.state;
        };

      mkSysNetDomainEntry =
        system:
        let
          qubeBuilders = mkNixomeQubeBuilders system;
        in
        qubeBuilders.mkNixNativeDomain {
          name = "sys-net";
          guestRole = "sys-net";
          modules = [
            ({ ... }: {
              networking.hostName = sysNetRoleDefaults.guestHostName;
            })
          ];
          rootImageSizeMiB = sysNetRoleDefaults.rootImageSizeMiB;
          kernelName = nixomeQubesVmKernelName;
          label = sysNetRoleDefaults.label;
          netvm = sysNetRoleDefaults.netvm;
          prefs = sysNetRoleDefaults.prefs;
          features = sysNetRoleDefaults.features;
          services = sysNetRoleDefaults.services;
          devices = sysNetRoleDefaults.devices;
          state = sysNetRoleDefaults.state;
        };

      materializeNixClosureDomain =
        entry:
        entry.domain
        // {
          prefs = entry.domain.prefs // {
            kernelopts = entry.kernelopts;
          };
          backend = entry.domain.backend // {
            system = entry.domain.backend.system // {
              source = "${entry.configuration.config.system.build.toplevel}";
              sourceRef = null;
            };
            root = entry.domain.backend.root // {
              source = "${entry.rootImage}/root.img";
              sourceRef = null;
            };
            store = entry.domain.backend.store // {
              source = "${entry.storeArtifact}";
              sourceRef = null;
            };
          };
        };

      mkModuleContractCheck =
        system:
        let
          pkgs = pkgsFor system;
          cfg = self.nixosConfigurations.test.config;
          optOut = cfg.virtualisation.qubes.optOutRecommendedConfiguration;
          hasOptOut = name: builtins.elem name optOut;
        in
        pkgs.runCommand "nixome-module-contract"
          {
            preferLocalBuild = true;
            allowSubstitutes = false;

            nixomeDom0Enabled = boolString cfg.nixome.dom0.enable;
            qubesDom0Enabled = boolString cfg.virtualisation.qubes.dom0.enable;
            qubesSecure = boolString cfg.virtualisation.qubes.secure;
            xenEnabled = boolString cfg.virtualisation.xen.enable;
            initrdSystemdEnabled = boolString cfg.boot.initrd.systemd.enable;
            qubesdEnabled = boolString cfg.systemd.services.qubesd.enable;
            qubesDbDom0Enabled = boolString cfg.systemd.services.qubes-db-dom0.enable;
            qrexecPolicyEnabled = boolString cfg.systemd.services.qubes-qrexec-policy-daemon.enable;
            dedicatedSysNetOptOut = boolString (hasOptOut "dedicated-sys-net");
            dedicatedSysUsbOptOut = boolString (hasOptOut "dedicated-sys-usb");
            dom0GuiOptOut = boolString (hasOptOut "dom0-is-gui");
            xenPackage = cfg.virtualisation.xen.package.pname or (lib.getName cfg.virtualisation.xen.package);
            qemuPackage =
              cfg.virtualisation.xen.qemu.package.pname or (lib.getName cfg.virtualisation.xen.qemu.package);
            libvirtPackage =
              cfg.virtualisation.libvirtd.package.pname or (lib.getName cfg.virtualisation.libvirtd.package);
            libvirtFirewallBackend = cfg.virtualisation.libvirtd.firewallBackend;
          }
          ''
            set -eu

            check() {
              name="$1"
              expected="$2"
              actual="$3"

              if [ "$actual" != "$expected" ]; then
                printf 'contract check failed: %s expected %s, got %s\n' "$name" "$expected" "$actual" >&2
                exit 1
              fi
            }

            check nixome-dom0-enabled true "$nixomeDom0Enabled"
            check qubes-dom0-enabled true "$qubesDom0Enabled"
            check qubes-secure false "$qubesSecure"
            check xen-enabled true "$xenEnabled"
            check initrd-systemd-enabled true "$initrdSystemdEnabled"
            check qubesd-enabled true "$qubesdEnabled"
            check qubes-db-dom0-enabled true "$qubesDbDom0Enabled"
            check qrexec-policy-enabled true "$qrexecPolicyEnabled"
            check dedicated-sys-net-opt-out true "$dedicatedSysNetOptOut"
            check dedicated-sys-usb-opt-out true "$dedicatedSysUsbOptOut"
            check dom0-gui-opt-out true "$dom0GuiOptOut"
            check xen-package qubes-vmm-xen "$xenPackage"
            check qemu-package qemu-xen "$qemuPackage"
            check libvirt-package qubes-core-libvirt "$libvirtPackage"
            check libvirt-firewall-backend iptables "$libvirtFirewallBackend"

            printf 'nixome module contract ok\n'
            touch "$out"
          '';
    in
    {
      lib = {
        nixomeSystem = import ./lib/nixome/system.nix;
        nixomeTopology = import ./lib/nixome/topology.nix;
        nixomeQubeTemplate = import ./lib/nixome/qube-template.nix;
      };

      nixome.seedInputs = nixomeConfigSeedInputs;

      overlays.nixomeImageBuildTools = nixomeImageBuildToolsOverlay;

      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
          templateBuilders = mkNixomeQubeBuilders system;
          configDomainEntries = mkConfigDomainEntries system;
          configDomainPackages = lib.listToAttrs (
            lib.concatMap (
              name:
              let
                entry = configDomainEntries.${name};
              in
              [
                {
                  name = entry.rootImageName;
                  value = entry.rootImage;
                }
                {
                  name = entry.storeArtifactName;
                  value = entry.storeArtifact;
                }
              ]
            ) (builtins.attrNames configDomainEntries)
          );
        in
        configDomainPackages
        // {
          nixome-config-apply = pkgs.callPackage ./pkgs/nixome-config-apply { };
          nixome-config-archive-candidate = pkgs.callPackage ./pkgs/nixome-config-archive { };
          nixome-config-boot-verify = pkgs.callPackage ./pkgs/nixome-config-boot-verify { };
          nixome-config-control = pkgs.callPackage ./pkgs/nixome-config-control { };
          nixome-config-seed-inputs-import = pkgs.callPackage ./pkgs/nixome-config-seed-inputs-import { };
          nixome-config-status = pkgs.callPackage ./pkgs/nixome-config-status { };
          nixome-config-vms-smoke = pkgs.callPackage ./pkgs/nixome-config-vms-smoke { };
          nixome-cli = pkgs.callPackage ./pkgs/nixome-cli { };
          nixome = pkgs.callPackage ./pkgs/nixome-cli { };
          codex = pkgs.codex;
          zellij = pkgs.zellij;
          nixome-nix-domain-storage = pkgs.callPackage ./pkgs/nixome-nix-domain-storage { };
          nixome-nix-closure-domain-smoke = pkgs.callPackage ./pkgs/nixome-nix-closure-domain-smoke { };
          nixome-qubes-storage = pkgs.python3.pkgs.callPackage ./pkgs/nixome-qubes-storage { };
          nixome-qube-bootstrap = pkgs.callPackage ./pkgs/nixome-qube-bootstrap { };
          nixome-qube-gui-smoke = pkgs.callPackage ./pkgs/nixome-qube-gui-smoke { };
          nixome-setup-config = pkgs.callPackage ./pkgs/nixome-setup-config { };
          nixome-qubes-smoke = pkgs.callPackage ./pkgs/nixome-qubes-smoke { };
          nixome-observe-dom0 = pkgs.callPackage ./pkgs/nixome-observe { };
          nixome-qubes-vm-kernel = pkgs.repackRpmKernel {
            kernelSrc = pkgs.fetchurl {
              url = "https://ftp.qubes-os.org/repo/yum/r4.3/current/dom0/fc41/rpm/kernel-qubes-vm-6.12.64-1.qubes.fc41.x86_64.rpm";
              hash = "sha256-nK0KbnT8hUwIZPhUgW8nSRNAbZDgjexHWfeeYDiFxsU=";
            };
            modulesSrc = pkgs.fetchurl {
              url = "https://ftp.qubes-os.org/repo/yum/r4.3/current/dom0/fc41/rpm/kernel-modules-6.12.64-1.qubes.fc41.x86_64.rpm";
              hash = "sha256-JBPQM27u/nQwKnfGGDLpQ35BhRwSpy+vcZLUlI06ELo=";
            };
            kernelVersion = "6.12.64-1";
            fedoraVersion = "fc41";
          };
          nixome-nix-closure-root-image-test = templateBuilders.mkNixClosureRootImage {
            imageName = "nixome-nix-closure-root-image-test";
            rootImageSizeMiB = 64;
          };
          nixome-nix-closure-store-manifest-test = templateBuilders.mkNixStoreClosureManifest {
            name = "nixome-nix-closure-store-manifest-test";
            rootPaths = [
              (pkgs.writeText "nixome-store-manifest-test-root" "nixome-store-manifest-test\n")
            ];
          };
          nixome-vm-image = self.nixosConfigurations.vm.config.system.build.image;
          nixome-vm-gui-image = self.nixosConfigurations.vm-gui.config.system.build.image;
          nixome-vm-gui-config-image = self.nixosConfigurations.vm-gui-config.config.system.build.image;
          ovmf-fd = pkgs.OVMF.fd;
          qemu-kvm = pkgs.qemu_kvm;
          qemu-utils = pkgs.qemu-utils;
          default = self.packages.${system}.nixome-observe-dom0;
        }
      );

      checks = forAllSystems (system: {
        config-apply-package = self.packages.${system}.nixome-config-apply;
        config-archive-package = self.packages.${system}.nixome-config-archive-candidate;
        config-boot-verify-package = self.packages.${system}.nixome-config-boot-verify;
        config-control-package = self.packages.${system}.nixome-config-control;
        codex-package = self.packages.${system}.codex;
        zellij-package = self.packages.${system}.zellij;
        config-seed-inputs-import-package = self.packages.${system}.nixome-config-seed-inputs-import;
        config-status-package = self.packages.${system}.nixome-config-status;
        nixome-cli-package = self.packages.${system}.nixome-cli;
        module-contract = mkModuleContractCheck system;
        config-vms-smoke-package = self.packages.${system}.nixome-config-vms-smoke;
        nix-closure-domain-smoke-package = self.packages.${system}.nixome-nix-closure-domain-smoke;
        qube-bootstrap-package = self.packages.${system}.nixome-qube-bootstrap;
        qube-gui-smoke-package = self.packages.${system}.nixome-qube-gui-smoke;
        setup-config-package = self.packages.${system}.nixome-setup-config;
        observe-package = self.packages.${system}.nixome-observe-dom0;
      });

      formatter = forAllSystems (system: (pkgsFor system).nixfmt);

      nixosModules.dom0 =
        { ... }:
        {
          nixpkgs.overlays = [
            self.overlays.nixomeImageBuildTools
          ];

          imports = [
            nixos-qubes.nixosModules.qubes
            ./modules/dom0.nix
            ./modules/qubes-topology.nix
          ];
        };

      nixosModules.default = self.nixosModules.dom0;

      nixosConfigurations.test = lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          self.nixosModules.dom0
          ./machines/test.nix
        ];
      };

      nixosConfigurations.vm = lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          self.nixosModules.dom0
          ./machines/vm.nix
        ];
      };

      nixosConfigurations.vm-gui = lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          self.nixosModules.dom0
          ./machines/vm-gui.nix
        ];
      };

      nixosConfigurations.vm-gui-config = lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = {
          nixomeQubeKernel = {
            name = nixomeQubesVmKernelName;
            package = self.packages.x86_64-linux.nixome-qubes-vm-kernel;
          };
          nixomeConfigDomains =
            let
              bootstrapDomainEntries = (mkConfigDomainEntries "x86_64-linux") // {
                sys-net = mkSysNetDomainEntry "x86_64-linux";
                dev = mkDevDomainEntry "x86_64-linux";
              };
            in
            builtins.mapAttrs (_name: entry: materializeNixClosureDomain entry) bootstrapDomainEntries;
          inherit nixomeConfigSeedInputs;
        };
        modules = [
          self.nixosModules.dom0
          ./machines/vm-gui.nix
          ./machines/config-system.nix
        ];
      };

      nixosConfigurations.nixome-domain-config-vault =
        (mkConfigDomainEntries "x86_64-linux").config-vault.configuration;
      nixosConfigurations.nixome-domain-config-staging =
        (mkConfigDomainEntries "x86_64-linux").config-staging.configuration;
      nixosConfigurations.nixome-domain-config-validator =
        (mkConfigDomainEntries "x86_64-linux").config-validator.configuration;
      nixosConfigurations.nixome-domain-sys-net = (mkSysNetDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.nixome-domain-dev = (mkDevDomainEntry "x86_64-linux").configuration;
    };
}
