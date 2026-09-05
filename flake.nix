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

      pkgsFor =
        system:
        import nixpkgs {
          inherit system;
          config.allowUnfreePredicate =
            pkg:
            builtins.elem (pkg.pname or (lib.getName pkg)) [
              "qubes-artwork"
              "qubes-vmm-stubdom-linux"
            ];
        };

      boolString = value: if value then "true" else "false";

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
      overlays.nixomeImageBuildTools = _final: prev: {
        lkl = prev.lkl.overrideAttrs (oldAttrs: {
          postPatch = (oldAttrs.postPatch or "") + ''
            substituteInPlace tools/lkl/cptofs.c \
              --replace-fail 'lkl_start_kernel("mem=100M")' 'lkl_start_kernel("mem=1024M")'
          '';
        });
      };

      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          nixome-qubes-smoke = pkgs.callPackage ./pkgs/nixome-qubes-smoke { };
          nixome-observe-dom0 = pkgs.callPackage ./pkgs/nixome-observe { };
          nixome-vm-image = self.nixosConfigurations.vm.config.system.build.image;
          nixome-vm-gui-image = self.nixosConfigurations.vm-gui.config.system.build.image;
          ovmf-fd = pkgs.OVMF.fd;
          qemu-kvm = pkgs.qemu_kvm;
          qemu-utils = pkgs.qemu-utils;
          default = self.packages.${system}.nixome-observe-dom0;
        }
      );

      checks = forAllSystems (system: {
        module-contract = mkModuleContractCheck system;
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
    };
}
