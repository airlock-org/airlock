{
  description = "Nixome: Qubes control stack on NixOS Xen dom0";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    nixos-qubes = {
      url = "path:./vendor/nixos-qubes";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
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
        system:
        {
          configuration,
          imageName,
        }:
        let
          pkgs = pkgsFor system;
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
          inherit lib pkgs;
          config = configuration.config;
          name = "${imageName}-root-image";
          baseName = "root";
          format = "raw";
          partitionTableType = "none";
          installBootLoader = false;
          copyChannel = false;
          diskSize = "auto";
          additionalSpace = "1024M";
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
        system:
        {
          templateName,
          rootImageFile,
        }:
        let
          pkgs = pkgsFor system;
        in
        pkgs.callPackage ./pkgs/nixome-qubes-template {
          inherit templateName rootImageFile;
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
      overlays.nixomeImageBuildTools = nixomeImageBuildToolsOverlay;

      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
          configSystemRootImage = mkQubeRootImage system {
            configuration = self.nixosConfigurations.qube-template-config-system;
            imageName = "nixome-nixos-config-system";
          };
        in
        {
          nixome-config-vms-smoke = pkgs.callPackage ./pkgs/nixome-config-vms-smoke { };
          nixome-qube-bootstrap = pkgs.callPackage ./pkgs/nixome-qube-bootstrap { };
          nixome-qube-gui-smoke = pkgs.callPackage ./pkgs/nixome-qube-gui-smoke { };
          nixome-qubes-reconcile = pkgs.callPackage ./pkgs/nixome-qubes-reconcile { };
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
          nixome-nixos-config-system-root-image = configSystemRootImage;
          nixome-nixos-config-vault-template = mkQubesTemplatePackage system {
            templateName = "nixome-nixos-config-vault";
            rootImageFile = "${configSystemRootImage}/root.img";
          };
          nixome-nixos-config-mod-template = mkQubesTemplatePackage system {
            templateName = "nixome-nixos-config-mod";
            rootImageFile = "${configSystemRootImage}/root.img";
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
        module-contract = mkModuleContractCheck system;
        config-vms-smoke-package = self.packages.${system}.nixome-config-vms-smoke;
        qube-bootstrap-package = self.packages.${system}.nixome-qube-bootstrap;
        qube-gui-smoke-package = self.packages.${system}.nixome-qube-gui-smoke;
        qubes-reconcile-package = self.packages.${system}.nixome-qubes-reconcile;
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
          nixomeQubeKernelOpts = {
            configMod = mkQubeKernelOpts self.nixosConfigurations.qube-template-config-system;
            configVault = mkQubeKernelOpts self.nixosConfigurations.qube-template-config-system;
          };
          nixomeTemplatePackages = {
            configVault = self.packages.x86_64-linux.nixome-nixos-config-vault-template;
            configMod = self.packages.x86_64-linux.nixome-nixos-config-mod-template;
          };
        };
        modules = [
          self.nixosModules.dom0
          ./machines/vm-gui.nix
          ./machines/config-system.nix
        ];
      };

      nixosConfigurations.qube-template-config-system = lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          nixos-qubes.nixosModules.qubes
          (
            { lib, ... }:
            {
              nixpkgs.config.allowUnfreePredicate =
                pkg:
                builtins.elem (pkg.pname or (lib.getName pkg)) [
                  "qubes-artwork"
                  "qubes-vmm-stubdom-linux"
                ];
            }
          )
          ./machines/qube-template-config-system.nix
        ];
      };

      nixosConfigurations.qube-template-config-vault = lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          nixos-qubes.nixosModules.qubes
          (
            { lib, ... }:
            {
              nixpkgs.config.allowUnfreePredicate =
                pkg:
                builtins.elem (pkg.pname or (lib.getName pkg)) [
                  "qubes-artwork"
                  "qubes-vmm-stubdom-linux"
                ];
            }
          )
          ./machines/qube-template-config-vault.nix
        ];
      };

      nixosConfigurations.qube-template-config-mod = lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          nixos-qubes.nixosModules.qubes
          (
            { lib, ... }:
            {
              nixpkgs.config.allowUnfreePredicate =
                pkg:
                builtins.elem (pkg.pname or (lib.getName pkg)) [
                  "qubes-artwork"
                  "qubes-vmm-stubdom-linux"
                ];
            }
          )
          ./machines/qube-template-config-mod.nix
        ];
      };
    };
}
