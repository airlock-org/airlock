{
  description = "Airlock: Qubes control stack on NixOS Xen dom0";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-libvirt_10_5.url = "github:NixOS/nixpkgs/e0464e47880a69896f0fb1810f00e0de469f770a";
    dank-material-shell = {
      url = "github:AvengeMedia/DankMaterialShell/v1.6.1";
      inputs.nixpkgs.follows = "nixpkgs";
    };

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

      airlockImageBuildToolsOverlay =
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
              # Airlock templates are built from Nix config. Do not expose the
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
                    "supported\\nInstall supported templates through Airlock config"
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
            airlockImageBuildToolsOverlay
          ];
          config.allowUnfreePredicate =
            pkg:
            builtins.elem (pkg.pname or (lib.getName pkg)) [
              "qubes-artwork"
              "qubes-vmm-stubdom-linux"
            ];
        };

      boolString = value: if value then "true" else "false";

      airlockQubesVmKernelName = "6.12.64-1.fc41";
      nestedDevSysNetPciAssignment = {
        backend = "dom0";
        id = "00_03.0";
        match = "port";
        mode = "required";
        options.no-strict-reset = true;
      };

      airlockSupportConfigInputSource =
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
          name = "airlock-support-config-input-source";
          src = ./.;
          filter = keep;
        };

      airlockConfigInputs = {
        nixpkgs = "${nixpkgs}";
        nixpkgsLibvirt105 = "${nixpkgs-libvirt_10_5}";
        nixosQubes = "${nixos-qubes}";
        airlockSupport = "${airlockSupportConfigInputSource}";
      };

      mkAirlockQubeBuilders =
        system:
        import ./lifecycle/misc/lib/qube-template.nix {
          inherit nixpkgs system;
          nixpkgsLibvirt105Source = "${nixpkgs-libvirt_10_5}";
          airlockSupport = "${self}";
          nixosQubesSource = "${nixos-qubes}";
        };

      mkBootstrapDomains =
        networkDebug:
        import ./lifecycle/bootstrap/default-domains {
          inherit
            lib
            mkAirlockQubeBuilders
            airlockConfigInputs
            airlockQubesVmKernelName
            networkDebug
            ;
        };

      bootstrapDomains = mkBootstrapDomains false;
      networkDebugBootstrapDomains = mkBootstrapDomains true;

      mkSysGpuDomainEntry = import ./lifecycle/misc/experiments/sys-gpu.nix {
        inherit mkAirlockQubeBuilders;
        kernelName = airlockQubesVmKernelName;
      };

      inherit (bootstrapDomains)
        materializeNixClosureDomain
        mkConfigDomainEntries
        mkCmdVmDomainEntry
        mkHomeVmDomainEntry
        mkBrowserVmDomainEntry
        mkSysFirewallDomainEntry
        mkSysGuiDomainEntry
        mkSysDevDomainEntry
        mkSysNetDomainEntry
        ;

      mkBootstrapConfigDomains =
        system: networkDebug:
        let
          bootstrapDomainEntriesSource = mkBootstrapDomains networkDebug;
          bootstrapDomainEntries = (mkConfigDomainEntries system) // {
            usr-cmd = bootstrapDomainEntriesSource.mkCmdVmDomainEntry system;
            usr-home = bootstrapDomainEntriesSource.mkHomeVmDomainEntry system;
            usr-browser = bootstrapDomainEntriesSource.mkBrowserVmDomainEntry system;
            sys-net = bootstrapDomainEntriesSource.mkSysNetDomainEntry system;
            sys-firewall = bootstrapDomainEntriesSource.mkSysFirewallDomainEntry system;
            sys-gui = bootstrapDomainEntriesSource.mkSysGuiDomainEntry system;
          };
        in
        builtins.mapAttrs (_name: entry: materializeNixClosureDomain entry) bootstrapDomainEntries;

      mkConfigSystemSpecialArgs = system: networkDebug: {
        airlockQubeKernel = {
          name = airlockQubesVmKernelName;
          package = self.packages.${system}.airlock-qubes-vm-kernel;
        };
        airlockConfigDomains = mkBootstrapConfigDomains system networkDebug;
        inherit airlockConfigInputs;
      };

      mkMinimalSysGuiDomainEntry =
        system:
        bootstrapDomains.mkSysGuiDomainEntryWith system {
          personalLaunchers = false;
          configurationName = "airlock-domain-sys-gui-minimal";
        };

      mkVmConfiguration =
        {
          system ? "x86_64-linux",
          gui ? false,
          configSystem ? false,
          devMode ? true,
          networkDebug ? false,
          gpuLab ? false,
          guiMinimal ? false,
        }:
        assert !guiMinimal || (gui && !configSystem && !gpuLab);
        lib.nixosSystem {
          inherit system;
          specialArgs =
            lib.optionalAttrs configSystem (
              (mkConfigSystemSpecialArgs system networkDebug)
              // lib.optionalAttrs gpuLab {
                airlockConfigDomains = (mkBootstrapConfigDomains system networkDebug) // {
                  sys-gpu = materializeNixClosureDomain (mkSysGpuDomainEntry system);
                };
              }
            )
            // lib.optionalAttrs guiMinimal {
              airlockQubeKernel = {
                name = airlockQubesVmKernelName;
                package = self.packages.${system}.airlock-qubes-vm-kernel;
              };
              airlockGuiDomain = materializeNixClosureDomain (mkMinimalSysGuiDomainEntry system);
            };
          modules = [
            self.nixosModules.dom0
            (if gui then ./lifecycle/image/profiles/vm-gui.nix else ./lifecycle/image/profiles/vm.nix)
          ]
          ++ lib.optional devMode ./lifecycle/image/profiles/dev-mode.nix
          ++ lib.optional configSystem ./lifecycle/bootstrap/config-system.nix
          ++ lib.optional guiMinimal ./lifecycle/image/profiles/vm-gui-minimal.nix
          ++ lib.optional (configSystem && devMode) (
            { ... }:
            {
              # The nested QEMU launcher exposes the first virtio NIC for dom0
              # SSH/package access and the second virtio NIC for sys-net.
              airlock.qubes.domains.sys-net.devices.pci = [ nestedDevSysNetPciAssignment ];
            }
          );
        };

      usbSysGuiLoginModule = { lib, ... }: {
        # Shared by both USB graphics modes. Set the developer login at boot
        # through NixOS account configuration, without a qrexec password update.
        users.mutableUsers = lib.mkForce false;
        users.users.user.initialPassword = lib.mkForce "password";
        users.users.user.initialHashedPassword = lib.mkForce null;
      };

      mkUsbSysGuiDomainEntry = system:
        bootstrapDomains.mkSysGuiDomainEntryWith system {
          personalLaunchers = false;
          configurationName = "airlock-domain-sys-gui-usb";
          extraModules = [
            usbSysGuiLoginModule
            ({ ... }: { airlock.devDom0Terminal.guest.enable = true; })
          ];
        };

      mkUsbAmdSysGuiDomainEntry = system: desktop: personalLaunchers: directDom0Terminal:
        (mkAirlockQubeBuilders system).mkNixNativeDomain {
          name = "sys-gui";
          guestRole = "sys-gui";
          configurationName = "airlock-domain-sys-gui-usb-gui-lab${
            lib.optionalString (desktop == "hyprland") "-hyprland"
            + lib.optionalString (desktop == "xfce") "-xfce"
          }";
          kernelName = airlockQubesVmKernelName;
          label = "black";
          netvm = null;
          prefs = {
            autostart = false;
            guivm = null;
            audiovm = null;
            virt_mode = "hvm";
            memory = 4096;
            maxmem = 4096;
            vcpus = 4;
          };
          features = { "video-model" = "none"; "airlock-role" = "sys-gui"; };
          services = { guivm = true; "guivm-gui-agent" = false; };
          devices.pci = [ ];
          modules = [
            usbSysGuiLoginModule
            ./lifecycle/misc/modules/sys-gui/desktop/selector.nix
          ] ++ lib.optionals (desktop == "hyprland") [
            ./lifecycle/misc/modules/sys-gui/desktop/hyprland/hidpi-amd-lab.nix
          ] ++ [
            ({ pkgs, lib, ... }: {
              networking.hostName = "sys-gui";
              airlock.sysGuiDesktop.session = desktop;
              airlock.sysGuiHyprland = lib.mkIf (desktop == "hyprland") {
                shellPackage = import ./lifecycle/misc/modules/sys-gui/desktop/hyprland/dms-shell-hidpi.nix {
                  package = inputs.dank-material-shell.packages.${system}.default;
                };
              };
              airlock.sysGuiGuest = {
                displayMode = "physical-amd-lab";
                physicalDesktopOnBoot = true;
                personalLaunchers.enable = personalLaunchers;
                sysDevTailscaleShortcut.enable = personalLaunchers && !directDom0Terminal;
              };
              airlock.devDom0Terminal.guest.enable = directDom0Terminal;
              hardware.enableRedistributableFirmware = true;
              hardware.graphics.enable = true;
              services.xserver.videoDrivers = [ "amdgpu" ];
              boot.kernelModules = [ "amdgpu" ];
              environment.systemPackages = [ pkgs.pciutils pkgs.mesa-demos ];
            })
          ];
        };

      mkUsbConfiguration = system:
      let
        entries = {
          hybrid = mkUsbSysGuiDomainEntry system;
          gui-lab = mkUsbAmdSysGuiDomainEntry system "gnome" false true;
          gui-lab-hyprland = mkUsbAmdSysGuiDomainEntry system "hyprland" false true;
          gui-lab-xfce = mkUsbAmdSysGuiDomainEntry system "xfce" false true;
          lab-gui = mkUsbAmdSysGuiDomainEntry system "gnome" true false;
          lab-sys-dev = (bootstrapDomains.mkSysDevDomainEntry system) // { preparedDomainName = "sys-dev"; };
          lab-usr-home = (bootstrapDomains.mkHomeVmDomainEntry system) // { preparedDomainName = "usr-home"; };
          lab-usr-browser = (bootstrapDomains.mkBrowserVmDomainEntry system) // { preparedDomainName = "usr-browser"; };
        };
        preparedStore = import ./lifecycle/image/prepared-guest-store.nix {
          inherit lib entries;
          pkgs = pkgsFor system;
        };
        preparedDomain = name:
          let domain = materializeNixClosureDomain entries.${name};
          in domain // {
            backend = domain.backend // {
              store = domain.backend.store // {
                source = "${preparedStore.descriptors}/${name}";
              };
            };
          };
      in lib.nixosSystem {
        inherit system;
        specialArgs = {
          airlockQubeKernel = {
            name = airlockQubesVmKernelName;
            package = self.packages.${system}.airlock-qubes-vm-kernel;
          };
          airlockGuiDomain = preparedDomain "hybrid";
          airlockUsbPreparedStore = preparedStore;
          airlockUsbGuestEntries = entries;
        };
        modules = [
          self.nixosModules.dom0
          ./lifecycle/image/profiles/usb-dev.nix
          ./lifecycle/image/hardware/thinkpad-t14-gen3-amd.nix
          ({ lib, ... }: {
            specialisation.usb-text-recovery.configuration = {
              imports = [ ./lifecycle/image/profiles/usb-headless.nix ];
              # Do not reconcile or start VMs from possibly changed qubes.xml.
              airlock.qubes.setupConfigOnBoot.enable = lib.mkForce false;
            };
            specialisation.usb-gui-lab.configuration = {
              imports = [ ./lifecycle/image/profiles/usb-gui-lab.nix ];
              # Replace hybrid guest fields, leaving PCI selection to the
              # reusable module and this machine's hardware inventory.
              airlock.qubes.domains.sys-gui = lib.mapAttrs (_: lib.mkForce)
                (builtins.removeAttrs (preparedDomain "gui-lab") [ "devices" ]);
            };
            # Keep the guest builders below for future experiments, but do not
            # emit Hyprland or XFCE boot-menu entries during USB-lab work.
            # specialisation.usb-gui-lab-hyprland.configuration = { ... };
            # specialisation.usb-gui-lab-xfce.configuration = { ... };
            specialisation.usb-lab.configuration = {
              imports = [ ./lifecycle/image/profiles/usb-lab.nix ];
              airlock.qubes.domains = lib.mapAttrs (_: lib.mkForce) {
                # Keep this explicit physical assignment in the forced prepared
                # descriptor. Without it, the descriptor's device defaults can
                # win during specialisation merging and sys-gui starts headless.
                sys-gui = (builtins.removeAttrs (preparedDomain "lab-gui") [ "devices" ]) // {
                  devices.pci = [
                    {
                      backend = "dom0";
                      id = "00_08.1-00_00.0";
                      mode = "required";
                      match = "port";
                      options = { };
                    }
                  ];
                };
                sys-dev = preparedDomain "lab-sys-dev";
                usr-home = preparedDomain "lab-usr-home";
                usr-browser = (preparedDomain "lab-usr-browser") // { netvm = "sys-dev"; };
              };
            };
          })
        ];
      };

      mkModuleContractCheck =
        system:
        let
          pkgs = pkgsFor system;
          cfg = self.nixosConfigurations.test.config;
          optOut = cfg.virtualisation.qubes.optOutRecommendedConfiguration;
          hasOptOut = name: builtins.elem name optOut;
        in
        pkgs.runCommand "airlock-module-contract"
          {
            preferLocalBuild = true;
            allowSubstitutes = false;

            airlockDom0Enabled = boolString cfg.airlock.dom0.enable;
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

            check airlock-dom0-enabled true "$airlockDom0Enabled"
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

            printf 'airlock module contract ok\n'
            touch "$out"
          '';
    in
    {
      lib = {
        airlockSystem = import ./lifecycle/misc/lib/system.nix;
        airlockTopology = import ./lifecycle/misc/lib/topology.nix;
        airlockQubeTemplate = import ./lifecycle/misc/lib/qube-template.nix;
      };

      airlock.configInputs = airlockConfigInputs;

      overlays.airlockImageBuildTools = airlockImageBuildToolsOverlay;

      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
          templateBuilders = mkAirlockQubeBuilders system;
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
          qubes-input-proxy = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/qubes-input-proxy { };
          airlock-config-apply = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-config-apply { };
          airlock-config-archive-candidate =
            pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-config-archive
              { };
          airlock-config-bootstrap-current =
            pkgs.callPackage ./lifecycle/bootstrap/tools/pkgs/airlock-config-bootstrap-current
              { };
          airlock-config-boot-verify =
            pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-config-boot-verify
              { };
          airlock-config-control = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-config-control { };
          airlock-config-inputs-import =
            pkgs.callPackage ./lifecycle/misc/tools/pkgs/airlock-config-inputs-import
              { };
          airlock-config-status = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-config-status { };
          airlock-config-vms-smoke =
            pkgs.callPackage ./lifecycle/misc/tools/pkgs/airlock-config-vms-smoke
              { };
          airlock-cli = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-cli { };
          airlock = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-cli { };
          codex = pkgs.codex;
          zellij = pkgs.zellij;
          airlock-nix-domain-storage =
            pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-nix-domain-storage
              { };
          airlock-nix-closure-domain-smoke =
            pkgs.callPackage ./lifecycle/misc/tools/pkgs/airlock-nix-closure-domain-smoke
              { };
          airlock-qubes-storage =
            pkgs.python3.pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-qubes-storage
              { };
          airlock-qube-bootstrap =
            pkgs.callPackage ./lifecycle/bootstrap/tools/pkgs/airlock-qube-bootstrap
              { };
          airlock-qube-gui-smoke = pkgs.callPackage ./lifecycle/misc/tools/pkgs/airlock-qube-gui-smoke { };
          qubes-gui-agent-linux = pkgs.qubes-gui-agent-linux;
          qubes-gui-daemon = pkgs.qubes-gui-daemon;
          airlock-setup-config = pkgs.callPackage ./lifecycle/misc/tools/pkgs/airlock-setup-config { };
          airlock-start-network-providers =
            pkgs.callPackage ./lifecycle/misc/tools/pkgs/airlock-start-network-providers
              { };
          airlock-qubes-smoke = pkgs.callPackage ./lifecycle/misc/tools/pkgs/airlock-qubes-smoke { };
          airlock-observe-dom0 = pkgs.callPackage ./lifecycle/runtime/tools/pkgs/airlock-observe { };
          airlock-gpu-probe = pkgs.callPackage ./lifecycle/misc/tools/pkgs/airlock-gpu-probe { };
          airlock-domain-sys-gpu-root-image = (mkSysGpuDomainEntry system).rootImage;
          airlock-domain-sys-gpu-store-closure = (mkSysGpuDomainEntry system).storeArtifact;
          airlock-vm-gpu-lab-image = self.nixosConfigurations.vm-gpu-lab.config.system.build.image;
          airlock-vm-gui-minimal-image = self.nixosConfigurations.vm-gui-minimal.config.system.build.image;
          airlock-usb-dev-image = self.nixosConfigurations.usb-dev.config.system.build.image;
          airlock-usb-dom0-console-image = self.nixosConfigurations.usb-dom0-console.config.system.build.image;
          airlock-domain-sys-gui-minimal-root-image = (mkMinimalSysGuiDomainEntry system).rootImage;
          airlock-domain-sys-gui-minimal-store-closure = (mkMinimalSysGuiDomainEntry system).storeArtifact;
          airlock-qubes-vm-kernel = pkgs.repackRpmKernel {
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
          airlock-nix-closure-root-image-test = templateBuilders.mkNixClosureRootImage {
            imageName = "airlock-nix-closure-root-image-test";
            rootImageSizeMiB = 64;
          };
          airlock-nix-closure-store-manifest-test = templateBuilders.mkNixStoreClosureManifest {
            name = "airlock-nix-closure-store-manifest-test";
            rootPaths = [
              (pkgs.writeText "airlock-store-manifest-test-root" "airlock-store-manifest-test\n")
            ];
          };
          airlock-vm-dev-image = self.nixosConfigurations."vm-dev".config.system.build.image;
          airlock-vm-release-image = self.nixosConfigurations."vm-release".config.system.build.image;
          airlock-vm-image = self.nixosConfigurations.vm.config.system.build.image;
          airlock-vm-gui-dev-image = self.nixosConfigurations."vm-gui-dev".config.system.build.image;
          airlock-vm-gui-release-image = self.nixosConfigurations."vm-gui-release".config.system.build.image;
          airlock-vm-gui-image = self.nixosConfigurations.vm-gui.config.system.build.image;
          airlock-vm-gui-config-dev-image =
            self.nixosConfigurations."vm-gui-config-dev".config.system.build.image;
          airlock-vm-gui-config-release-image =
            self.nixosConfigurations."vm-gui-config-release".config.system.build.image;
          airlock-vm-gui-config-image = self.nixosConfigurations.vm-gui-config.config.system.build.image;
          airlock-vm-gui-config-network-debug-image =
            self.nixosConfigurations.vm-gui-config-network-debug.config.system.build.image;
          ovmf-fd = pkgs.OVMF.fd;
          qemu-kvm = pkgs.qemu_kvm;
          qemu-utils = pkgs.qemu-utils;
          default = self.packages.${system}.airlock-observe-dom0;
        }
      );

      checks = forAllSystems (system: {
        config-apply-package = self.packages.${system}.airlock-config-apply;
        config-archive-package = self.packages.${system}.airlock-config-archive-candidate;
        config-bootstrap-current-package = self.packages.${system}.airlock-config-bootstrap-current;
        config-boot-verify-package = self.packages.${system}.airlock-config-boot-verify;
        config-control-package = self.packages.${system}.airlock-config-control;
        codex-package = self.packages.${system}.codex;
        zellij-package = self.packages.${system}.zellij;
        config-inputs-import-package = self.packages.${system}.airlock-config-inputs-import;
        config-status-package = self.packages.${system}.airlock-config-status;
        airlock-cli-package = self.packages.${system}.airlock-cli;
        module-contract = mkModuleContractCheck system;
        config-vms-smoke-package = self.packages.${system}.airlock-config-vms-smoke;
        nix-closure-domain-smoke-package = self.packages.${system}.airlock-nix-closure-domain-smoke;
        qube-bootstrap-package = self.packages.${system}.airlock-qube-bootstrap;
        qube-gui-smoke-package = self.packages.${system}.airlock-qube-gui-smoke;
        setup-config-package = self.packages.${system}.airlock-setup-config;
        start-network-providers-package = self.packages.${system}.airlock-start-network-providers;
        observe-package = self.packages.${system}.airlock-observe-dom0;
      });

      formatter = forAllSystems (system: (pkgsFor system).nixfmt);

      nixosModules.dom0 =
        { lib, ... }:
        {
          nixpkgs.overlays = lib.mkAfter [
            self.overlays.airlockImageBuildTools
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

      # Minimal nested desktop harness. No physical GPU assignment and no
      # dependency on the full bootstrap/configuration VM topology.
      nixosConfigurations.usb-dev = mkUsbConfiguration "x86_64-linux";
      nixosConfigurations.airlock-domain-sys-gui-usb =
        (mkUsbSysGuiDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.airlock-domain-sys-gui-usb-gui-lab =
        (mkUsbAmdSysGuiDomainEntry "x86_64-linux" "gnome" false true).configuration;
      nixosConfigurations.airlock-domain-sys-gui-usb-gui-lab-hyprland =
        (mkUsbAmdSysGuiDomainEntry "x86_64-linux" "hyprland" false true).configuration;
      nixosConfigurations.airlock-domain-sys-gui-usb-gui-lab-xfce =
        (mkUsbAmdSysGuiDomainEntry "x86_64-linux" "xfce" false true).configuration;
      nixosConfigurations.airlock-domain-sys-gui-usb-lab =
        (mkUsbAmdSysGuiDomainEntry "x86_64-linux" "gnome" true false).configuration;

      nixosConfigurations.usb-dom0-console = lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          self.nixosModules.dom0
          ./lifecycle/image/profiles/usb-dom0-console.nix
        ];
      };

      nixosConfigurations.vm-gui-minimal = mkVmConfiguration {
        gui = true;
        guiMinimal = true;
        devMode = false;
      };
      nixosConfigurations.airlock-domain-sys-gui-minimal =
        (mkMinimalSysGuiDomainEntry "x86_64-linux").configuration;

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

      # Separate development image; the normal bootstrap and XFCE session are
      # unchanged. sys-gpu has no physical PCI assignments and starts on demand.
      nixosConfigurations.vm-gpu-lab = mkVmConfiguration {
        gui = true;
        configSystem = true;
        gpuLab = true;
      };

      nixosConfigurations.airlock-domain-sys-gpu = (mkSysGpuDomainEntry "x86_64-linux").configuration;

      nixosConfigurations.airlock-domain-sys-config-vault =
        (mkConfigDomainEntries "x86_64-linux").sys-config-vault.configuration;
      nixosConfigurations.airlock-domain-sys-config-staging =
        (mkConfigDomainEntries "x86_64-linux").sys-config-staging.configuration;
      nixosConfigurations.airlock-domain-sys-config-validator =
        (mkConfigDomainEntries "x86_64-linux").sys-config-validator.configuration;
      nixosConfigurations.airlock-domain-sys-net = (mkSysNetDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.airlock-domain-sys-net-network-debug =
        (networkDebugBootstrapDomains.mkSysNetDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.airlock-domain-sys-firewall =
        (mkSysFirewallDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.airlock-domain-sys-dev =
        (mkSysDevDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.airlock-domain-usr-home =
        (mkHomeVmDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.airlock-domain-usr-browser =
        (mkBrowserVmDomainEntry "x86_64-linux").configuration;
      nixosConfigurations.airlock-domain-sys-gui = (mkSysGuiDomainEntry "x86_64-linux").configuration;
    };
}
