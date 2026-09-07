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

  mkNixClosureKernelOpts =
    configuration:
    {
      storeDevice ? "/dev/xvde",
    }:
    "${mkQubeKernelOpts configuration} nixome.system=${configuration.config.system.build.toplevel} nixome.storedev=${storeDevice}";

  mkNixClosureEarlyInit =
    pkgs.runCommand "nixome-nix-closure-early-init"
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
            fputs("nixome-early-init: ", stderr);
            vfprintf(stderr, fmt, ap);
            fprintf(stderr, ": %s\n", strerror(errno));
            va_end(ap);
        }

        static void log_msg(const char *fmt, ...) {
            va_list ap;
            va_start(ap, fmt);
            fputs("nixome-early-init: ", stderr);
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

        static int try_store_device(const char *device, const char *system_path, int noisy) {
            if (access(device, R_OK) != 0) {
                return 0;
            }

            if (mount(device, "/nix/store", "ext4", MS_RDONLY | MS_NODEV, "ro") != 0) {
                if (noisy) {
                    log_errno("cannot mount candidate store device %s", device);
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

            read_cmdline_option("nixome.storedev", explicit_device, sizeof(explicit_device));

            for (attempt = 0; attempt < 30; attempt++) {
                int i;

                if (explicit_device[0] != '\0' &&
                    try_store_device(explicit_device, system_path, attempt == 29)) {
                    return 1;
                }

                for (i = 0; fallback_devices[i] != NULL; i++) {
                    if (explicit_device[0] != '\0' &&
                        strcmp(explicit_device, fallback_devices[i]) == 0) {
                        continue;
                    }
                    if (try_store_device(fallback_devices[i], system_path, 0)) {
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

            if (!read_cmdline_option("nixome.system", system_path, sizeof(system_path))) {
                log_msg("missing required kernel option nixome.system=/nix/store/...");
                return 1;
            }
            if (strncmp(system_path, "/nix/store/", 11) != 0) {
                log_msg("nixome.system must point into /nix/store: %s", system_path);
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
          pkgs.findutils
        ];
        preferLocalBuild = true;
        allowSubstitutes = false;
      }
      ''
        set -eu

        root="$PWD/root"
        mkdir -p \
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
        chmod 0555 "$root/nix/store"
        install -m 0555 ${mkNixClosureEarlyInit}/sbin/init "$root/sbin/init"

        {
          printf 'schema=nixome.nix-closure-root.v1\n'
          printf 'contains_store=false\n'
          printf 'init=/sbin/init\n'
          printf 'store_mountpoint=/nix/store\n'
          printf 'store_device_default=/dev/xvde\n'
        } > "$out/MANAGED_BY_NIXOME"

        (cd "$root" && find . -mindepth 1 -print | sort) > "$out/root-files.txt"

        truncate -s ${toString rootImageSizeMiB}M "$out/root.img"
        mkfs.ext4 -q -F -L nixroot -d "$root" "$out/root.img"

        cat > "$out/manifest.json" <<EOF
        {"schema":"nixome.nix-closure-root-image.v1","name":"${imageName}","image":"root.img","contains_store":false,"init":"/sbin/init","store_mountpoint":"/nix/store","store_device_default":"/dev/xvde"}
        EOF
      '';

  mkNixStoreClosureImage =
    {
      name,
      rootPaths,
      additionalSpaceMiB ? 256,
      device ? "/dev/xvde",
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
          pkgs.findutils
          pkgs.gawk
        ];
        preferLocalBuild = true;
        allowSubstitutes = false;
      }
      ''
        set -eu

        root="$PWD/root"
        mkdir -p "$root" "$out"

        while IFS= read -r store_path; do
          store_entry="''${store_path#/nix/store/}"
          case "$store_entry" in
            ""|*/*)
              printf 'unsupported Nix store path in closure: %s\n' "$store_path" >&2
              exit 1
              ;;
          esac
          cp -a "$store_path" "$root/$store_entry"
        done < ${closure}/store-paths

        used_apparent_bytes="$(du -sb "$root" | awk '{ print $1 }')"
        used_bytes="$(du -sB1 "$root" | awk '{ print $1 }')"
        used_inodes="$(find "$root" -mindepth 1 -printf x | wc -c)"
        requested_spare_bytes="$((${toString additionalSpaceMiB} * 1024 * 1024))"
        inode_spare="$((used_inodes / 2 + 8192))"
        inode_count="$((used_inodes + inode_spare))"
        metadata_spare_bytes="$((used_bytes / 2 + 512 * 1024 * 1024 + inode_count * 512))"
        if [ "$requested_spare_bytes" -gt "$metadata_spare_bytes" ]; then
          spare_bytes="$requested_spare_bytes"
        else
          spare_bytes="$metadata_spare_bytes"
        fi
        image_bytes="$((used_bytes + spare_bytes))"
        image_bytes="$((((image_bytes + 1048575) / 1048576) * 1048576))"
        truncate -s "$image_bytes" "$out/store.img"
        mkfs.ext4 -q -F -m 0 -L nixstore -N "$inode_count" -d "$root" "$out/store.img"

        cp ${closure}/store-paths "$out/store-paths"
        {
          printf 'used_apparent_bytes=%s\n' "$used_apparent_bytes"
          printf 'used_bytes=%s\n' "$used_bytes"
          printf 'used_inodes=%s\n' "$used_inodes"
          printf 'requested_spare_bytes=%s\n' "$requested_spare_bytes"
          printf 'metadata_spare_bytes=%s\n' "$metadata_spare_bytes"
          printf 'spare_bytes=%s\n' "$spare_bytes"
          printf 'inode_spare=%s\n' "$inode_spare"
          printf 'inode_count=%s\n' "$inode_count"
          printf 'image_bytes=%s\n' "$image_bytes"
        } > "$out/image-sizing.txt"
        cat > "$out/manifest.json" <<EOF
        {"schema":"nixome.store-closure-image.v1","name":"${name}","image":"store.img","mountpoint":"/nix/store","device":"${device}","read_only":true,"sizing":"allocated-used-plus-metadata-spare","inode_sizing":"used-plus-50-percent-and-floor"}
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
            schema: "nixome.store-closure-manifest.v1",
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
      nixomeConfigSeedInputs ? null,
      nixomePrebuiltTemplateSources ? { },
      extraSpecialArgs ? { },
      additionalRootSpace ? null,
      additionalStoreSpaceMiB ? 256,
      configurationName ? "nixome-domain-${builtins.replaceStrings [ "." ] [ "-" ] name}",
      rootImageName ? "${configurationName}-root-image",
      storeMode ? "shared-closure",
      storeArtifactName ? "${configurationName}-${
        if storeMode == "closure-image" then "store-image" else "store-closure"
      }",
      kernelName ? "6.12.64-1.fc41",
      label ? "green",
      netvm ? null,
      prefs ? { },
      features ? { },
      services ? { },
      rootImageSizeMiB ? 512,
      storeDevice ? "/dev/xvde",
    }:
    let
      configuration = lib.nixosSystem {
        inherit system;
        specialArgs = {
          inherit
            nixomeConfigSeedInputs
            nixomePrebuiltTemplateSources
            ;
        }
        // extraSpecialArgs;
        modules = [
          "${nixpkgs}/nixos/modules/profiles/minimal.nix"
          "${nixosQubesSource}/nixos/modules/virtualisation/qubes"
          "${nixomeSupport}/modules/qube-guest.nix"
          {
            nixpkgs.pkgs = pkgs;
            nixome.qubeGuest.enable = true;
            nixome.qubeGuest.role = guestRole;
            documentation.enable = lib.mkDefault false;
            programs.command-not-found.enable = lib.mkDefault false;
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
      _storeModeCheck =
        if
          builtins.elem storeMode [
            "closure-image"
            "shared-closure"
          ]
        then
          null
        else
          throw "mkNixNativeDomain.storeMode must be closure-image or shared-closure";
      rootImage = mkNixClosureRootImage {
        imageName = rootImageName;
        rootImageSizeMiB = rootImageSizeMiBEffective;
      };
      storeArtifact =
        if _storeModeCheck == null && storeMode == "closure-image" then
          mkNixStoreClosureImage {
            name = storeArtifactName;
            rootPaths = [ configuration.config.system.build.toplevel ];
            additionalSpaceMiB = additionalStoreSpaceMiB;
            device = storeDevice;
          }
        else
          mkNixStoreClosureManifest {
            name = storeArtifactName;
            rootPaths = [ configuration.config.system.build.toplevel ];
            device = storeDevice;
          };
      defaultPrefs = {
        autostart = false;
        default_user = "user";
        include_in_backups = true;
        kernel = kernelName;
        kernelopts = mkNixClosureKernelOpts configuration { inherit storeDevice; };
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
            mode = storeMode;
            sourceRef = {
              build = "packages.${system}.${storeArtifactName}";
              subpath = if storeMode == "closure-image" then "store.img" else "";
            };
            mountpoint = "/nix/store";
            device = storeDevice;
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
        storeArtifact
        storeArtifactName
        ;
      storeImage = storeArtifact;
      storeImageName = storeArtifactName;
      kernelopts = mkNixClosureKernelOpts configuration { inherit storeDevice; };
      configurationName = configurationName;
    };
in
{
  inherit
    mkNixNativeDomain
    mkNixClosureKernelOpts
    mkNixClosureRootImage
    mkNixStoreClosureImage
    mkNixStoreClosureManifest
    mkQubeKernelOpts
    pkgs
    qubesOverlays
    ;
}
