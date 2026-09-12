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

      nixomeImageBuildToolsOverlay =
        _final: prev:
        {
          lkl = prev.lkl.overrideAttrs (oldAttrs: {
            postPatch = (oldAttrs.postPatch or "") + ''
              substituteInPlace tools/lkl/cptofs.c \
                --replace-fail 'lkl_start_kernel("mem=100M")' 'lkl_start_kernel("mem=1024M")'
            '';
          });
        }
        // lib.optionalAttrs (prev ? qubes-manager) {
          qubes-manager = prev.qubes-manager.overrideAttrs (oldAttrs: {
            postInstall = (oldAttrs.postInstall or "") + ''
              # Nixome templates are built from Nix config. Do not expose the
              # upstream Fedora/Debian template installer in dom0 until that
              # RPC/repository path is intentionally supported.
              qubes_manager_py="$out/${prev.python3.sitePackages}/qubesmanager"

              rm -f \
                "$out/bin/qvm-template-gui" \
                "$out/bin/qubes-template-manager" \
                "$out/share/applications/qubes-template-manager.desktop" \
                "$out/share/applications/qubes-template-switcher.desktop"

              rm -f \
                "$qubes_manager_py/qvm_template_gui.py" \
                "$qubes_manager_py/template_manager.py" \
                "$qubes_manager_py/ui_qvmtemplate.py" \
                "$qubes_manager_py/ui_templateinstallconfirmdlg.py" \
                "$qubes_manager_py/ui_templateinstallprogressdlg.py" \
                "$qubes_manager_py/ui_templatemanager.py" \
                "$qubes_manager_py/__pycache__/"*template*.pyc \
                "$qubes_manager_py/__pycache__/qvm_template_gui."*.pyc

              for dist_info in "$out/${prev.python3.sitePackages}"/qubesmanager-*.dist-info; do
                if [ -d "$dist_info" ]; then
                  if [ -f "$dist_info/entry_points.txt" ]; then
                    sed -i \
                      -e '/^qubes-template-manager = /d' \
                      -e '/^qvm-template-gui = /d' \
                      "$dist_info/entry_points.txt"
                  fi
                  if [ -f "$dist_info/RECORD" ]; then
                    sed -i \
                      -e '/bin\/qubes-template-manager/d' \
                      -e '/bin\/qvm-template-gui/d' \
                      -e '/qubesmanager\/qvm_template_gui[.]/d' \
                      -e '/qubesmanager\/template_manager[.]/d' \
                      -e '/qubesmanager\/ui_qvmtemplate[.]/d' \
                      -e '/qubesmanager\/ui_templateinstall/d' \
                      -e '/qubesmanager\/ui_templatemanager[.]/d' \
                      "$dist_info/RECORD"
                  fi
                fi
              done

              if [ -f "$out/etc/xdg/menus/applications-merged/qubes-tools.menu" ]; then
                sed -i '/qubes-template-manager[.]desktop/d' \
                  "$out/etc/xdg/menus/applications-merged/qubes-tools.menu"
              fi

              if [ -f "$qubes_manager_py/ui_qubemanager.py" ]; then
                substituteInPlace "$qubes_manager_py/ui_qubemanager.py" \
                  --replace-fail "self.menu_system.addAction(self.action_manage_templates)" \
                    "self.action_manage_templates.setVisible(False)"
              fi

              if [ -f "$qubes_manager_py/qube_manager.py" ]; then
                substituteInPlace "$qubes_manager_py/qube_manager.py" \
                  --replace-fail "        spawn_in_background('qubes-template-manager')" \
                    "        return" \
                  --replace-fail "supported\\nInstall new template with Template Manager" \
                    "supported\\nInstall supported templates through Nixome config"
              fi

              if [ -f "$qubes_manager_py/tests/test_qube_manager.py" ]; then
                substituteInPlace "$qubes_manager_py/tests/test_qube_manager.py" \
                  --replace-fail "assert qubes_manager.action_manage_templates.isEnabled()" \
                    "assert not qubes_manager.action_manage_templates.isVisible()" \
                  --replace-fail "mock_subprocess.assert_called_once_with(['qubes-template-manager'])" \
                    "mock_subprocess.assert_not_called()"
              fi
            '';
          });
        };

      pkgsFor =
        system:
        import nixpkgs {
          inherit system;
          overlays = [
            nixos-qubes.overlays.default
            nixomeImageBuildToolsOverlay
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
      nestedDevSysNetPciAssignment = {
        backend = "dom0";
        id = "00_03.0";
        match = "port";
        mode = "required";
        options.no-strict-reset = true;
      };

      nixomeSupportConfigInputSource =
        let
          root = toString ./.;
          keep =
            path: _type:
            let
              rel = lib.removePrefix "${root}/" (toString path);
              keepTree = name: rel == name || lib.hasPrefix "${name}/" rel;
            in
            rel == "flake.nix" || rel == "flake.lock" || keepTree "lifecycle" || rel == "tests";
        in
        lib.cleanSourceWith {
          name = "nixome-support-config-input-source";
          src = ./.;
          filter = keep;
        };

      nixomeConfigInputs = {
        nixpkgs = "${nixpkgs}";
        nixpkgsLibvirt105 = "${nixpkgs-libvirt_10_5}";
        nixosQubes = "${nixos-qubes}";
        nixomeSupport = "${nixomeSupportConfigInputSource}";
      };

      mkNixomeQubeBuilders =
        system:
        import ./lifecycle/misc/lib/qube-template.nix {
          inherit nixpkgs system;
          nixpkgsLibvirt105Source = "${nixpkgs-libvirt_10_5}";
          nixomeSupport = "${self}";
          nixosQubesSource = "${nixos-qubes}";
        };

      mkBootstrapDomains =
        networkDebug:
        import ./lifecycle/bootstrap/default-domains {
          inherit
            lib
            mkNixomeQubeBuilders
            nixomeConfigInputs
            nixomeQubesVmKernelName
            networkDebug
            ;
        };

      bootstrapDomains = mkBootstrapDomains false;
      networkDebugBootstrapDomains = mkBootstrapDomains true;

      inherit (bootstrapDomains)
        materializeNixClosureDomain
        mkConfigDomainEntries
        mkSysFirewallDomainEntry
        mkSysGuiDomainEntry
        mkSysNetDomainEntry
        ;

      mkBootstrapConfigDomains =
        system: networkDebug:
        let
          bootstrapDomainEntriesSource = mkBootstrapDomains networkDebug;
          bootstrapDomainEntries = (mkConfigDomainEntries system) // {
            sys-net = bootstrapDomainEntriesSource.mkSysNetDomainEntry system;
            sys-firewall = bootstrapDomainEntriesSource.mkSysFirewallDomainEntry system;
            sys-gui = bootstrapDomainEntriesSource.mkSysGuiDomainEntry system;
          };
        in
        builtins.mapAttrs (_name: entry: materializeNixClosureDomain entry) bootstrapDomainEntries;

      mkConfigSystemSpecialArgs = system: networkDebug: {
        nixomeQubeKernel = {
          name = nixomeQubesVmKernelName;
          package = self.packages.${system}.nixome-qubes-vm-kernel;
        };
        nixomeConfigDomains = mkBootstrapConfigDomains system networkDebug;
        inherit nixomeConfigInputs;
      };

      mkVmConfiguration =
        {
          system ? "x86_64-linux",
          gui ? false,
          configSystem ? false,
          devMode ? true,
          networkDebug ? false,
        }:
        lib.nixosSystem {
          inherit system;
          specialArgs = lib.optionalAttrs configSystem (mkConfigSystemSpecialArgs system networkDebug);
          modules = [
            self.nixosModules.dom0
            (if gui then ./lifecycle/image/profiles/vm-gui-base.nix else ./lifecycle/image/profiles/vm-base.nix)
          ]
          ++ lib.optional devMode ./lifecycle/image/profiles/dev-mode.nix
          ++ lib.optional configSystem ./lifecycle/bootstrap/config-system.nix
          ++ lib.optional (configSystem && devMode) (
            { ... }:
            {
              # The nested QEMU launcher exposes the first virtio NIC for dom0
              # SSH/package access and the second virtio NIC for sys-net.
              nixome.qubes.domains.sys-net.devices.pci = [ nestedDevSysNetPciAssignment ];
            }
          );
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
        nixomeSystem = import ./lifecycle/misc/lib/system.nix;
        nixomeTopology = import ./lifecycle/misc/lib/topology.nix;
        nixomeQubeTemplate = import ./lifecycle/misc/lib/qube-template.nix;
      };

      nixome.configInputs = nixomeConfigInputs;

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
          nixome-config-apply = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-config-apply { };
          nixome-config-archive-candidate =
            pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-config-archive
              { };
          nixome-config-bootstrap-current =
            pkgs.callPackage ./lifecycle/bootstrap/tools/pkgs/nixome-config-bootstrap-current
              { };
          nixome-config-boot-verify =
            pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-config-boot-verify
              { };
          nixome-config-control = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-config-control { };
          nixome-config-inputs-import =
            pkgs.callPackage ./lifecycle/misc/tools/pkgs/nixome-config-inputs-import
              { };
          nixome-config-status = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-config-status { };
          nixome-config-vms-smoke = pkgs.callPackage ./lifecycle/misc/tools/pkgs/nixome-config-vms-smoke { };
          nixome-cli = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-cli { };
          nixome = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-cli { };
          codex = pkgs.codex;
          zellij = pkgs.zellij;
          nixome-nix-domain-storage =
            pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-nix-domain-storage
              { };
          nixome-nix-closure-domain-smoke =
            pkgs.callPackage ./lifecycle/misc/tools/pkgs/nixome-nix-closure-domain-smoke
              { };
          nixome-qubes-storage =
            pkgs.python3.pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-qubes-storage
              { };
          nixome-qube-bootstrap = pkgs.callPackage ./lifecycle/bootstrap/tools/pkgs/nixome-qube-bootstrap { };
          nixome-qube-gui-smoke = pkgs.callPackage ./lifecycle/misc/tools/pkgs/nixome-qube-gui-smoke { };
          qubes-gui-agent-linux = pkgs.qubes-gui-agent-linux;
          qubes-gui-daemon = pkgs.qubes-gui-daemon;
          nixome-setup-config = pkgs.callPackage ./lifecycle/misc/tools/pkgs/nixome-setup-config { };
          nixome-start-network-providers =
            pkgs.callPackage ./lifecycle/misc/tools/pkgs/nixome-start-network-providers
              { };
          nixome-qubes-smoke = pkgs.callPackage ./lifecycle/misc/tools/pkgs/nixome-qubes-smoke { };
          nixome-observe-dom0 = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/nixome-observe { };
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
          nixome-vm-dev-image = self.nixosConfigurations."vm-dev".config.system.build.image;
          nixome-vm-release-image = self.nixosConfigurations."vm-release".config.system.build.image;
          nixome-vm-image = self.nixosConfigurations.vm.config.system.build.image;
          nixome-vm-gui-dev-image = self.nixosConfigurations."vm-gui-dev".config.system.build.image;
          nixome-vm-gui-release-image = self.nixosConfigurations."vm-gui-release".config.system.build.image;
          nixome-vm-gui-image = self.nixosConfigurations.vm-gui.config.system.build.image;
          nixome-vm-gui-config-dev-image =
            self.nixosConfigurations."vm-gui-config-dev".config.system.build.image;
          nixome-vm-gui-config-release-image =
            self.nixosConfigurations."vm-gui-config-release".config.system.build.image;
          nixome-vm-gui-config-image = self.nixosConfigurations.vm-gui-config.config.system.build.image;
          nixome-vm-gui-config-network-debug-image =
            self.nixosConfigurations.vm-gui-config-network-debug.config.system.build.image;
          ovmf-fd = pkgs.OVMF.fd;
          qemu-kvm = pkgs.qemu_kvm;
          qemu-utils = pkgs.qemu-utils;
          default = self.packages.${system}.nixome-observe-dom0;
        }
      );

      checks = forAllSystems (system: {
        config-apply-package = self.packages.${system}.nixome-config-apply;
        config-archive-package = self.packages.${system}.nixome-config-archive-candidate;
        config-bootstrap-current-package = self.packages.${system}.nixome-config-bootstrap-current;
        config-boot-verify-package = self.packages.${system}.nixome-config-boot-verify;
        config-control-package = self.packages.${system}.nixome-config-control;
        codex-package = self.packages.${system}.codex;
        zellij-package = self.packages.${system}.zellij;
        config-inputs-import-package = self.packages.${system}.nixome-config-inputs-import;
        config-status-package = self.packages.${system}.nixome-config-status;
        nixome-cli-package = self.packages.${system}.nixome-cli;
        module-contract = mkModuleContractCheck system;
        config-vms-smoke-package = self.packages.${system}.nixome-config-vms-smoke;
        nix-closure-domain-smoke-package = self.packages.${system}.nixome-nix-closure-domain-smoke;
        qube-bootstrap-package = self.packages.${system}.nixome-qube-bootstrap;
        qube-gui-smoke-package = self.packages.${system}.nixome-qube-gui-smoke;
        setup-config-package = self.packages.${system}.nixome-setup-config;
        start-network-providers-package = self.packages.${system}.nixome-start-network-providers;
        observe-package = self.packages.${system}.nixome-observe-dom0;
      });

      formatter = forAllSystems (system: (pkgsFor system).nixfmt);

      nixosModules.dom0 =
        { lib, ... }:
        {
          nixpkgs.overlays = lib.mkAfter [
            self.overlays.nixomeImageBuildTools
          ];

          imports = [
            nixos-qubes.nixosModules.qubes
            ./lifecycle/misc/modules/dom0.nix
            ./lifecycle/misc/modules/qubes-topology.nix
          ];
        };

      nixosModules.default = self.nixosModules.dom0;

      nixosConfigurations.test = lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          self.nixosModules.dom0
          ./lifecycle/image/profiles/test.nix
        ];
      };

      nixosConfigurations."vm-dev" = mkVmConfiguration { devMode = true; };
      nixosConfigurations."vm-release" = mkVmConfiguration { devMode = false; };
      nixosConfigurations.vm = self.nixosConfigurations."vm-dev";

      nixosConfigurations."vm-gui-dev" = mkVmConfiguration {
        gui = true;
        devMode = true;
      };
      nixosConfigurations."vm-gui-release" = mkVmConfiguration {
        gui = true;
        devMode = false;
      };
      nixosConfigurations.vm-gui = self.nixosConfigurations."vm-gui-dev";

      nixosConfigurations."vm-gui-config-dev" = mkVmConfiguration {
        gui = true;
        configSystem = true;
        devMode = true;
      };
      nixosConfigurations."vm-gui-config-release" = mkVmConfiguration {
        gui = true;
        configSystem = true;
        devMode = false;
      };
      nixosConfigurations.vm-gui-config = self.nixosConfigurations."vm-gui-config-dev";
      nixosConfigurations.vm-gui-config-network-debug = mkVmConfiguration {
        gui = true;
        configSystem = true;
        devMode = true;
        networkDebug = true;
      };

      nixosConfigurations.nixome-domain-sys-config-vault =
        (mkConfigDomainEntries "x86_64-linux").sys-config-vault.configuration;
      nixosConfigurations.nixome-domain-sys-config-staging =
        (mkConfigDomainEntries "x86_64-linux").sys-config-staging.configuration;
      nixosConfigurations.nixome-domain-sys-config-validator =
        (mkConfigDomainEntries "x86_64-linux").sys-config-validator.configuration;
      nixosConfigurations.nixome-domain-sys-net = (mkSysNetDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.nixome-domain-sys-net-network-debug =
        (networkDebugBootstrapDomains.mkSysNetDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.nixome-domain-sys-firewall =
        (mkSysFirewallDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.nixome-domain-sys-gui = (mkSysGuiDomainEntry "x86_64-linux").configuration;
    };
}
