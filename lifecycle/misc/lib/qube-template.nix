{
  nixpkgs,
  nixpkgsLibvirt105Source,
  nixosQubesSource,
  airlockSupport,
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
    (import "${airlockSupport}/lifecycle/misc/overlays/qubes-manager-without-template-tools.nix")
    (final: prev: {
      # Mutter embeds this path; programs.xwayland.package alone is not enough.
      # Reuse upstream Qubes' shmoverride wrapper rather than patching the store.
      mutter = prev.mutter.override {
        xwayland = final.qubes-gui-daemon.xwayland.overrideAttrs (old: {
          meta = (old.meta or { }) // { mainProgram = "Xwayland"; };
        });
      };
    })
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

  mkNixClosureKernelOpts =
    configuration:
    {
      storeDevice ? "/dev/xvde",
      storeFormat ? "ext4",
      storeBlobDevice ? null,
    }:
    lib.concatStringsSep " " (
      [
        (mkQubeKernelOpts configuration)
        "airlock.system=${configuration.config.system.build.toplevel}"
        "airlock.storedev=${storeDevice}"
        "airlock.storefmt=${storeFormat}"
      ]
      ++ lib.optionals (storeBlobDevice != null) [
        "airlock.storeblobdev=${storeBlobDevice}"
      ]
    );

  mkNixClosureEarlyInit =
    pkgs.runCommand "airlock-nix-closure-early-init"
      {
        nativeBuildInputs = [ pkgs.pkgsStatic.stdenv.cc ];
        preferLocalBuild = true;
        allowSubstitutes = false;
      }
      ''
        set -eu

        mkdir -p "$out/sbin"
        cat > early-init.c <<'EOF'
        #define _GNU_SOURCE
        #include <errno.h>
        #include <fcntl.h>
        #include <limits.h>
        #include <stdarg.h>
        #include <stdio.h>
        #include <stdlib.h>
        #include <string.h>
        #include <sys/mount.h>
        #include <sys/stat.h>
        #include <sys/types.h>
        #include <sys/wait.h>
        #include <unistd.h>

        static void log_errno(const char *fmt, ...) {
            va_list ap;
            va_start(ap, fmt);
            fputs("airlock-early-init: ", stderr);
            vfprintf(stderr, fmt, ap);
            fprintf(stderr, ": %s\n", strerror(errno));
            va_end(ap);
        }

        static void log_msg(const char *fmt, ...) {
            va_list ap;
            va_start(ap, fmt);
            fputs("airlock-early-init: ", stderr);
            vfprintf(stderr, fmt, ap);
            fputc('\n', stderr);
            va_end(ap);
        }

        static int mkdir_p(const char *path, mode_t mode) {
            char tmp[PATH_MAX];
            char *p;

            if (strlen(path) >= sizeof(tmp)) {
                errno = ENAMETOOLONG;
                return -1;
            }

            strcpy(tmp, path);
            for (p = tmp + 1; *p; p++) {
                if (*p != '/') {
                    continue;
                }
                *p = '\0';
                if (mkdir(tmp, mode) != 0 && errno != EEXIST) {
                    return -1;
                }
                *p = '/';
            }

            if (mkdir(tmp, mode) != 0 && errno != EEXIST) {
                return -1;
            }
            return 0;
        }

        static void mount_basic(const char *source, const char *target, const char *type,
                                unsigned long flags, const char *data) {
            if (mkdir_p(target, 0755) != 0) {
                log_errno("cannot create %s", target);
                return;
            }
            if (mount(source, target, type, flags, data) != 0 && errno != EBUSY) {
                log_errno("cannot mount %s on %s", type, target);
            }
        }

        static int read_cmdline_option(const char *key, char *buf, size_t buflen) {
            int fd;
            ssize_t nread;
            char cmdline[8192];
            char *saveptr = NULL;
            char *token;
            size_t keylen = strlen(key);

            fd = open("/proc/cmdline", O_RDONLY | O_CLOEXEC);
            if (fd < 0) {
                log_errno("cannot open /proc/cmdline");
                return 0;
            }
            nread = read(fd, cmdline, sizeof(cmdline) - 1);
            close(fd);
            if (nread <= 0) {
                return 0;
            }
            cmdline[nread] = '\0';

            for (token = strtok_r(cmdline, " \t\r\n", &saveptr);
                 token != NULL;
                 token = strtok_r(NULL, " \t\r\n", &saveptr)) {
                if (strncmp(token, key, keylen) != 0 || token[keylen] != '=') {
                    continue;
                }
                snprintf(buf, buflen, "%s", token + keylen + 1);
                return buf[0] != '\0';
            }

            return 0;
        }

        static int system_init_present(const char *system_path) {
            char init_path[PATH_MAX];
            if (snprintf(init_path, sizeof(init_path), "%s/init", system_path) >= (int)sizeof(init_path)) {
                errno = ENAMETOOLONG;
                return 0;
            }
            return access(init_path, X_OK) == 0;
        }

        static int path_is_mountpoint(const char *path) {
            FILE *mountinfo;
            char line[8192];

            mountinfo = fopen("/proc/self/mountinfo", "r");
            if (mountinfo == NULL) {
                log_errno("cannot read /proc/self/mountinfo");
                return 0;
            }

            while (fgets(line, sizeof(line), mountinfo) != NULL) {
                char *saveptr = NULL;
                char *field;
                int field_index = 0;

                for (field = strtok_r(line, " ", &saveptr);
                     field != NULL;
                     field = strtok_r(NULL, " ", &saveptr)) {
                    field_index++;
                    if (field_index == 5) {
                        if (strcmp(field, path) == 0) {
                            fclose(mountinfo);
                            return 1;
                        }
                        break;
                    }
                }
            }

            fclose(mountinfo);
            return 0;
        }

        static int run_program(const char *path, char *const argv[], int noisy) {
            pid_t pid;
            int status;

            pid = fork();
            if (pid < 0) {
                if (noisy) {
                    log_errno("cannot fork %s", path);
                }
                return -1;
            }
            if (pid == 0) {
                execv(path, argv);
                _exit(127);
            }
            if (waitpid(pid, &status, 0) < 0) {
                if (noisy) {
                    log_errno("cannot wait for %s", path);
                }
                return -1;
            }
            return WIFEXITED(status) ? WEXITSTATUS(status) : -1;
        }

        static void try_mount_kernel_modules(void) {
            const char *devices[] = { "/dev/xvdd", "/dev/vdd", "/dev/sdd", NULL };
            const char *fstypes[] = { "ext4", "ext3", NULL };
            int i;

            if (path_is_mountpoint("/lib/modules")) {
                return;
            }
            if (mkdir_p("/lib/modules", 0755) != 0) {
                log_errno("cannot create /lib/modules");
                return;
            }
            for (i = 0; devices[i] != NULL; i++) {
                int j;
                if (access(devices[i], R_OK) != 0) {
                    continue;
                }
                for (j = 0; fstypes[j] != NULL; j++) {
                    if (mount(devices[i], "/lib/modules", fstypes[j], MS_RDONLY | MS_NODEV, "ro") == 0) {
                        log_msg("mounted Qubes kernel modules from %s", devices[i]);
                        return;
                    }
                }
            }
        }

        static void try_modprobe(const char *module) {
            char *const argv[] = { "/sbin/modprobe", (char *)module, NULL };
            if (access("/sbin/modprobe", X_OK) != 0) {
                return;
            }
            if (run_program("/sbin/modprobe", argv, 0) == 0) {
                log_msg("loaded kernel module %s", module);
            }
        }

        static int try_store_device(const char *device, const char *system_path,
                                    const char *store_format, const char *blob_device,
                                    int noisy) {
            const char *fstype = store_format;
            char mount_data[PATH_MAX + 32];

            if (access(device, R_OK) != 0) {
                return 0;
            }
            if (strcmp(store_format, "erofs") == 0) {
                if (blob_device == NULL || blob_device[0] == '\0') {
                    if (noisy) {
                        log_msg("erofs store requires airlock.storeblobdev");
                    }
                    return 0;
                }
                if (access(blob_device, R_OK) != 0) {
                    if (noisy) {
                        log_errno("cannot access candidate store blob device %s", blob_device);
                    }
                    return 0;
                }
                if (snprintf(mount_data, sizeof(mount_data), "ro,device=%s", blob_device) >= (int)sizeof(mount_data)) {
                    errno = ENAMETOOLONG;
                    if (noisy) {
                        log_errno("store blob device option is too long");
                    }
                    return 0;
                }
            } else {
                log_msg("unsupported airlock.storefmt=%s", store_format);
                return 0;
            }

            if (mount(device, "/nix/store", fstype, MS_RDONLY | MS_NODEV, mount_data) != 0) {
                if (noisy) {
                    log_errno("cannot mount %s candidate store device %s", fstype, device);
                }
                return 0;
            }

            if (system_init_present(system_path)) {
                log_msg("mounted read-only Nix store from %s", device);
                return 1;
            }

            if (noisy) {
                log_msg("mounted %s, but %s/init is not present", device, system_path);
            }
            if (umount2("/nix/store", 0) != 0 && noisy) {
                log_errno("cannot unmount rejected store device %s", device);
            }
            return 0;
        }

        static int mount_store(const char *system_path) {
            char explicit_device[PATH_MAX] = "";
            char explicit_blob_device[PATH_MAX] = "/dev/xvdf";
            char store_format[32] = "erofs";
            const char *fallback_devices[] = {
                "/dev/xvde", "/dev/xvdf", "/dev/xvdg", "/dev/xvdh",
                "/dev/xvdi", "/dev/xvdj", "/dev/xvdk", "/dev/xvdl",
                "/dev/vde", "/dev/vdf", "/dev/vdg", "/dev/vdh",
                "/dev/sde", "/dev/sdf", "/dev/sdg", "/dev/sdh",
                NULL
            };
            int attempt;

            if (mkdir_p("/nix/store", 0555) != 0) {
                log_errno("cannot create /nix/store mountpoint");
                return 0;
            }

            if (path_is_mountpoint("/nix/store") && system_init_present(system_path)) {
                log_msg("using pre-mounted /nix/store containing %s/init", system_path);
                return 1;
            }

            read_cmdline_option("airlock.storedev", explicit_device, sizeof(explicit_device));
            read_cmdline_option("airlock.storeblobdev", explicit_blob_device, sizeof(explicit_blob_device));
            read_cmdline_option("airlock.storefmt", store_format, sizeof(store_format));
            if (strcmp(store_format, "erofs") == 0) {
                try_mount_kernel_modules();
                try_modprobe("erofs");
            } else {
                log_msg("unsupported airlock.storefmt=%s", store_format);
                return 0;
            }

            for (attempt = 0; attempt < 30; attempt++) {
                int i;

                if (explicit_device[0] != '\0' &&
                    try_store_device(
                        explicit_device,
                        system_path,
                        store_format,
                        explicit_blob_device,
                        attempt == 29
                    )) {
                    return 1;
                }

                for (i = 0; fallback_devices[i] != NULL; i++) {
                    if (explicit_device[0] != '\0' &&
                        strcmp(explicit_device, fallback_devices[i]) == 0) {
                        continue;
                    }
                    if (try_store_device(
                        fallback_devices[i],
                        system_path,
                        store_format,
                        explicit_blob_device,
                        0
                    )) {
                        return 1;
                    }
                }

                sleep(1);
            }

            log_msg("could not mount a Nix store containing %s/init", system_path);
            return 0;
        }

        static int run_activate(const char *system_path) {
            char activate_path[PATH_MAX];
            char *const activate_argv[] = { activate_path, NULL };
            pid_t pid;
            int status;

            if (snprintf(activate_path, sizeof(activate_path), "%s/activate", system_path) >= (int)sizeof(activate_path)) {
                errno = ENAMETOOLONG;
                log_errno("activation path is too long");
                return -1;
            }

            pid = fork();
            if (pid < 0) {
                log_errno("cannot fork activation");
                return -1;
            }
            if (pid == 0) {
                execv(activate_path, activate_argv);
                log_errno("cannot exec %s", activate_path);
                _exit(127);
            }

            if (waitpid(pid, &status, 0) < 0) {
                log_errno("cannot wait for activation");
                return -1;
            }
            if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
                log_msg("activation failed with status %d", status);
                return -1;
            }

            return 0;
        }

        static void exec_real_init(const char *system_path, int argc, char **argv) {
            char init_path[PATH_MAX];
            char **init_argv;
            int i;

            if (snprintf(init_path, sizeof(init_path), "%s/init", system_path) >= (int)sizeof(init_path)) {
                errno = ENAMETOOLONG;
                log_errno("init path is too long");
                exit(1);
            }

            init_argv = calloc((size_t)argc + 1, sizeof(char *));
            if (init_argv == NULL) {
                log_errno("cannot allocate init argv");
                exit(1);
            }

            init_argv[0] = init_path;
            for (i = 1; i < argc; i++) {
                init_argv[i] = argv[i];
            }
            init_argv[argc] = NULL;

            execv(init_path, init_argv);
            log_errno("cannot exec %s", init_path);
            exit(127);
        }

        int main(int argc, char **argv) {
            char system_path[PATH_MAX] = "";

            mount_basic("proc", "/proc", "proc", MS_NOSUID | MS_NODEV | MS_NOEXEC, NULL);
            mount_basic("sysfs", "/sys", "sysfs", MS_NOSUID | MS_NODEV | MS_NOEXEC, NULL);
            mount_basic("devtmpfs", "/dev", "devtmpfs", MS_NOSUID, "mode=0755");
            mount_basic("tmpfs", "/run", "tmpfs", MS_NOSUID | MS_NODEV, "mode=0755");

            mkdir_p("/etc", 0755);
            mkdir_p("/nix/var/nix/profiles", 0755);
            mkdir_p("/run/qubes", 0755);
            mkdir_p("/var/log/qubes", 0755);
            if (symlink("/run", "/var/run") != 0 && errno != EEXIST) {
                log_errno("cannot initialize /var/run");
                return 1;
            }
            mkdir_p("/rw", 0755);
            mkdir_p("/home", 0755);
            mkdir_p("/usr/local", 0755);

            if (!read_cmdline_option("airlock.system", system_path, sizeof(system_path))) {
                log_msg("missing required kernel option airlock.system=/nix/store/...");
                return 1;
            }
            if (strncmp(system_path, "/nix/store/", 11) != 0) {
                log_msg("airlock.system must point into /nix/store: %s", system_path);
                return 1;
            }

            if (!mount_store(system_path)) {
                return 1;
            }
            if (run_activate(system_path) != 0) {
                return 1;
            }

            exec_real_init(system_path, argc, argv);
        }
        EOF

        "$CC" -Os -static -Wall -Wextra -o "$out/sbin/init" early-init.c
        chmod 0555 "$out/sbin/init"
      '';

  mkNixClosureRootImage =
    {
      imageName,
      rootImageSizeMiB ? 512,
    }:
    pkgs.runCommand "${imageName}-nix-closure-root-image"
      {
        nativeBuildInputs = [
          pkgs.coreutils
          pkgs.e2fsprogs
          pkgs.fakeroot
          pkgs.findutils
        ];
        preferLocalBuild = true;
        allowSubstitutes = false;
      }
      ''
        set -eu

        root="$PWD/root"
        mkdir -p \
          "$root/bin" \
          "$root/dev" \
          "$root/etc" \
          "$root/home" \
          "$root/lib/modules" \
          "$root/nix/store" \
          "$root/nix/var/nix/profiles" \
          "$root/proc" \
          "$root/run/qubes" \
          "$root/rw" \
          "$root/sbin" \
          "$root/sys" \
          "$root/tmp" \
          "$root/usr/local" \
          "$root/var/log/qubes" \
          "$out"

        ln -s /run "$root/var/run"
        install -m 0555 ${pkgs.pkgsStatic.busybox}/bin/busybox "$root/bin/busybox"
        ln -s /bin/busybox "$root/sbin/modprobe"
        chmod 0555 "$root/nix/store"
        install -m 0555 ${mkNixClosureEarlyInit}/sbin/init "$root/sbin/init"

        {
          printf 'schema=airlock.nix-closure-root.v1\n'
          printf 'contains_store=false\n'
          printf 'init=/sbin/init\n'
          printf 'store_mountpoint=/nix/store\n'
          printf 'store_device_default=/dev/xvde\n'
          printf 'store_blob_device_default=/dev/xvdf\n'
          printf 'store_formats=erofs\n'
        } > "$out/MANAGED_BY_AIRLOCK"

        (cd "$root" && find . -mindepth 1 -print | sort) > "$out/root-files.txt"

        truncate -s ${toString rootImageSizeMiB}M "$out/root.img"
        # As in NixOS make-ext4-fs, record root ownership in the image without
        # requiring a privileged build. Otherwise -d preserves the build user's
        # UID/GID, making /var user-owned and preventing safe tmpfiles setup.
        fakeroot mkfs.ext4 -q -F -L nixroot -d "$root" "$out/root.img"

        cat > "$out/manifest.json" <<EOF
        {"schema":"airlock.nix-closure-root-image.v1","name":"${imageName}","image":"root.img","contains_store":false,"init":"/sbin/init","store_mountpoint":"/nix/store","store_device_default":"/dev/xvde","store_blob_device_default":"/dev/xvdf","store_formats":["erofs"]}
        EOF
      '';

  mkNixStoreClosureManifest =
    {
      name,
      rootPaths,
      device ? "/dev/xvde",
      mountpoint ? "/nix/store",
    }:
    let
      closure = pkgs.closureInfo {
        inherit rootPaths;
      };
      rootPathsJson = builtins.toJSON (builtins.map toString rootPaths);
    in
    pkgs.runCommand "${name}-store-closure"
      {
        nativeBuildInputs = [
          pkgs.coreutils
          pkgs.jq
        ];
        preferLocalBuild = true;
        allowSubstitutes = false;
      }
      ''
        set -eu

        mkdir -p "$out"
        cp ${closure}/store-paths "$out/store-paths"

        object_count="$(wc -l < "$out/store-paths" | tr -d ' ')"
        jq -n \
          --arg name "${name}" \
          --arg device "${device}" \
          --arg mountpoint "${mountpoint}" \
          --argjson root_paths '${rootPathsJson}' \
          --argjson object_count "$object_count" \
          '{
            schema: "airlock.store-closure-manifest.v1",
            name: $name,
            root_paths: $root_paths,
            store_paths_file: "store-paths",
            object_count: $object_count,
            mountpoint: $mountpoint,
            device: $device,
            read_only: true
          }' > "$out/manifest.json"
      '';

  mkNixNativeDomain =
    {
      name,
      modules,
      guestRole ? "base",
      airlockConfigInputs ? null,
      airlockPrebuiltTemplateSources ? { },
      extraSpecialArgs ? { },
      additionalRootSpace ? null,
      configurationName ? "airlock-domain-${builtins.replaceStrings [ "." ] [ "-" ] name}",
      rootImageName ? "${configurationName}-root-image",
      storeArtifactName ? "${configurationName}-store-closure",
      kernelName ? "6.12.64-1.fc41",
      label ? "green",
      netvm ? null,
      prefs ? { },
      features ? { },
      services ? { },
      devices ? { },
      state ? { },
      rootImageSizeMiB ? 512,
      storeDevice ? "/dev/xvde",
      storeBlobDevice ? "/dev/xvdf",
    }:
    let
      configuration = lib.nixosSystem {
        inherit system;
        specialArgs = {
          inherit
            airlockConfigInputs
            airlockPrebuiltTemplateSources
            ;
        }
        // extraSpecialArgs;
        modules = [
          "${airlockSupport}/lifecycle/misc/modules/airlock-minimal.nix"
          "${nixosQubesSource}/nixos/modules/virtualisation/qubes"
          "${airlockSupport}/lifecycle/misc/modules/qube-guest.nix"
          {
            nixpkgs.pkgs = pkgs;
            airlock.minimalGuest.enable = true;
            airlock.qubeGuest.enable = true;
            airlock.qubeGuest.role = guestRole;
            system.nixos.versionSuffix = lib.mkForce "";
          }
        ]
        ++ modules;
      };
      rootImageSizeMiBEffective =
        if additionalRootSpace == null then
          rootImageSizeMiB
        else if builtins.isInt additionalRootSpace then
          additionalRootSpace
        else if builtins.isString additionalRootSpace && lib.hasSuffix "M" additionalRootSpace then
          builtins.fromJSON (lib.removeSuffix "M" additionalRootSpace)
        else if builtins.isString additionalRootSpace && lib.hasSuffix "G" additionalRootSpace then
          (builtins.fromJSON (lib.removeSuffix "G" additionalRootSpace)) * 1024
        else
          throw "mkNixNativeDomain.additionalRootSpace must be an integer MiB value or a string ending in M/G";
      storeFormat = "erofs";
      rootImage = mkNixClosureRootImage {
        imageName = rootImageName;
        rootImageSizeMiB = rootImageSizeMiBEffective;
      };
      storeArtifact = mkNixStoreClosureManifest {
        name = storeArtifactName;
        rootPaths = [ configuration.config.system.build.toplevel ];
        device = storeDevice;
      };
      defaultPrefs = {
        autostart = false;
        default_user = "user";
        include_in_backups = true;
        kernel = kernelName;
        kernelopts = mkNixClosureKernelOpts configuration {
          inherit storeDevice storeFormat;
          storeBlobDevice = storeBlobDevice;
        };
        memory = 1024;
        maxmem = 2048;
        qrexec_timeout = 120;
        vcpus = 1;
        virt_mode = "pvh";
      };
      defaultFeatures = {
        "airlock-backend" = "nix-closure";
        "airlock-managed" = "1";
        "airlock-role" = "nix-native";
        os = "Linux";
        qrexec = "1";
      };
      domain = {
        inherit label netvm services devices;
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
            mode = "shared-closure";
            sourceRef = {
              build = "packages.${system}.${storeArtifactName}";
              subpath = "";
            };
            mountpoint = "/nix/store";
            device = storeDevice;
            blobDevice = storeBlobDevice;
            viewBackend = "erofs-objectmap";
            readOnly = true;
          };
          state = {
            persistent = "${name}-private";
            volatile = null;
          }
          // state;
        };
      };
    in
    {
      inherit
        configuration
        domain
        rootImage
        rootImageName
        storeArtifact
        storeArtifactName
        ;
      kernelopts = mkNixClosureKernelOpts configuration {
        inherit storeDevice storeFormat;
        storeBlobDevice = storeBlobDevice;
      };
      configurationName = configurationName;
    };
in
{
  inherit
    mkNixNativeDomain
    mkNixClosureKernelOpts
    mkNixClosureRootImage
    mkNixStoreClosureManifest
    mkQubeKernelOpts
    pkgs
    qubesOverlays
    ;
}
