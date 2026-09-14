{
  config,
  lib,
  airlockConfigInputs ? null,
  airlockPrebuiltTemplateSources ? { },
  pkgs,
  ...
}:
let
  cfg = config.airlock.qubeGuest;
  hasConfigVaultRole = cfg.role == "sys-config-vault";
  hasConfigStagingRole = cfg.role == "sys-config-staging";
  hasConfigValidatorRole = cfg.role == "sys-config-validator";
  hasCmdRole = cfg.role == "cmd";
  hasSysNetRole = cfg.role == "sys-net";
  hasSysGuiRole = cfg.role == "sys-gui";
  hasNetworkProviderRole = hasSysNetRole || cfg.role == "sys-firewall";
  hasConfigRole = hasConfigVaultRole || hasConfigStagingRole || hasConfigValidatorRole;
  hasPrivateNixRole =
    hasConfigVaultRole || hasConfigStagingRole || hasConfigValidatorRole || hasCmdRole;
  hasConfigInputsImportRole = hasConfigStagingRole || hasConfigValidatorRole;
  qubesAgentLinux = pkgs.qubes-core-agent-linux;
  qubesDbPython =
    (pkgs.qubes-core-qubesdb.override {
      withPython = true;
      python = pkgs.python3;
    }).pythonModule;
  qubesPython = pkgs.python3.withPackages (ps: [
    ps.dbus-python
    qubesDbPython
  ]);
  qubesDnatToNs = pkgs.writeShellScript "airlock-qubes-setup-dnat-to-ns" ''
    exec ${qubesPython}/bin/python ${qubesAgentLinux}/lib/qubes/qubes-setup-dnat-to-ns "$@"
  '';
  qubesNetworkProxySetup = pkgs.runCommand "airlock-qubes-network-proxy-setup" { } ''
    install -D -m 0555 ${qubesAgentLinux}/lib/qubes/init/network-proxy-setup.sh "$out/bin/network-proxy-setup"
    substituteInPlace "$out/bin/network-proxy-setup" \
      --replace-fail "${qubesAgentLinux}/lib/qubes/qubes-setup-dnat-to-ns" "${qubesDnatToNs}"
  '';
  roleGit = pkgs.gitMinimal;
  airlockCli = pkgs.callPackage ../../runtime/tools/pkgs/airlock-cli { };
  systemdPackage = config.systemd.package;
  configInputsPublicPath = "/etc/airlock/config-inputs";
  cmdPersistentPaths = lib.unique (cfg.cmdPersistentPaths ++ [ cfg.codexHome ]);

  qubesGuestPath = [
    pkgs.coreutils
    pkgs.diffutils
    pkgs.e2fsprogs
    pkgs.findutils
    pkgs.gawk
    pkgs.glibc.bin
    pkgs.gnugrep
    pkgs.kmod
    pkgs.qubes-core-qubesdb
    pkgs.qubes-linux-utils
    systemdPackage
    pkgs.util-linux
  ];

  guestDiagnosticsPath = [
    pkgs.coreutils
    pkgs.findutils
    pkgs.gnugrep
    pkgs.kmod
    pkgs.qubes-core-qubesdb
    systemdPackage
    pkgs.util-linux
  ];

  networkGuestPath = qubesGuestPath ++ networkProviderPackages;
  networkProviderPackages = [
    pkgs.conntrack-tools
    pkgs.iproute2
    pkgs.iptables
    pkgs.networkmanager
    pkgs.nftables
    pkgs.procps
    pkgs.qubes-vmm-xen
    pkgs.tinyproxy
  ];

  xenDriverDomainEtc = pkgs.symlinkJoin {
    name = "airlock-xen-driver-domain-etc";
    paths = [
      pkgs.qubes-vmm-xen
      qubesAgentLinux
      pkgs.qubes-linux-utils
    ];
    stripPrefix = "/etc/xen";
  };

  updatesProxyRpcService = pkgs.writeShellScript "airlock-qubes-updates-proxy-rpc-service" ''
    set -euo pipefail
    exec ${pkgs.socat}/bin/socat STDIO TCP:127.0.0.1:8082
  '';
  codexAuthImportService = pkgs.writeShellScript "airlock-codex-auth-import" ''
    set -euo pipefail

    codex_home=${lib.escapeShellArg cfg.codexHome}
    temp="$(mktemp "''${TMPDIR:-/tmp}/airlock-codex-auth.XXXXXX")"
    trap 'rm -f "$temp"' EXIT
    ${pkgs.coreutils}/bin/cat > "$temp"
    ${pkgs.jq}/bin/jq -e type "$temp" >/dev/null
    ${pkgs.coreutils}/bin/install -d -m 0700 "$codex_home"
    ${pkgs.coreutils}/bin/install -m 0600 "$temp" "$codex_home/auth.json"
  '';
  codexConfigImportService = pkgs.writeShellScript "airlock-codex-config-import" ''
    set -euo pipefail

    codex_home=${lib.escapeShellArg cfg.codexHome}
    temp="$(mktemp "''${TMPDIR:-/tmp}/airlock-codex-config.XXXXXX")"
    trap 'rm -f "$temp"' EXIT
    ${pkgs.coreutils}/bin/cat > "$temp"
    ${pkgs.coreutils}/bin/install -d -m 0700 "$codex_home"
    ${pkgs.coreutils}/bin/install -m 0600 "$temp" "$codex_home/config.toml"
  '';

  tinyproxyUpdatesConfig = pkgs.writeText "tinyproxy-updates.conf" ''
    User tinyproxy
    Group tinyproxy
    Port 8082
    Timeout 60
    DefaultErrorFile "${pkgs.tinyproxy}/share/tinyproxy/default.html"
    StatFile "${pkgs.tinyproxy}/share/tinyproxy/stats.html"
    Syslog On
    LogLevel Notice
    PidFile "/run/tinyproxy-updates/tinyproxy.pid"

    MaxClients 50
    DisableViaHeader Yes

    Allow 127.0.0.1
    Allow 10.137.0.0/16

    ConnectPort 443
    ConnectPort 873

    Filter "${qubesAgentLinux}/etc/tinyproxy/updates-blacklist"
  '';

  qubesSetupIp = pkgs.runCommand "airlock-qubes-setup-ip" { } ''
    install -D -m 0555 ${qubesAgentLinux.src}/network/setup-ip "$out/bin/setup-ip"
    substituteInPlace "$out/bin/setup-ip" \
      --replace-fail "/usr/lib/qubes/init/functions" "${qubesAgentLinux}/lib/qubes/init/functions" \
      --replace-fail "/usr/lib/qubes/qubes-setup-dnat-to-ns" "${qubesDnatToNs}" \
      --replace-fail "/usr/bin/qubesdb-read" "${pkgs.qubes-core-qubesdb}/bin/qubesdb-read" \
      --replace-fail "/usr/bin/resolvectl" "${systemdPackage}/bin/resolvectl" \
      --replace-fail "/usr/bin/nmcli" "${pkgs.networkmanager}/bin/nmcli" \
      --replace-fail "/lib/systemd/systemd-sysctl" "${systemdPackage}/lib/systemd/systemd-sysctl"
    patchShebangs "$out/bin/setup-ip"
  '';

  qubesIptablesStart = pkgs.writeShellScript "airlock-qubes-iptables-start" ''
    set -eu

    apply_rules() {
      family="$1"
      rules="${qubesAgentLinux}/etc/qubes/qubes-$family.nft"

      if [ "$family" = ipv6 ] &&
        ! qubesdb-read /qubes-ip6 >/dev/null 2>&1 &&
        ! qubesdb-read /qubes-netvm-gateway6 >/dev/null 2>&1; then
        rules="${qubesAgentLinux}/etc/qubes/qubes-ipv6-disabled.nft"
      fi

      [ -f "$rules" ] || return 6
      nft -f "$rules"
    }

    apply_rules ipv4
    apply_rules ipv6
  '';

  qubesNetworkManagerHook = pkgs.writeShellScript "airlock-qubes-nmhook" ''
    export PATH=${lib.escapeShellArg (lib.makeBinPath networkGuestPath)}:$PATH

    case "''${2:-}" in
      up|vpn-up|vpn-down|dhcp4-change)
        ${qubesDnatToNs}
        rc="$?"
        if [ "$rc" -ne 100 ]; then
          systemctl --no-block try-restart qubes-updates-proxy.service
        fi
        ;;
    esac

    exit 0
  '';

  qubesExternalIpHook = pkgs.writeShellScript "airlock-qubes-external-ip" ''
    case "''${2:-}" in
      up)
        inet="$(${pkgs.iproute2}/bin/ip addr show dev "$1" | ${pkgs.gnugrep}/bin/grep inet || true)"
        ${pkgs.qubes-core-qubesdb}/bin/qubesdb-write /qubes-netvm-external-ip "$inet"
        ;;
      down)
        ${pkgs.qubes-core-qubesdb}/bin/qubesdb-write /qubes-netvm-external-ip ""
        ;;
    esac
  '';

  roleInitPathFor =
    role:
    lib.makeBinPath (
      [
        pkgs.coreutils
        pkgs.gawk
        roleGit
        pkgs.gnutar
        pkgs.jq
        pkgs.qubes-core-qubesdb
        pkgs.util-linux
      ]
      ++
        lib.optionals
          (
            role == "sys-config-vault"
            || role == "sys-config-staging"
            || role == "sys-config-validator"
            || role == "cmd"
          )
          [
            configNix
            pkgs.nix
          ]
    );

  rolePackages = {
    base = [ ];
    sys-net = networkProviderPackages;
    sys-firewall = networkProviderPackages;
    sys-config-vault = [
      configVaultInit
      configVaultArchiveCandidate
      configVaultArchiveDeployment
      configVaultExportCurrent
      (lib.hiPrio configNix)
      pkgs.gawk
      roleGit
      pkgs.gnutar
      pkgs.jq
    ];
    sys-config-staging = [
      configStagingExportCandidate
      configStagingImportCurrent
      configStagingInit
      configStagingValidate
      (lib.hiPrio configNix)
      pkgs.gawk
      roleGit
      pkgs.gnutar
      pkgs.jq
    ];
    sys-config-validator = [
      configValidatorValidateCandidate
      configValidatorInit
      (lib.hiPrio configNix)
      pkgs.gawk
      roleGit
      pkgs.gnutar
      pkgs.jq
    ];
    cmd = [
      (lib.hiPrio configNix)
      airlockCli
      pkgs.bashInteractive
      pkgs.cacert
      pkgs.cargo
      pkgs.clippy
      pkgs.codex
      pkgs.diffutils
      pkgs.fd
      pkgs.file
      pkgs.findutils
      pkgs.gawk
      pkgs.git
      pkgs.gnugrep
      pkgs.gnumake
      pkgs.gnused
      pkgs.gnutar
      pkgs.gzip
      pkgs.jq
      pkgs.less
      pkgs.nixfmt
      pkgs.openssh
      pkgs.procps
      pkgs.python3
      pkgs.ripgrep
      pkgs.rustc
      pkgs.rustfmt
      pkgs.uv
      pkgs.vim
      pkgs.which
      pkgs.xz
      pkgs.zellij
    ];
  };

  initialRuntimeConfigFlake = builtins.readFile ../../bootstrap/initial-runtime-config/flake.nix;
  initialRuntimeConfigVms = builtins.readFile ../../bootstrap/initial-runtime-config/airlock/config-vms.nix;
  initialRuntimeDom0Host = builtins.readFile ../../bootstrap/initial-runtime-config/airlock/dom0-host.nix;
  initialRuntimeDom0ProfileActivate = builtins.readFile ../../bootstrap/initial-runtime-config/dom0-profile/bin/airlock-dom0-profile-activate;
  initialRuntimeDom0ProfileJson = builtins.readFile ../../bootstrap/initial-runtime-config/dom0-profile/share/airlock/profile.json;
  initialRuntimeLocal = builtins.readFile ../../bootstrap/initial-runtime-config/airlock/local.nix;
  initialRuntimeConfig = builtins.readFile ../../bootstrap/initial-runtime-config/airlock/runtime.nix;
  initialRuntimeSystem = builtins.readFile ../../bootstrap/initial-runtime-config/airlock/system.nix;
  initialRuntimeSystemSchema = builtins.readFile ../lib/system.nix;
  initialRuntimeTopologySchema = builtins.readFile ../lib/topology.nix;
  initialRuntimePrebuiltTemplateSources = lib.generators.toPretty { } airlockPrebuiltTemplateSources;
  initialRuntimeConfigRepoPaths = [
    "README.md"
    "dom0-profile/bin/airlock-dom0-profile-activate"
    "dom0-profile/share/airlock/profile.json"
    "flake.nix"
    "airlock/config-vms.nix"
    "airlock/dom0-host.nix"
    "airlock/local.nix"
    "airlock/runtime.nix"
    "airlock/prebuilt-template-sources.nix"
    "airlock/system.nix"
    "airlock/system-schema.nix"
    "airlock/topology.nix"
  ];
  initialRuntimeConfigRepoGitAddArgs = lib.escapeShellArgs initialRuntimeConfigRepoPaths;
  initialRuntimeConfigRepoFilesScript = ''
    write_initial_runtime_config_repo_files() {
      mkdir -p dom0-profile/bin dom0-profile/share/airlock airlock
      cat > README.md <<'EOF'
    # Airlock Runtime Configuration

    This is the editable Airlock configuration candidate. dom0 archives a
    validated commit into sys-config-vault before attempting deployment.
    EOF
      cat > flake.nix <<'EOF'
    ${initialRuntimeConfigFlake}
    EOF
      cat > airlock/topology.nix <<'EOF'
    ${initialRuntimeTopologySchema}
    EOF
      cat > airlock/system-schema.nix <<'EOF'
    ${initialRuntimeSystemSchema}
    EOF
      cat > airlock/config-vms.nix <<'EOF'
    ${initialRuntimeConfigVms}
    EOF
      cat > airlock/dom0-host.nix <<'EOF'
    ${initialRuntimeDom0Host}
    EOF
      cat > airlock/local.nix <<'EOF'
    ${initialRuntimeLocal}
    EOF
      cat > airlock/runtime.nix <<'EOF'
    ${initialRuntimeConfig}
    EOF
      cat > airlock/system.nix <<'EOF'
    ${initialRuntimeSystem}
    EOF
      cat > airlock/prebuilt-template-sources.nix <<'EOF'
    ${initialRuntimePrebuiltTemplateSources}
    EOF
      cat > dom0-profile/bin/airlock-dom0-profile-activate <<'EOF'
    ${initialRuntimeDom0ProfileActivate}
    EOF
      cat > dom0-profile/share/airlock/profile.json <<'EOF'
    ${initialRuntimeDom0ProfileJson}
    EOF
      chmod 0755 dom0-profile/bin/airlock-dom0-profile-activate
    }
  '';
  configInputPathSubstitutionScript = ''
    substitute_config_inputs() {
      config_inputs_root="$1"
      ${pkgs.gnused}/bin/sed -i \
        -e "s|__AIRLOCK_NIXPKGS_URL__|path:$config_inputs_root/nixpkgs|g" \
        -e "s|__NIXPKGS_LIBVIRT_10_5_PATH__|$config_inputs_root/nixpkgs-libvirt_10_5|g" \
        -e "s|__AIRLOCK_NIXOS_QUBES_PATH__|$config_inputs_root/nixos-qubes|g" \
        -e "s|__AIRLOCK_SUPPORT_PATH__|$config_inputs_root/airlock-support|g" \
        flake.nix
      for old_config_inputs_root in \
        /rw/airlock/config-inputs \
        /rw/home/${lib.escapeShellArg cfg.user}/.local/share/airlock/config-inputs \
        ${lib.escapeShellArg cfg.configInputsPath}; do
        ${pkgs.gnused}/bin/sed -i \
          -e "s|path:$old_config_inputs_root/nixpkgs|path:$config_inputs_root/nixpkgs|g" \
          -e "s|$old_config_inputs_root/nixpkgs-libvirt_10_5|$config_inputs_root/nixpkgs-libvirt_10_5|g" \
          -e "s|$old_config_inputs_root/nixos-qubes|$config_inputs_root/nixos-qubes|g" \
          -e "s|$old_config_inputs_root/airlock-support|$config_inputs_root/airlock-support|g" \
          flake.nix
      done
    }
  '';
  configGuestInitCommand =
    if hasConfigVaultRole then
      "${configVaultInit}/bin/airlock-config-vault-init"
    else if hasConfigStagingRole then
      "${configStagingInit}/bin/airlock-config-staging-init"
    else if hasConfigValidatorRole then
      "${configValidatorInit}/bin/airlock-config-validator-init"
    else
      "${pkgs.coreutils}/bin/true";
  configNixWrapper = pkgs.writeShellScript "airlock-config-nix" ''
    set -eu

    for arg in "$@"; do
      case "$arg" in
        --store|--store=*)
          exec ${pkgs.nix}/bin/nix "$@"
          ;;
      esac
    done

    store_root="''${AIRLOCK_CONFIG_NIX_STORE_ROOT:-${cfg.configNixStoreRoot}}"
    case "$store_root" in
      /*) ;;
      *)
        printf 'AIRLOCK_CONFIG_NIX_STORE_ROOT must be absolute: %s\n' "$store_root" >&2
        exit 1
        ;;
    esac

    mkdir -p "$store_root"
    exec ${pkgs.nix}/bin/nix --store "$store_root" "$@"
  '';
  configNix = pkgs.runCommand "airlock-config-nix" { } ''
    mkdir -p "$out/bin"
    ln -s ${configNixWrapper} "$out/bin/airlock-config-nix"
    ln -s ${configNixWrapper} "$out/bin/nix"
  '';

  configVaultInit = pkgs.writeShellScriptBin "airlock-config-vault-init" ''
        set -eu

        repo="''${AIRLOCK_CONFIG_ARCHIVE_PATH:-${cfg.configArchivePath}}"
        config_inputs_root="''${AIRLOCK_CONFIG_INPUTS_PATH:-${configInputsPublicPath}}"
        tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
        trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

        ${initialRuntimeConfigRepoFilesScript}
        ${configInputPathSubstitutionScript}

        config_inputs_ready() {
          [ -d "$config_inputs_root/nixpkgs" ] &&
            [ -d "$config_inputs_root/nixpkgs-libvirt_10_5" ] &&
            [ -d "$config_inputs_root/nixos-qubes" ] &&
            [ -d "$config_inputs_root/airlock-support" ]
        }

        write_archive_readme() {
          cat > README.md <<'EOF'
    # Airlock Configuration Vault

    This repository archives Airlock configuration baselines, candidates, and
    deployment records written by dom0.

    The `current` link points at the latest config snapshot that should be used
    to initialize an editable sys-config-staging worktree.
    EOF
        }

        commit_archive_if_needed() {
          message="$1"
          if ! ${roleGit}/bin/git diff --cached --quiet; then
            ${roleGit}/bin/git \
              -c user.name='Airlock Config Vault' \
              -c user.email='airlock-config-vault@localhost' \
              commit -m "$message"
          fi
        }

        ensure_archive_repo() {
          mkdir -p "$repo"
          cd "$repo"
          if [ ! -d .git ]; then
            ${roleGit}/bin/git init -b main
          fi
          mkdir -p candidates deployments
          : > candidates/.gitkeep
          : > deployments/.gitkeep
          write_archive_readme
          ${roleGit}/bin/git add README.md candidates/.gitkeep deployments/.gitkeep
        }

        set_current_link() {
          candidate_commit="$1"
          [ -d "candidates/$candidate_commit/snapshot" ] || {
            printf 'current candidate snapshot is missing: %s\n' "$candidate_commit" >&2
            exit 1
          }
          printf '%s\n' "$candidate_commit" > latest-current
          ${pkgs.coreutils}/bin/ln -sfn "candidates/$candidate_commit/snapshot" current
          ${roleGit}/bin/git add latest-current current
        }

        set_current_from_latest_deployment() {
          [ -f latest-deployment ] || return 1
          deployment_id="$(${pkgs.coreutils}/bin/head -n 1 latest-deployment)"
          [ -n "$deployment_id" ] || return 1
          [ -f "deployments/$deployment_id/deployment.json" ] || return 1
          candidate_commit="$(${pkgs.jq}/bin/jq -r '.candidate_commit // ""' "deployments/$deployment_id/deployment.json")"
          [ -n "$candidate_commit" ] && [ "$candidate_commit" != null ] || return 1
          set_current_link "$candidate_commit"
          commit_archive_if_needed "record current config from deployment $deployment_id"
          return 0
        }

        create_initial_current() {
          initial_repo="$tmpdir/initial-config"
          ${pkgs.coreutils}/bin/mkdir -p "$initial_repo"
          cd "$initial_repo"
          write_initial_runtime_config_repo_files
          substitute_config_inputs "$config_inputs_root"
          ${roleGit}/bin/git init -b main
          ${roleGit}/bin/git add ${initialRuntimeConfigRepoGitAddArgs}
          if config_inputs_ready; then
            ${configNix}/bin/airlock-config-nix \
              --extra-experimental-features nix-command \
              --extra-experimental-features flakes \
              flake lock \
              --impure \
              --offline
          else
            printf 'config vault config inputs are not present yet under %s; flake.lock deferred\n' "$config_inputs_root"
          fi
          if [ -f flake.lock ]; then
            ${roleGit}/bin/git add flake.lock
          fi
          ${roleGit}/bin/git \
            -c user.name='Airlock Initial Runtime Config' \
            -c user.email='airlock-initial-runtime-config@localhost' \
            commit -m 'init airlock config candidate'

          candidate_commit="$(${roleGit}/bin/git rev-parse HEAD)"
          archive_dir="$repo/candidates/$candidate_commit"
          snapshot_dir="$archive_dir/snapshot"
          ${pkgs.coreutils}/bin/mkdir -p "$archive_dir" "$snapshot_dir"
          ${roleGit}/bin/git bundle create "$archive_dir/config.bundle" HEAD refs/heads/main >/dev/null
          ${pkgs.jq}/bin/jq -c -n \
            --arg schema "airlock.config-candidate.v1" \
            --arg commit "$candidate_commit" \
            --arg branch "main" \
            --arg source_role "sys-config-vault" \
            --arg source "initial-runtime-config" \
            '{
              schema: $schema,
              commit: $commit,
              branch: $branch,
              source_role: $source_role,
              source: $source
            }' > "$archive_dir/manifest.json"
          ${roleGit}/bin/git archive --format=tar HEAD |
            ${pkgs.gnutar}/bin/tar -C "$snapshot_dir" -xf -
          ${pkgs.jq}/bin/jq -c -n \
            --arg schema "airlock.config-vault-current.v1" \
            --arg status "current" \
            --arg candidate_commit "$candidate_commit" \
            --arg branch "main" \
            --arg source "initial-runtime-config" \
            --arg snapshot "$archive_dir/snapshot" \
            --arg bundle "$archive_dir/config.bundle" \
            '{
              schema: $schema,
              status: $status,
              candidate_commit: $candidate_commit,
              branch: $branch,
              source: $source,
              snapshot: $snapshot,
              bundle: $bundle
            }' > "$archive_dir/metadata.json"

          cd "$repo"
          printf '%s\n' "$candidate_commit" > latest-candidate
          set_current_link "$candidate_commit"
          ${roleGit}/bin/git add "candidates/$candidate_commit" latest-candidate
          commit_archive_if_needed "initialize current config baseline ''${candidate_commit:0:12}"
        }

        ensure_archive_repo
        if [ -f latest-current ]; then
          current_commit="$(${pkgs.coreutils}/bin/head -n 1 latest-current)"
          if [ -n "$current_commit" ] && [ -d "candidates/$current_commit/snapshot" ]; then
            set_current_link "$current_commit"
            commit_archive_if_needed "repair current config pointer"
            printf 'config archive current exists: %s %s\n' "$repo" "$current_commit"
            exit 0
          fi
        fi
        if set_current_from_latest_deployment; then
          printf 'config archive current restored from latest deployment: %s\n' "$repo"
          exit 0
        fi
        create_initial_current
        printf 'config archive current initialized: %s\n' "$repo"
  '';

  configVaultExportCurrent = pkgs.writeShellScriptBin "airlock-config-vault-export-current" ''
    set -eu

    repo="''${AIRLOCK_CONFIG_ARCHIVE_PATH:-${cfg.configArchivePath}}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    ${configVaultInit}/bin/airlock-config-vault-init >/dev/null

    cd "$repo"
    candidate_commit=""
    if [ -f latest-deployment ]; then
      deployment_id="$(${pkgs.coreutils}/bin/head -n 1 latest-deployment)"
      if [ -n "$deployment_id" ] && [ -f "deployments/$deployment_id/deployment.json" ]; then
        candidate_commit="$(${pkgs.jq}/bin/jq -r '.candidate_commit // ""' "deployments/$deployment_id/deployment.json")"
      fi
    fi
    if { [ -z "$candidate_commit" ] || [ "$candidate_commit" = null ]; } && [ -f latest-current ]; then
      candidate_commit="$(${pkgs.coreutils}/bin/head -n 1 latest-current)"
    fi
    if { [ -z "$candidate_commit" ] || [ "$candidate_commit" = null ]; } && [ -f latest-candidate ]; then
      candidate_commit="$(${pkgs.coreutils}/bin/head -n 1 latest-candidate)"
    fi

    case "$candidate_commit" in
      ""|*[!0-9a-f]*)
        printf 'invalid current config commit: %s\n' "$candidate_commit" >&2
        exit 1
        ;;
    esac
    [ -f "candidates/$candidate_commit/config.bundle" ] || {
      printf 'current config bundle is missing: %s\n' "$candidate_commit" >&2
      exit 1
    }
    [ -f "candidates/$candidate_commit/manifest.json" ] || {
      printf 'current config manifest is missing: %s\n' "$candidate_commit" >&2
      exit 1
    }
    ${pkgs.jq}/bin/jq -e \
      --arg commit "$candidate_commit" \
      '.schema == "airlock.config-candidate.v1" and .commit == $commit' \
      "candidates/$candidate_commit/manifest.json" >/dev/null

    ${pkgs.coreutils}/bin/cp "candidates/$candidate_commit/config.bundle" "$tmpdir/config.bundle"
    ${pkgs.coreutils}/bin/cp "candidates/$candidate_commit/manifest.json" "$tmpdir/manifest.json"
    exec ${pkgs.gnutar}/bin/tar -C "$tmpdir" -cf - config.bundle manifest.json
  '';

  configStagingImportCurrent = pkgs.writeShellScriptBin "airlock-config-staging-import-current" ''
    set -eu

    repo="''${AIRLOCK_CONFIG_REPO_PATH:-${cfg.configRepoPath}}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    current_tar="$tmpdir/current.tar"
    ${pkgs.coreutils}/bin/cat > "$current_tar"
    ${pkgs.gnutar}/bin/tar -tf "$current_tar" |
      ${pkgs.gawk}/bin/awk '
        $0 != "config.bundle" && $0 != "manifest.json" {
          printf "unexpected current config tar member: %s\n", $0 > "/dev/stderr"
          exit 1
        }
      '
    ${pkgs.gnutar}/bin/tar -C "$tmpdir" -xf "$current_tar"

    [ -f "$tmpdir/config.bundle" ] || {
      printf 'current config tar is missing config.bundle\n' >&2
      exit 1
    }
    [ -f "$tmpdir/manifest.json" ] || {
      printf 'current config tar is missing manifest.json\n' >&2
      exit 1
    }

    manifest_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/manifest.json")"
    candidate_commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/manifest.json")"
    branch="$(${pkgs.jq}/bin/jq -r '.branch // "main"' "$tmpdir/manifest.json")"
    [ "$manifest_schema" = "airlock.config-candidate.v1" ] || {
      printf 'unsupported current config manifest schema: %s\n' "$manifest_schema" >&2
      exit 1
    }
    case "$candidate_commit" in
      ""|*[!0-9a-f]*)
        printf 'invalid current config commit: %s\n' "$candidate_commit" >&2
        exit 1
        ;;
    esac
    case "$branch" in
      ""|*[!A-Za-z0-9._/-]*)
        printf 'invalid current config branch: %s\n' "$branch" >&2
        exit 1
        ;;
    esac

    verify_repo="$tmpdir/verify.git"
    ${roleGit}/bin/git init --bare -b main "$verify_repo" >/dev/null
    ${roleGit}/bin/git -C "$verify_repo" bundle verify "$tmpdir/config.bundle" > "$tmpdir/bundle-verify.log" 2>&1
    bundle_commit="$(
      ${roleGit}/bin/git -C "$verify_repo" bundle list-heads "$tmpdir/config.bundle" |
        ${pkgs.gawk}/bin/awk 'NR == 1 { print $1 }'
    )"
    [ "$bundle_commit" = "$candidate_commit" ] || {
      printf 'current config bundle commit %s does not match manifest commit %s\n' "$bundle_commit" "$candidate_commit" >&2
      exit 1
    }

    import_status=imported
    reason="staging initialized from sys-config-vault current"
    current_commit=""
    dirty=""

    clone_current_into_repo() {
      parent="''${repo%/*}"
      ${pkgs.coreutils}/bin/mkdir -p "$parent"
      clone_tmp="$tmpdir/staging-clone"
      ${roleGit}/bin/git clone "$tmpdir/config.bundle" "$clone_tmp" > "$tmpdir/git-clone.log" 2>&1
      ${roleGit}/bin/git -C "$clone_tmp" checkout -B "$branch" "$candidate_commit" > "$tmpdir/git-checkout.log" 2>&1
      cd "$tmpdir"
      ${pkgs.coreutils}/bin/rm -rf "$repo"
      ${pkgs.coreutils}/bin/mv "$clone_tmp" "$repo"
      current_commit="$candidate_commit"
    }

    if [ -d "$repo/.git" ]; then
      cd "$repo"
      dirty="$(${roleGit}/bin/git status --porcelain)"
      if ! current_commit="$(${roleGit}/bin/git rev-parse HEAD 2>/dev/null)"; then
        if [ -n "$dirty" ]; then
          import_status=skipped
          reason="staging empty Git repo is dirty; refusing to overwrite"
        else
          clone_current_into_repo
        fi
      elif [ "$current_commit" = "$candidate_commit" ] && [ -z "$dirty" ]; then
        import_status=current
        reason="staging already matches sys-config-vault current"
      elif [ -n "$dirty" ]; then
        import_status=skipped
        reason="staging worktree is dirty; refusing to overwrite"
      else
        import_status=skipped
        reason="staging already has a different committed worktree"
      fi
    elif [ -e "$repo" ] && [ -n "$(${pkgs.findutils}/bin/find "$repo" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null || true)" ]; then
      import_status=skipped
      reason="staging path exists and is not an empty Git repository"
    else
      clone_current_into_repo
    fi

    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "airlock.config-staging-import-current.v1" \
      --arg status "passed" \
      --arg import_status "$import_status" \
      --arg reason "$reason" \
      --arg repo "$repo" \
      --arg current_commit "$current_commit" \
      --arg candidate_commit "$candidate_commit" \
      --arg dirty "$dirty" \
      '{
        schema: $schema,
        status: $status,
        import_status: $import_status,
        reason: $reason,
        repo: $repo,
        current_commit: $current_commit,
        candidate_commit: $candidate_commit,
        dirty: $dirty
      }'
  '';

  configCurrentExportService = pkgs.writeShellScript "airlock-config-current-export-qrexec" ''
    exec ${configVaultExportCurrent}/bin/airlock-config-vault-export-current
  '';

  configCurrentImportService = pkgs.writeShellScript "airlock-config-current-import-qrexec" ''
    exec ${configStagingImportCurrent}/bin/airlock-config-staging-import-current
  '';

  configStagingInit = pkgs.writeShellScriptBin "airlock-config-staging-init" ''
    set -eu

    repo="''${AIRLOCK_CONFIG_REPO_PATH:-${cfg.configRepoPath}}"
    config_inputs_root="''${AIRLOCK_CONFIG_INPUTS_PATH:-${configInputsPublicPath}}"

    config_inputs_ready() {
      [ -d "$config_inputs_root/nixpkgs" ] &&
        [ -d "$config_inputs_root/nixpkgs-libvirt_10_5" ] &&
        [ -d "$config_inputs_root/nixos-qubes" ] &&
        [ -d "$config_inputs_root/airlock-support" ]
    }

    ${initialRuntimeConfigRepoFilesScript}
    ${configInputPathSubstitutionScript}

    config_inputs_need_refresh() {
      for input_file in flake.nix flake.lock; do
        [ -f "$input_file" ] || continue
        ${pkgs.gnugrep}/bin/grep -F '__AIRLOCK_NIXPKGS_URL__' "$input_file" >/dev/null 2>&1 && return 0
        ${pkgs.gnugrep}/bin/grep -F '/rw/airlock/config-inputs/' "$input_file" >/dev/null 2>&1 && return 0
        ${pkgs.gnugrep}/bin/grep -F '/rw/home/${lib.escapeShellArg cfg.user}/.local/share/airlock/config-inputs/' "$input_file" >/dev/null 2>&1 && return 0
        ${pkgs.gnugrep}/bin/grep -F '${cfg.configInputsPath}/' "$input_file" >/dev/null 2>&1 && return 0
      done
      return 1
    }

    require_clean_worktree_for_input_update() {
      dirty="$(${roleGit}/bin/git status --porcelain)"
      if [ -n "$dirty" ]; then
        printf 'config staging worktree is dirty; refusing to refresh config input paths\n%s\n' "$dirty" >&2
        exit 1
      fi
    }

    add_config_input_lock_if_ready() {
      if [ -f flake.lock ]; then
        printf 'config staging flake.lock already exists: %s\n' "$repo/flake.lock"
        return 0
      fi

      if ! config_inputs_ready; then
        printf 'config staging config inputs are not present yet under %s; flake.lock deferred\n' "$config_inputs_root"
        return 0
      fi

      substitute_config_inputs "$config_inputs_root"
      ${configNix}/bin/airlock-config-nix \
        --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        flake lock \
        --impure \
        --offline
      ${roleGit}/bin/git add flake.nix flake.lock
    }

    if [ -d "$repo/.git" ]; then
      cd "$repo"
      if [ "''${AIRLOCK_CONFIG_REFRESH_INPUTS:-0}" = 1 ]; then
        if ! config_inputs_ready; then
          printf 'config staging config inputs are not present yet under %s; refresh deferred\n' "$config_inputs_root"
          printf 'config staging repo already exists: %s\n' "$repo"
          exit 0
        fi
        dirty="$(${roleGit}/bin/git status --porcelain)"
        if [ -n "$dirty" ]; then
          printf 'config staging config input refresh deferred because worktree is dirty\n%s\n' "$dirty"
        else
          substitute_config_inputs "$config_inputs_root"
          ${configNix}/bin/airlock-config-nix \
            --extra-experimental-features nix-command \
            --extra-experimental-features flakes \
            flake lock \
            --recreate-lock-file \
            --impure \
            --offline
          if [ -n "$(${roleGit}/bin/git status --porcelain flake.nix flake.lock)" ]; then
            ${roleGit}/bin/git add flake.nix flake.lock
            ${roleGit}/bin/git \
              -c user.name='Airlock Config Staging' \
              -c user.email='airlock-config-staging@localhost' \
              commit -m 'refresh airlock config inputs'
          elif config_inputs_need_refresh; then
            printf 'config staging config input paths still need refresh after lock update\n' >&2
            exit 1
          else
            printf 'config staging config input lock is current\n'
          fi
        fi
      elif config_inputs_need_refresh; then
        printf 'config staging config input refresh is required but deferred for existing repo: %s\n' "$repo"
      fi
      if [ ! -f flake.lock ]; then
        require_clean_worktree_for_input_update
        add_config_input_lock_if_ready
        if ! ${roleGit}/bin/git diff --cached --quiet; then
          ${roleGit}/bin/git \
            -c user.name='Airlock Config Staging' \
            -c user.email='airlock-config-staging@localhost' \
            commit -m 'lock airlock config candidate inputs'
        fi
      fi
      printf 'config staging repo already exists: %s\n' "$repo"
      exit 0
    fi

    if [ "''${AIRLOCK_CONFIG_STAGING_INIT_FROM_INITIAL_CONFIG:-0}" != 1 ]; then
      ${pkgs.coreutils}/bin/mkdir -p "$repo"
      if [ -n "$(${pkgs.findutils}/bin/find "$repo" -mindepth 1 -maxdepth 1 -print -quit 2>/dev/null || true)" ]; then
        printf 'config staging path exists but is not a Git repo; waiting for sys-config-vault current import: %s\n' "$repo"
      else
        printf 'config staging repo is empty; waiting for sys-config-vault current import: %s\n' "$repo"
      fi
      exit 0
    fi

    mkdir -p "$repo"
    cd "$repo"
    ${roleGit}/bin/git init -b main
    write_initial_runtime_config_repo_files
    substitute_config_inputs "$config_inputs_root"
    ${roleGit}/bin/git add ${initialRuntimeConfigRepoGitAddArgs}
    add_config_input_lock_if_ready
    ${roleGit}/bin/git \
      -c user.name='Airlock Config Staging' \
      -c user.email='airlock-config-staging@localhost' \
      commit -m 'init airlock config candidate'
  '';

  configStagingValidate = pkgs.writeShellScriptBin "airlock-config-staging-validate" ''
    set -eu

    repo="''${AIRLOCK_CONFIG_REPO_PATH:-${cfg.configRepoPath}}"
    validation_dir="''${AIRLOCK_CONFIG_VALIDATION_DIR:-${cfg.configValidationPath}}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    if [ ! -d "$repo/.git" ]; then
      printf 'missing Git repo at %s; run airlock-config-staging-init first\n' "$repo" >&2
      exit 1
    fi

    mkdir -p "$validation_dir/logs"
    cd "$repo"

    commit="$(${roleGit}/bin/git rev-parse HEAD)"
    result_file="$validation_dir/$commit.json"
    result_tmp="$result_file.tmp"
    topology_json="$validation_dir/topology-$commit.json"
    system_json="$validation_dir/system-$commit.json"
    staged_tar="$tmpdir/config-candidate-staged.tar"
    validated_tar="$validation_dir/validated-candidate-$commit.tar"
    validator_stderr="$validation_dir/logs/staging-validator-$commit.stderr.log"
    dirty="$(${roleGit}/bin/git status --porcelain)"

    write_dirty_validation() {
      status="$1"
      reason="$2"

      ${pkgs.jq}/bin/jq -c -n \
        --arg schema "airlock.config-validation.v1" \
        --arg status "$status" \
        --arg reason "$reason" \
        --arg validator_role "config-staging-local" \
        --arg repo "$repo" \
        --arg commit "$commit" \
        --arg dirty "$dirty" \
        '{
          schema: $schema,
          status: $status,
          reason: $reason,
          validator_role: $validator_role,
          repo: $repo,
          commit: $commit,
          checks: [
            {
              name: "git-clean",
              status: (if $dirty == "" then "passed" else "failed" end),
              details: $dirty
            },
            {
              name: "nix-eval-airlock-topology",
              status: "skipped",
              attr: "airlock.topology"
            },
            {
              name: "nix-eval-airlock-system",
              status: "skipped",
              attr: "airlock.system"
            },
            {
              name: "nix-flake-check-no-build",
              status: "skipped"
            }
          ]
        }' > "$result_tmp"
      mv "$result_tmp" "$result_file"
      cat "$result_file"
    }

    if [ -n "$dirty" ]; then
      write_dirty_validation failed "config staging worktree is dirty; commit changes before export"
      exit 1
    fi

    if ! ${configStagingExportCandidate}/bin/airlock-config-staging-export-candidate > "$staged_tar" 2> "$validator_stderr"; then
      cat "$validator_stderr" >&2 || true
      write_dirty_validation failed "config staging export failed"
      exit 1
    fi

    if ! AIRLOCK_CONFIG_VALIDATION_DIR="$validation_dir" \
      ${configValidatorValidateCandidate}/bin/airlock-config-validator-validate-candidate \
        < "$staged_tar" \
        > "$validated_tar" \
        2> "$validator_stderr"; then
      cat "$validator_stderr" >&2 || true
      if [ -r "$result_file" ]; then
        cat "$result_file"
      fi
      exit 1
    fi

    ${pkgs.gnutar}/bin/tar -tf "$validated_tar" |
      ${pkgs.gawk}/bin/awk '
        $0 != "config.bundle" && $0 != "validation.json" && $0 != "manifest.json" && $0 != "topology.json" && $0 != "system.json" {
          printf "unexpected validated candidate tar member: %s\n", $0 > "/dev/stderr"
          exit 1
        }
      '
    ${pkgs.gnutar}/bin/tar -C "$tmpdir" -xf "$validated_tar" validation.json topology.json system.json
    ${pkgs.coreutils}/bin/cp "$tmpdir/topology.json" "$topology_json"
    ${pkgs.coreutils}/bin/cp "$tmpdir/system.json" "$system_json"

    ${pkgs.jq}/bin/jq -c \
      --arg repo "$repo" \
      --arg topology_json "$topology_json" \
      --arg system_json "$system_json" \
      --arg validated_tar "$validated_tar" \
      --arg validator_stderr "$validator_stderr" \
      '
        .repo = $repo
        | .validated_candidate = $validated_tar
        | .validator_stderr = $validator_stderr
        | .checks = (
            .checks
            | map(
                if .name == "nix-eval-airlock-topology" then
                  .output = $topology_json
                elif .name == "airlock-topology-json" then
                  .path = $topology_json
                elif .name == "nix-eval-airlock-system" then
                  .output = $system_json
                elif .name == "airlock-system-json" then
                  .path = $system_json
                else
                  .
                end
              )
          )
      ' "$tmpdir/validation.json" > "$result_tmp"
    mv "$result_tmp" "$result_file"
    cat "$result_file"
    ${pkgs.jq}/bin/jq -e '.status == "passed"' "$result_file" >/dev/null
  '';

  configStagingExportCandidate = pkgs.writeShellScriptBin "airlock-config-staging-export-candidate" ''
    set -eu

    repo="''${AIRLOCK_CONFIG_REPO_PATH:-${cfg.configRepoPath}}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    if [ ! -d "$repo/.git" ]; then
      printf 'missing Git repo at %s; run airlock-config-staging-init first\n' "$repo" >&2
      exit 1
    fi

    cd "$repo"
    dirty="$(${roleGit}/bin/git status --porcelain)"
    if [ -n "$dirty" ]; then
      printf 'config staging worktree is dirty; commit changes before export\n' >&2
      printf '%s\n' "$dirty" >&2
      exit 1
    fi

    commit="$(${roleGit}/bin/git rev-parse HEAD)"
    branch="$(${roleGit}/bin/git symbolic-ref --short HEAD)"
    ${roleGit}/bin/git bundle create "$tmpdir/config.bundle" HEAD "refs/heads/$branch" >/dev/null
    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "airlock.config-candidate.v1" \
      --arg commit "$commit" \
      --arg branch "$branch" \
      --arg source_role "sys-config-staging" \
      '{schema: $schema, commit: $commit, branch: $branch, source_role: $source_role}' \
      > "$tmpdir/manifest.json"

    exec ${pkgs.gnutar}/bin/tar -C "$tmpdir" -cf - config.bundle manifest.json
  '';

  configCandidateExportService = pkgs.writeShellScript "airlock-config-candidate-export-qrexec" ''
    exec ${configStagingExportCandidate}/bin/airlock-config-staging-export-candidate
  '';

  configValidatorInit = pkgs.writeShellScriptBin "airlock-config-validator-init" ''
    set -eu

    validation_dir="''${AIRLOCK_CONFIG_VALIDATION_DIR:-${cfg.configValidationPath}}"
    mkdir -p "$validation_dir/logs"
    printf 'config validator workspace ready: %s\n' "$validation_dir"
  '';

  configValidatorValidateCandidate = pkgs.writeShellScriptBin "airlock-config-validator-validate-candidate" ''
    set -eu

    validation_dir="''${AIRLOCK_CONFIG_VALIDATION_DIR:-${cfg.configValidationPath}}"
    run_full_flake_check="''${AIRLOCK_CONFIG_VALIDATION_BUILD:-0}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    candidate_tar="$tmpdir/candidate.tar"
    ${pkgs.coreutils}/bin/cat > "$candidate_tar"
    case "$run_full_flake_check" in
      0|1)
        ;;
      *)
        printf 'AIRLOCK_CONFIG_VALIDATION_BUILD must be 0 or 1\n' >&2
        exit 2
        ;;
    esac
    ${pkgs.gnutar}/bin/tar -tf "$candidate_tar" |
      ${pkgs.gawk}/bin/awk '
        $0 != "config.bundle" && $0 != "manifest.json" {
          printf "unexpected staged candidate tar member: %s\n", $0 > "/dev/stderr"
          exit 1
        }
      '
    ${pkgs.gnutar}/bin/tar -C "$tmpdir" -xf "$candidate_tar"

    [ -f "$tmpdir/config.bundle" ] || {
      printf 'staged candidate tar is missing config.bundle\n' >&2
      exit 1
    }
    [ -f "$tmpdir/manifest.json" ] || {
      printf 'staged candidate tar is missing manifest.json\n' >&2
      exit 1
    }

    manifest_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/manifest.json")"
    manifest_commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/manifest.json")"
    [ "$manifest_schema" = "airlock.config-candidate.v1" ] || {
      printf 'unsupported candidate manifest schema: %s\n' "$manifest_schema" >&2
      exit 1
    }
    [ -n "$manifest_commit" ] && [ "$manifest_commit" != null ] || {
      printf 'candidate manifest is missing commit\n' >&2
      exit 1
    }

    verify_repo="$tmpdir/verify.git"
    ${roleGit}/bin/git init --bare -b main "$verify_repo" >/dev/null
    ${roleGit}/bin/git -C "$verify_repo" bundle verify "$tmpdir/config.bundle" > "$tmpdir/bundle-verify.log" 2>&1
    candidate_commit="$(
      ${roleGit}/bin/git -C "$verify_repo" bundle list-heads "$tmpdir/config.bundle" |
        ${pkgs.gawk}/bin/awk 'NR == 1 { print $1 }'
    )"
    [ -n "$candidate_commit" ] || {
      printf 'candidate bundle did not expose a commit\n' >&2
      exit 1
    }
    [ "$candidate_commit" = "$manifest_commit" ] || {
      printf 'candidate commit %s does not match manifest commit %s\n' "$candidate_commit" "$manifest_commit" >&2
      exit 1
    }

    worktree="$tmpdir/worktree"
    ${roleGit}/bin/git clone "$tmpdir/config.bundle" "$worktree" > "$tmpdir/git-clone.log" 2>&1
    cd "$worktree"
    checked_commit="$(${roleGit}/bin/git rev-parse HEAD)"
    dirty="$(${roleGit}/bin/git status --porcelain)"
    [ "$checked_commit" = "$candidate_commit" ] || {
      printf 'cloned commit %s does not match candidate commit %s\n' "$checked_commit" "$candidate_commit" >&2
      exit 1
    }

    mkdir -p "$validation_dir/logs"
    result_file="$validation_dir/$candidate_commit.json"
    result_tmp="$result_file.tmp"
    flake_log="$validation_dir/logs/flake-check-$candidate_commit.log"
    flake_tests_log="$validation_dir/logs/flake-check-build-$candidate_commit.log"
    topology_eval_log="$validation_dir/logs/nix-eval-topology-$candidate_commit.log"
    topology_log="$validation_dir/logs/topology-json-$candidate_commit.log"
    template_refs_log="$validation_dir/logs/template-source-refs-$candidate_commit.log"
    system_eval_log="$validation_dir/logs/nix-eval-system-$candidate_commit.log"
    system_log="$validation_dir/logs/system-json-$candidate_commit.log"
    system_refs_log="$validation_dir/logs/system-source-refs-$candidate_commit.log"
    local_flake_log="$tmpdir/flake-check.log"
    local_flake_tests_log="$tmpdir/flake-check-build.log"
    local_topology_eval_log="$tmpdir/nix-eval-topology.log"
    local_topology_log="$tmpdir/topology-json.log"
    local_template_refs_log="$tmpdir/template-source-refs.log"
    local_system_eval_log="$tmpdir/nix-eval-system.log"
    local_system_log="$tmpdir/system-json.log"
    local_system_refs_log="$tmpdir/system-source-refs.log"
    generated_topology="$tmpdir/generated-topology.json"
    generated_system="$tmpdir/generated-system.json"

    write_validation() {
      status="$1"
      reason="$2"
      topology_eval_status="$3"
      topology_status="$4"
      topology_path="$5"
      topology_eval_log_path="$6"
      topology_log_path="$7"
      template_refs_status="$8"
      template_refs_log_path="$9"
      system_eval_status="''${10}"
      system_status="''${11}"
      system_path="''${12}"
      system_eval_log_path="''${13}"
      system_log_path="''${14}"
      system_refs_status="''${15}"
      system_refs_log_path="''${16}"
      flake_status="''${17}"
      flake_tests_status="''${18}"

      ${pkgs.jq}/bin/jq -c -n \
        --arg schema "airlock.config-validation.v1" \
        --arg status "$status" \
        --arg reason "$reason" \
        --arg validator_role "sys-config-validator" \
        --arg commit "$candidate_commit" \
        --arg dirty "$dirty" \
        --arg manifest "$tmpdir/manifest.json" \
        --arg bundle_verify_log "$tmpdir/bundle-verify.log" \
        --arg git_clone_log "$tmpdir/git-clone.log" \
        --arg topology_eval_status "$topology_eval_status" \
        --arg topology_status "$topology_status" \
        --arg topology_path "$topology_path" \
        --arg topology_eval_log "$topology_eval_log_path" \
        --arg topology_log "$topology_log_path" \
        --arg template_refs_status "$template_refs_status" \
        --arg template_refs_log "$template_refs_log_path" \
        --arg system_eval_status "$system_eval_status" \
        --arg system_status "$system_status" \
        --arg system_path "$system_path" \
        --arg system_eval_log "$system_eval_log_path" \
        --arg system_log "$system_log_path" \
        --arg system_refs_status "$system_refs_status" \
        --arg system_refs_log "$system_refs_log_path" \
        --arg flake_status "$flake_status" \
        --arg flake_log "$flake_log" \
        --arg flake_tests_status "$flake_tests_status" \
        --arg flake_tests_log "$flake_tests_log" \
        '{
          schema: $schema,
          status: $status,
          reason: $reason,
          validator_role: $validator_role,
          commit: $commit,
          checks: [
            {
              name: "candidate-manifest",
              status: "passed",
              manifest: $manifest
            },
            {
              name: "git-bundle-verify",
              status: "passed",
              log: $bundle_verify_log
            },
            {
              name: "git-clone",
              status: "passed",
              log: $git_clone_log
            },
            {
              name: "git-clean",
              status: (if $dirty == "" then "passed" else "failed" end),
              details: $dirty
            },
            {
              name: "nix-eval-airlock-topology",
              status: $topology_eval_status,
              attr: "airlock.topology",
              output: $topology_path,
              log: $topology_eval_log
            },
            {
              name: "airlock-topology-json",
              status: $topology_status,
              path: $topology_path,
              log: $topology_log
            },
            {
              name: "nix-template-source-refs-dry-run",
              status: $template_refs_status,
              log: $template_refs_log
            },
            {
              name: "nix-eval-airlock-system",
              status: $system_eval_status,
              attr: "airlock.system",
              output: $system_path,
              log: $system_eval_log
            },
            {
              name: "airlock-system-json",
              status: $system_status,
              path: $system_path,
              log: $system_log
            },
            {
              name: "nix-system-source-refs-dry-run",
              status: $system_refs_status,
              log: $system_refs_log
            },
            {
              name: "nix-flake-check-no-build",
              status: $flake_status,
              log: $flake_log
            },
            {
              name: "nix-flake-check",
              status: $flake_tests_status,
              log: $flake_tests_log
            }
          ]
        }' > "$result_tmp"
      mv "$result_tmp" "$result_file"
      ${pkgs.coreutils}/bin/cp "$result_file" "$tmpdir/validation.json"
    }

    if [ -n "$dirty" ]; then
      write_validation failed "validated candidate worktree is dirty" skipped skipped "topology.json" "$topology_eval_log" "$topology_log" skipped "$template_refs_log" skipped skipped "system.json" "$system_eval_log" "$system_log" skipped "$system_refs_log" skipped skipped
      cat "$tmpdir/validation.json" >&2
      exit 1
    fi

    mark_failed() {
      if [ "$status" = passed ]; then
        reason="$1"
      fi
      status=failed
    }

    topology_eval_status=skipped
    topology_status=skipped
    template_refs_status=skipped
    system_eval_status=skipped
    system_status=skipped
    system_refs_status=skipped
    flake_status=skipped
    flake_tests_status=skipped
    reason="validation passed"
    status=passed

    check_template_source_refs() {
      topology_file="$1"
      log="$2"

      : > "$log"
      refs="$(
        ${pkgs.jq}/bin/jq -r '
          [
            (
              (.templates // {})
              | to_entries[]
              | select(.value.source_ref != null)
              | {
                  kind: "template",
                  name: .key,
                  ref: .value.source_ref
                }
            ),
            (
              (.domains // {})
              | to_entries[]
              | . as $domain
              | ($domain.value.backend // {}) as $backend
              | select($backend.type == "nix-closure")
              | [
                  { component: "system", ref: ($backend.system.source_ref // null) },
                  { component: "kernel", ref: ($backend.kernel.source_ref // null) },
                  { component: "initrd", ref: ($backend.initrd.source_ref // null) },
                  { component: "modules", ref: ($backend.modules.source_ref // null) },
                  { component: "root", ref: ($backend.root.source_ref // null) },
                  { component: "store", ref: ($backend.store.source_ref // null) }
                ]
              | .[]
              | select(.ref != null)
              | {
                  kind: "nix-domain",
                  name: ($domain.key + "." + .component),
                  ref: .ref
                }
            )
          ]
          | .[]
          | @base64
        ' "$topology_file"
      )"

      if [ -z "$refs" ]; then
        printf 'no topology source refs declared\n' >> "$log"
        return 0
      fi

      printf '%s\n' "$refs" |
        while IFS= read -r encoded; do
          [ -n "$encoded" ] || continue
          ref_json="$(printf '%s' "$encoded" | ${pkgs.coreutils}/bin/base64 -d)"
          kind="$(printf '%s' "$ref_json" | ${pkgs.jq}/bin/jq -r '.kind')"
          name="$(printf '%s' "$ref_json" | ${pkgs.jq}/bin/jq -r '.name')"
          attr="$(printf '%s' "$ref_json" | ${pkgs.jq}/bin/jq -r '.ref.attr')"
          printf 'checking %s=%s attr=%s\n' "$kind" "$name" "$attr" >> "$log"
          ${configNix}/bin/airlock-config-nix \
            --extra-experimental-features nix-command \
            --extra-experimental-features flakes \
            build \
            --impure \
            --offline \
            --no-write-lock-file \
            --dry-run \
            --no-link \
            --show-trace \
            ".#$attr" >> "$log" 2>&1
        done
    }

    check_system_source_refs() {
      system_file="$1"
      log="$2"

      : > "$log"
      refs="$(
        ${pkgs.jq}/bin/jq -r '
          [
            { name: "dom0.profile", ref: .dom0.profile.source_ref },
            { name: "dom0.nixos.system", ref: .dom0.nixos.system.source_ref },
            { name: "dom0.nixos.kernel", ref: .dom0.nixos.kernel.source_ref },
            { name: "dom0.nixos.xen", ref: .dom0.nixos.xen.source_ref },
            { name: "dom0.nixos.bootloader", ref: .dom0.nixos.bootloader.source_ref },
            { name: "dom0.nixos.qubes", ref: .dom0.nixos.qubes.source_ref }
          ]
          | .[]
          | select(.ref != null)
          | @base64
        ' "$system_file"
      )"
      direct_sources="$(
        ${pkgs.jq}/bin/jq -r '
          [
            {
              component: "dom0.profile",
              source: .dom0.profile.source,
              activation_command: .dom0.profile.activation_command
            },
            {
              component: "dom0.nixos.system",
              source: .dom0.nixos.system.source,
              activation_command: null
            },
            {
              component: "dom0.nixos.kernel",
              source: .dom0.nixos.kernel.source,
              activation_command: null
            },
            {
              component: "dom0.nixos.xen",
              source: .dom0.nixos.xen.source,
              activation_command: null
            },
            {
              component: "dom0.nixos.bootloader",
              source: .dom0.nixos.bootloader.source,
              activation_command: null
            },
            {
              component: "dom0.nixos.qubes",
              source: .dom0.nixos.qubes.source,
              activation_command: null
            }
          ]
          | .[]
          | select(.source != null)
          | @base64
        ' "$system_file"
      )"

      if [ -z "$refs" ] && [ -z "$direct_sources" ]; then
        printf 'no system sources declared\n' >> "$log"
        return 0
      fi

      if [ -n "$refs" ]; then
        printf '%s\n' "$refs" |
          while IFS= read -r encoded; do
            [ -n "$encoded" ] || continue
            ref_json="$(printf '%s' "$encoded" | ${pkgs.coreutils}/bin/base64 -d)"
            name="$(printf '%s' "$ref_json" | ${pkgs.jq}/bin/jq -r '.name')"
            attr="$(printf '%s' "$ref_json" | ${pkgs.jq}/bin/jq -r '.ref.attr')"
            printf 'checking system=%s attr=%s\n' "$name" "$attr" >> "$log"
            ${configNix}/bin/airlock-config-nix \
              --extra-experimental-features nix-command \
              --extra-experimental-features flakes \
              build \
              --impure \
              --offline \
              --no-write-lock-file \
              --dry-run \
              --no-link \
              --show-trace \
              ".#$attr" >> "$log" 2>&1
          done
      fi

      if [ -n "$direct_sources" ]; then
        printf '%s\n' "$direct_sources" |
          while IFS= read -r encoded; do
          [ -n "$encoded" ] || continue
          source_json="$(printf '%s' "$encoded" | ${pkgs.coreutils}/bin/base64 -d)"
          component="$(printf '%s' "$source_json" | ${pkgs.jq}/bin/jq -r '.component')"
          source="$(printf '%s' "$source_json" | ${pkgs.jq}/bin/jq -r '.source')"
          activation_command="$(printf '%s' "$source_json" | ${pkgs.jq}/bin/jq -r '.activation_command // ""')"
          case "$source" in
            "")
              printf 'empty direct system source for %s\n' "$component" >> "$log"
              exit 1
              ;;
            /*)
              resolved="$source"
              ;;
            .|./|..|../*|*/..|*/../*|*//*)
              printf 'unsafe relative system source for %s: %s\n' "$component" "$source" >> "$log"
              exit 1
              ;;
            ./*)
              stripped="$(printf '%s' "$source" | ${pkgs.gnused}/bin/sed 's|^\./||')"
              resolved="$PWD/$stripped"
              ;;
            *)
              resolved="$PWD/$source"
              ;;
          esac
          printf 'checking system=%s source=%s resolved=%s\n' "$component" "$source" "$resolved" >> "$log"
          [ -e "$resolved" ] || {
            printf 'direct system source is missing for %s: %s\n' "$component" "$source" >> "$log"
            exit 1
          }
          if [ "$component" = "dom0.profile" ]; then
            [ -d "$resolved" ] || {
              printf 'direct dom0 profile source is not a directory: %s\n' "$source" >> "$log"
              exit 1
            }
            [ -n "$activation_command" ] || activation_command="bin/airlock-dom0-profile-activate"
            [ -x "$resolved/$activation_command" ] || {
              printf 'direct dom0 profile activation command is missing or not executable: %s/%s\n' "$source" "$activation_command" >> "$log"
              exit 1
            }
          fi
          if [ "$component" = "dom0.nixos.system" ]; then
            [ -d "$resolved" ] || {
              printf 'dom0 NixOS system source is not a directory: %s\n' "$source" >> "$log"
              exit 1
            }
            [ -x "$resolved/bin/switch-to-configuration" ] || {
              printf 'dom0 NixOS system source is missing bin/switch-to-configuration: %s\n' "$source" >> "$log"
              exit 1
            }
          fi
          done
      fi
    }

    if [ -f flake.nix ]; then
      if ${configNix}/bin/airlock-config-nix \
        --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        flake check \
        --impure \
        --offline \
        --no-write-lock-file \
        --no-build . > "$local_flake_log" 2>&1; then
        flake_status=passed
      else
        flake_status=failed
        mark_failed "nix flake check --no-build failed"
      fi
      ${pkgs.coreutils}/bin/cp "$local_flake_log" "$flake_log"
      if [ "$flake_status" = passed ] && [ "$run_full_flake_check" = 1 ]; then
        if ${configNix}/bin/airlock-config-nix \
          --extra-experimental-features nix-command \
          --extra-experimental-features flakes \
          flake check \
          --impure \
          --offline \
          --no-write-lock-file \
          . > "$local_flake_tests_log" 2>&1; then
          flake_tests_status=passed
        else
          flake_tests_status=failed
          mark_failed "nix flake check failed"
        fi
        ${pkgs.coreutils}/bin/cp "$local_flake_tests_log" "$flake_tests_log"
      elif [ "$flake_status" = passed ]; then
        printf 'skipped offline full flake build; set AIRLOCK_CONFIG_VALIDATION_BUILD=1 when all build inputs are preloaded\n' > "$flake_tests_log"
      else
        printf 'skipped because nix flake check --no-build did not pass\n' > "$flake_tests_log"
      fi
    else
      printf 'no flake.nix present; cannot evaluate airlock.topology\n' > "$flake_log"
      printf 'no flake.nix present; cannot run nix flake check\n' > "$flake_tests_log"
      flake_status=failed
      flake_tests_status=failed
      mark_failed "flake.nix is missing"
    fi

    if [ -f flake.nix ]; then
      if ${configNix}/bin/airlock-config-nix \
        --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        eval \
        --impure \
        --offline \
        --no-write-lock-file \
        .#airlock.topology --json > "$generated_topology" 2> "$local_topology_eval_log"; then
        topology_eval_status=passed
      else
        topology_eval_status=failed
        : > "$generated_topology"
        mark_failed "nix eval .#airlock.topology failed"
      fi
      ${pkgs.coreutils}/bin/cp "$local_topology_eval_log" "$topology_eval_log"
    else
      printf 'no flake.nix present; cannot evaluate airlock.topology\n' > "$topology_eval_log"
    fi

    if [ "$topology_eval_status" = passed ]; then
      if ${pkgs.jq}/bin/jq -e '
        .schema == "airlock.qubes-topology.v1" and
        (.templates | type == "object") and
        (.kernels | type == "object") and
        (.prefs | type == "object") and
        (.domains | type == "object") and
        (.qrexec_policy | type == "object") and
        (.qrexec_policy.rules | type == "array")
      ' "$generated_topology" > "$local_topology_log" 2>&1; then
        topology_status=passed
      else
          topology_status=failed
          mark_failed "evaluated airlock.topology JSON validation failed"
        fi
        ${pkgs.coreutils}/bin/cp "$local_topology_log" "$topology_log"

        if check_template_source_refs "$generated_topology" "$local_template_refs_log"; then
          template_refs_status=passed
        else
          template_refs_status=failed
          mark_failed "template source reference dry-run failed"
        fi
        ${pkgs.coreutils}/bin/cp "$local_template_refs_log" "$template_refs_log"
      else
        printf 'skipped because nix eval .#airlock.topology did not pass\n' > "$topology_log"
        printf 'skipped because nix eval .#airlock.topology did not pass\n' > "$template_refs_log"
      fi

      if ${configNix}/bin/airlock-config-nix \
        --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        eval \
        --impure \
        --offline \
        --no-write-lock-file \
        .#airlock.system --json > "$generated_system" 2> "$local_system_eval_log"; then
        system_eval_status=passed
      else
        system_eval_status=failed
        : > "$generated_system"
        mark_failed "nix eval .#airlock.system failed"
      fi
      ${pkgs.coreutils}/bin/cp "$local_system_eval_log" "$system_eval_log"

      if [ "$system_eval_status" = passed ]; then
        if ${pkgs.jq}/bin/jq -e '
          .schema == "airlock.system.v1" and
          (.dom0 | type == "object") and
          (.plan | type == "object") and
          (.plan.live_actions | type == "array") and
          (.plan.reboot_actions | type == "array") and
          (.plan.reboot_required | type == "boolean") and
          (.plan.reboot_reasons | type == "array")
        ' "$generated_system" > "$local_system_log" 2>&1; then
          system_status=passed
        else
          system_status=failed
          mark_failed "evaluated airlock.system JSON validation failed"
        fi
        ${pkgs.coreutils}/bin/cp "$local_system_log" "$system_log"

        if check_system_source_refs "$generated_system" "$local_system_refs_log"; then
          system_refs_status=passed
        else
          system_refs_status=failed
          mark_failed "system source reference dry-run failed"
        fi
        ${pkgs.coreutils}/bin/cp "$local_system_refs_log" "$system_refs_log"
      else
        printf 'skipped because nix eval .#airlock.system did not pass\n' > "$system_log"
        printf 'skipped because nix eval .#airlock.system did not pass\n' > "$system_refs_log"
      fi

    write_validation "$status" "$reason" "$topology_eval_status" "$topology_status" "topology.json" "$topology_eval_log" "$topology_log" "$template_refs_status" "$template_refs_log" "$system_eval_status" "$system_status" "system.json" "$system_eval_log" "$system_log" "$system_refs_status" "$system_refs_log" "$flake_status" "$flake_tests_status"
    if [ "$status" != passed ]; then
      cat "$tmpdir/validation.json" >&2
      exit 1
    fi

    ${pkgs.coreutils}/bin/cp "$generated_topology" "$tmpdir/topology.json"
    ${pkgs.coreutils}/bin/cp "$generated_system" "$tmpdir/system.json"
    exec ${pkgs.gnutar}/bin/tar -C "$tmpdir" -cf - config.bundle validation.json manifest.json topology.json system.json
  '';

  configCandidateValidateService = pkgs.writeShellScript "airlock-config-candidate-validate-qrexec" ''
    exec ${configValidatorValidateCandidate}/bin/airlock-config-validator-validate-candidate
  '';

  configInputsImportService = pkgs.writeShellScript "airlock-config-inputs-import-qrexec" ''
    set -eu

    export PATH=${
      lib.makeBinPath [
        pkgs.coreutils
        pkgs.gawk
        pkgs.gnugrep
        pkgs.gnutar
        pkgs.gzip
        pkgs.jq
        pkgs.nix
        pkgs.util-linux
      ]
    }

    import_user=${lib.escapeShellArg cfg.user}
    target_root="''${AIRLOCK_CONFIG_INPUTS_PATH:-${cfg.configInputsPath}}"
    store_root="''${AIRLOCK_CONFIG_NIX_STORE_ROOT:-${cfg.configNixStoreRoot}}"
    public_root=${lib.escapeShellArg configInputsPublicPath}
    case "$target_root" in
      /rw/*) ;;
      *)
        printf 'AIRLOCK_CONFIG_INPUTS_PATH must live under /rw: %s\n' "$target_root" >&2
        exit 1
        ;;
    esac
    case "$store_root" in
      /*) ;;
      *)
        printf 'AIRLOCK_CONFIG_NIX_STORE_ROOT must be absolute: %s\n' "$store_root" >&2
        exit 1
        ;;
    esac

    config_inputs_parent="''${target_root%/*}"
    mkdir -p "$config_inputs_parent" "$store_root" "$target_root"
    chown "$import_user:users" "$target_root" "$store_root" || true
    tmpdir="$(mktemp -d "$config_inputs_parent/.config-inputs-import.XXXXXX")"
    trap 'rm -rf "$tmpdir"' EXIT

    run_as_import_user() {
      ${pkgs.util-linux}/bin/runuser -u "$import_user" -- \
        ${pkgs.coreutils}/bin/env \
          HOME="/home/$import_user" \
          USER="$import_user" \
          LOGNAME="$import_user" \
          XDG_CACHE_HOME="/home/$import_user/.cache" \
          "$@"
    }

    mount_config_inputs() {
      mkdir -p "$public_root"
      jq -r '.inputs | to_entries[] | [.key, .value] | @tsv' "$target_root/manifest.json" |
        while IFS="$(printf '\t')" read -r name input_path; do
          case "$name" in
            nixpkgs|nixpkgs-libvirt_10_5|nixos-qubes|airlock-support) ;;
            *)
              printf 'invalid config input name: %s\n' "$name" >&2
              exit 1
              ;;
          esac
          input_store_path="$(printf '%s\n' "$input_path" | ${pkgs.gnused}/bin/sed 's|^\(/nix/store/[^/]*\).*|\1|')"
          input_relative_path="''${input_path#"$input_store_path"}"
          physical_path="$store_root$input_store_path$input_relative_path"
          [ -d "$physical_path" ] || {
            printf 'imported config input path is missing from private Nix store: %s\n' "$physical_path" >&2
            exit 1
          }
          mountpoint_path="$public_root/$name"
          if mountpoint -q "$mountpoint_path"; then
            umount "$mountpoint_path"
          fi
          if [ -e "$mountpoint_path" ] && [ ! -d "$mountpoint_path" ]; then
            rm -rf "$mountpoint_path"
          fi
          mkdir -p "$mountpoint_path"
          mount --bind "$physical_path" "$mountpoint_path"
        done
    }

    archive="$tmpdir/config-inputs.tar"
    cat > "$archive"
    mkdir -p "$tmpdir/unpack"

    tar -tf "$archive" |
      awk '
        $0 == "" ||
        $0 ~ /^\// ||
        $0 ~ /(^|\/)\.\.($|\/)/ ||
        $0 ~ /\/\// {
          printf "unsafe config input tar member: %s\n", $0 > "/dev/stderr"
          exit 1
        }
        $0 == "manifest.json" ||
        $0 == "store-paths" ||
        $0 == "store-export.nar.gz" {
          next
        }
        {
          printf "unexpected config input tar member: %s\n", $0 > "/dev/stderr"
          exit 1
        }
      '

    tar -C "$tmpdir/unpack" -xf "$archive"
    [ -f "$tmpdir/unpack/manifest.json" ] || {
      printf 'config input tar is missing manifest.json\n' >&2
      exit 1
    }
    [ -f "$tmpdir/unpack/store-paths" ] || {
      printf 'config input tar is missing store-paths\n' >&2
      exit 1
    }
    [ -f "$tmpdir/unpack/store-export.nar.gz" ] || {
      printf 'config input tar is missing store-export.nar.gz\n' >&2
      exit 1
    }

    jq -e '
      .schema == "airlock.config-inputs.v1" and
      (.inputs | keys | sort) == [
        "airlock-support",
        "nixos-qubes",
        "nixpkgs",
        "nixpkgs-libvirt_10_5"
      ] and
      all(.inputs[];
        test("^/nix/store/[A-Za-z0-9]+-[^/]+($|/)") and
        (contains("//") | not) and
        (contains("/../") | not)
      )
    ' "$tmpdir/unpack/manifest.json" >/dev/null

    awk '
      $0 !~ /^\/nix\/store\/[A-Za-z0-9]+-[^\/]+$/ {
        printf "invalid config input store path: %s\n", $0 > "/dev/stderr"
        exit 1
      }
    ' "$tmpdir/unpack/store-paths"

    while IFS= read -r store_path; do
      if ! run_as_import_user nix-store --store "$store_root" --check-validity "$store_path" >/dev/null 2>&1; then
        rm -rf "$store_root$store_path" "$store_root$store_path.lock"
      fi
    done < "$tmpdir/unpack/store-paths"
    gzip -dc "$tmpdir/unpack/store-export.nar.gz" |
      run_as_import_user nix-store --store "$store_root" --import >/dev/null
    while IFS= read -r store_path; do
      run_as_import_user nix-store --store "$store_root" --check-validity "$store_path" >/dev/null
    done < "$tmpdir/unpack/store-paths"

    cp "$tmpdir/unpack/manifest.json" "$target_root/manifest.json.new"
    chown "$import_user:users" "$target_root/manifest.json.new" || true
    mv "$target_root/manifest.json.new" "$target_root/manifest.json"
    mount_config_inputs

    locked=false
    ${lib.optionalString hasConfigStagingRole ''
      run_staging_init() {
        ${pkgs.coreutils}/bin/env \
          HOME=/home/${lib.escapeShellArg cfg.user} \
          USER=${lib.escapeShellArg cfg.user} \
          LOGNAME=${lib.escapeShellArg cfg.user} \
          XDG_CACHE_HOME=/home/${lib.escapeShellArg cfg.user}/.cache \
          AIRLOCK_CONFIG_INPUTS_PATH=${lib.escapeShellArg configInputsPublicPath} \
          AIRLOCK_CONFIG_REFRESH_INPUTS=1 \
          PATH=${lib.escapeShellArg (roleInitPathFor "sys-config-staging")} \
          ${configStagingInit}/bin/airlock-config-staging-init
      }
      if [ "$(${pkgs.coreutils}/bin/id -un)" = ${lib.escapeShellArg cfg.user} ]; then
        run_staging_init
      else
        ${pkgs.util-linux}/bin/runuser -u ${lib.escapeShellArg cfg.user} -- \
          ${pkgs.coreutils}/bin/env \
            HOME=/home/${lib.escapeShellArg cfg.user} \
            USER=${lib.escapeShellArg cfg.user} \
            LOGNAME=${lib.escapeShellArg cfg.user} \
            XDG_CACHE_HOME=/home/${lib.escapeShellArg cfg.user}/.cache \
            AIRLOCK_CONFIG_INPUTS_PATH=${lib.escapeShellArg configInputsPublicPath} \
            AIRLOCK_CONFIG_REFRESH_INPUTS=1 \
            PATH=${lib.escapeShellArg (roleInitPathFor "sys-config-staging")} \
            ${configStagingInit}/bin/airlock-config-staging-init
      fi
      [ -f ${lib.escapeShellArg cfg.configRepoPath}/flake.lock ] && locked=true
    ''}

    printf '{"schema":"airlock.config-inputs-import.v1","status":"passed","path":"%s","flake_lock":%s}\n' \
      "$target_root" \
      "$locked"
  '';

  configVaultArchiveCandidate = pkgs.writeShellScriptBin "airlock-config-vault-archive-candidate" ''
    set -eu

    repo="''${AIRLOCK_CONFIG_ARCHIVE_PATH:-${cfg.configArchivePath}}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    candidate_tar="$tmpdir/candidate.tar"
    ${pkgs.coreutils}/bin/cat > "$candidate_tar"
    ${pkgs.gnutar}/bin/tar -tf "$candidate_tar" |
      ${pkgs.gawk}/bin/awk '
        $0 != "config.bundle" && $0 != "validation.json" && $0 != "manifest.json" && $0 != "topology.json" && $0 != "system.json" {
          printf "unexpected candidate tar member: %s\n", $0 > "/dev/stderr"
          exit 1
        }
      '
    ${pkgs.gnutar}/bin/tar -C "$tmpdir" -xf "$candidate_tar"

    [ -f "$tmpdir/config.bundle" ] || {
      printf 'candidate tar is missing config.bundle\n' >&2
      exit 1
    }
    [ -f "$tmpdir/validation.json" ] || {
      printf 'candidate tar is missing validation.json\n' >&2
      exit 1
    }
    [ -f "$tmpdir/manifest.json" ] || {
      printf 'candidate tar is missing manifest.json\n' >&2
      exit 1
    }
    [ -f "$tmpdir/topology.json" ] || {
      printf 'candidate tar is missing topology.json\n' >&2
      exit 1
    }
    [ -f "$tmpdir/system.json" ] || {
      printf 'candidate tar is missing system.json\n' >&2
      exit 1
    }

    manifest_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/manifest.json")"
    manifest_commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/manifest.json")"
    validation_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/validation.json")"
    validation_status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/validation.json")"
    validation_commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/validation.json")"
    system_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/system.json")"
    [ "$manifest_schema" = "airlock.config-candidate.v1" ] || {
      printf 'unsupported candidate manifest schema: %s\n' "$manifest_schema" >&2
      exit 1
    }
    [ "$validation_schema" = "airlock.config-validation.v1" ] || {
      printf 'unsupported validation schema: %s\n' "$validation_schema" >&2
      exit 1
    }
    [ "$system_schema" = "airlock.system.v1" ] || {
      printf 'unsupported system schema: %s\n' "$system_schema" >&2
      exit 1
    }
    [ "$validation_status" = passed ] || {
      printf 'validation status is not passed: %s\n' "$validation_status" >&2
      exit 1
    }
    [ "$manifest_commit" = "$validation_commit" ] || {
      printf 'manifest commit %s does not match validation commit %s\n' "$manifest_commit" "$validation_commit" >&2
      exit 1
    }

    verify_repo="$tmpdir/verify.git"
    ${roleGit}/bin/git init --bare -b main "$verify_repo" >/dev/null
    ${roleGit}/bin/git -C "$verify_repo" bundle verify "$tmpdir/config.bundle" > "$tmpdir/bundle-verify.log" 2>&1
    candidate_commit="$(
      ${roleGit}/bin/git -C "$verify_repo" bundle list-heads "$tmpdir/config.bundle" |
        ${pkgs.gawk}/bin/awk 'NR == 1 { print $1 }'
    )"
    [ -n "$candidate_commit" ] || {
      printf 'candidate bundle did not expose a commit\n' >&2
      exit 1
    }
    [ "$candidate_commit" = "$validation_commit" ] || {
      printf 'candidate commit %s does not match validation commit %s\n' "$candidate_commit" "$validation_commit" >&2
      exit 1
    }

    snapshot_tmp="$tmpdir/snapshot"
    ${roleGit}/bin/git clone "$tmpdir/config.bundle" "$snapshot_tmp" > "$tmpdir/snapshot-git-clone.log" 2>&1
    ${roleGit}/bin/git -C "$snapshot_tmp" checkout --detach "$candidate_commit" > "$tmpdir/snapshot-git-checkout.log" 2>&1
    ${pkgs.coreutils}/bin/rm -rf "$snapshot_tmp/.git"

    if [ ! -d "$repo/.git" ]; then
      ${configVaultInit}/bin/airlock-config-vault-init >/dev/null
    fi

    archive_dir="$repo/candidates/$candidate_commit"
    snapshot_dir="$archive_dir/snapshot"
    ${pkgs.coreutils}/bin/mkdir -p "$archive_dir"
    ${pkgs.coreutils}/bin/cp "$tmpdir/config.bundle" "$archive_dir/config.bundle"
    ${pkgs.coreutils}/bin/cp "$tmpdir/manifest.json" "$archive_dir/manifest.json"
    ${pkgs.coreutils}/bin/cp "$tmpdir/validation.json" "$archive_dir/validation.json"
    ${pkgs.coreutils}/bin/cp "$tmpdir/topology.json" "$archive_dir/topology.json"
    ${pkgs.coreutils}/bin/cp "$tmpdir/system.json" "$archive_dir/system.json"
    ${pkgs.coreutils}/bin/cp "$tmpdir/bundle-verify.log" "$archive_dir/bundle-verify.log"
    ${pkgs.coreutils}/bin/cp "$tmpdir/snapshot-git-clone.log" "$archive_dir/snapshot-git-clone.log"
    ${pkgs.coreutils}/bin/cp "$tmpdir/snapshot-git-checkout.log" "$archive_dir/snapshot-git-checkout.log"
    ${pkgs.coreutils}/bin/rm -rf "$snapshot_dir"
    ${pkgs.coreutils}/bin/cp -a "$snapshot_tmp" "$snapshot_dir"
    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "airlock.config-candidate-archive-record.v1" \
      --arg candidate_commit "$candidate_commit" \
      --arg manifest_schema "$manifest_schema" \
      --arg validation_schema "$validation_schema" \
      --arg system_schema "$system_schema" \
      --arg snapshot "$archive_dir/snapshot" \
      --arg topology "$archive_dir/topology.json" \
      --arg system "$archive_dir/system.json" \
      --arg source "dom0" \
      '{
        schema: $schema,
        candidate_commit: $candidate_commit,
        manifest_schema: $manifest_schema,
        validation_schema: $validation_schema,
        system_schema: $system_schema,
        snapshot: $snapshot,
        topology: $topology,
        system: $system,
        archived_by: $source
      }' > "$archive_dir/metadata.json"
    printf '%s\n' "$candidate_commit" > "$repo/latest-candidate"

    cd "$repo"
    ${roleGit}/bin/git add "candidates/$candidate_commit" latest-candidate
    archive_status=current
    if ! ${roleGit}/bin/git diff --cached --quiet; then
      ${roleGit}/bin/git \
        -c user.name='Airlock dom0 archive' \
        -c user.email='airlock-dom0@localhost' \
        commit -m "archive candidate ''${candidate_commit:0:12}" >/dev/null
      archive_status=archived
    fi

    vault_commit="$(${roleGit}/bin/git rev-parse HEAD)"
    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "airlock.config-vault-archive.v1" \
      --arg status "passed" \
      --arg archive_status "$archive_status" \
      --arg candidate_commit "$candidate_commit" \
      --arg vault_commit "$vault_commit" \
      --arg archive_repo "$repo" \
      '{
        schema: $schema,
        status: $status,
        archive_status: $archive_status,
        candidate_commit: $candidate_commit,
        vault_commit: $vault_commit,
        archive_repo: $archive_repo
      }'
  '';

  configVaultArchiveDeployment = pkgs.writeShellScriptBin "airlock-config-vault-archive-deployment" ''
    set -eu

    repo="''${AIRLOCK_CONFIG_ARCHIVE_PATH:-${cfg.configArchivePath}}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    deployment_tar="$tmpdir/deployment.tar"
    ${pkgs.coreutils}/bin/cat > "$deployment_tar"
    ${pkgs.gnutar}/bin/tar -tf "$deployment_tar" |
      ${pkgs.gawk}/bin/awk '
        $0 != "deployment.json" &&
        $0 != "apply.json" &&
        $0 != "archive.json" &&
        $0 != "setup.json" &&
        $0 != "topology.json" &&
        $0 != "validated-topology.json" &&
        $0 != "template-builds.json" &&
        $0 != "nix-domain-builds.json" &&
        $0 != "nix-domain-materializations.json" &&
        $0 != "unsupported-domain-backends.json" &&
        $0 != "domain-references.json" &&
        $0 != "system.json" &&
        $0 != "resolved-system.json" &&
        $0 != "system-plan.json" &&
        $0 != "system-apply.json" &&
        $0 != "system-builds.json" &&
        $0 != "systemd-actions.json" &&
        $0 != "validation.json" &&
        $0 != "manifest.json" &&
        $0 != "logs/apply.commands.log" &&
        $0 != "logs/archive.stdout.log" &&
        $0 != "logs/archive.stderr.log" &&
        $0 != "logs/setup.stdout.log" &&
        $0 != "logs/setup.stderr.log" &&
        $0 != "logs/template-builds.log" &&
        $0 != "logs/nix-domain-builds.log" &&
        $0 != "logs/system-builds.log" &&
        $0 != "logs/system-apply.log" &&
        $0 != "logs/profile-activation.log" &&
        $0 != "logs/host-system-activation.log" &&
        $0 != "logs/systemd-actions.log" &&
        $0 != "logs/clone.log" &&
        $0 != "logs/checkout.log" {
          printf "unexpected deployment tar member: %s\n", $0 > "/dev/stderr"
          exit 1
        }
      '
    ${pkgs.gnutar}/bin/tar -C "$tmpdir" -xf "$deployment_tar"

    for required in \
      deployment.json \
      apply.json \
      archive.json \
      setup.json \
      topology.json \
      validated-topology.json \
      template-builds.json \
      nix-domain-builds.json \
      nix-domain-materializations.json \
      unsupported-domain-backends.json \
      domain-references.json \
      system.json \
      resolved-system.json \
      system-plan.json \
      system-apply.json \
      system-builds.json \
      systemd-actions.json \
      validation.json \
      manifest.json; do
      [ -f "$tmpdir/$required" ] || {
        printf 'deployment tar is missing %s\n' "$required" >&2
        exit 1
      }
    done

    deployment_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/deployment.json")"
    deployment_id="$(${pkgs.jq}/bin/jq -r '.deployment_id' "$tmpdir/deployment.json")"
    deployment_status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/deployment.json")"
    candidate_commit="$(${pkgs.jq}/bin/jq -r '.candidate_commit' "$tmpdir/deployment.json")"
    apply_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/apply.json")"
    apply_commit="$(${pkgs.jq}/bin/jq -r '.candidate_commit' "$tmpdir/apply.json")"
    archive_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/archive.json")"
    archive_status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/archive.json")"
    archive_commit="$(${pkgs.jq}/bin/jq -r '.candidate_commit' "$tmpdir/archive.json")"
    setup_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/setup.json")"
    topology_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/topology.json")"
    validated_topology_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/validated-topology.json")"
    template_builds_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/template-builds.json")"
    nix_domain_builds_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/nix-domain-builds.json")"
    nix_domain_materializations_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/nix-domain-materializations.json")"
    unsupported_domain_backends_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/unsupported-domain-backends.json")"
    system_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/system.json")"
    resolved_system_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/resolved-system.json")"
    system_apply_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/system-apply.json")"
    system_apply_status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/system-apply.json")"
    system_builds_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/system-builds.json")"
    systemd_actions_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/systemd-actions.json")"
    validation_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/validation.json")"
    validation_status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/validation.json")"
    validation_commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/validation.json")"

    domain_references_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/domain-references.json")"
    domain_references_status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/domain-references.json")"

    [ "$deployment_schema" = "airlock.config-deployment.v1" ] || {
      printf 'unsupported deployment schema: %s\n' "$deployment_schema" >&2
      exit 1
    }
    case "$deployment_id" in
      ""|*[!A-Za-z0-9_.:-]*)
        printf 'invalid deployment id: %s\n' "$deployment_id" >&2
        exit 1
        ;;
    esac
    case "$deployment_status" in
      passed|failed) ;;
      *)
        printf 'invalid deployment status: %s\n' "$deployment_status" >&2
        exit 1
        ;;
    esac
    [ -n "$candidate_commit" ] && [ "$candidate_commit" != null ] || {
      printf 'deployment record is missing candidate_commit\n' >&2
      exit 1
    }
    [ "$apply_schema" = "airlock.config-apply.v1" ] || {
      printf 'unsupported apply schema: %s\n' "$apply_schema" >&2
      exit 1
    }
    [ "$archive_schema" = "airlock.config-archive.v1" ] || {
      printf 'unsupported archive schema: %s\n' "$archive_schema" >&2
      exit 1
    }
    [ "$archive_status" = passed ] || {
      printf 'candidate archive did not pass: %s\n' "$archive_status" >&2
      exit 1
    }
    [ "$setup_schema" = "airlock.setup-config.v1" ] || {
      printf 'unsupported setup schema: %s\n' "$setup_schema" >&2
      exit 1
    }
    [ "$topology_schema" = "airlock.qubes-topology.v1" ] || {
      printf 'unsupported topology schema: %s\n' "$topology_schema" >&2
      exit 1
    }
    [ "$validated_topology_schema" = "airlock.qubes-topology.v1" ] || {
      printf 'unsupported validated topology schema: %s\n' "$validated_topology_schema" >&2
      exit 1
    }
    [ "$template_builds_schema" = "airlock.template-builds.v1" ] || {
      printf 'unsupported template builds schema: %s\n' "$template_builds_schema" >&2
      exit 1
    }
    [ "$nix_domain_builds_schema" = "airlock.nix-domain-builds.v1" ] || {
      printf 'unsupported Nix-native domain builds schema: %s\n' "$nix_domain_builds_schema" >&2
      exit 1
    }
    [ "$nix_domain_materializations_schema" = "airlock.nix-domain-materializations.v1" ] || {
      printf 'unsupported Nix-native domain materializations schema: %s\n' "$nix_domain_materializations_schema" >&2
      exit 1
    }
    [ "$unsupported_domain_backends_schema" = "airlock.unsupported-domain-backends.v1" ] || {
      printf 'unsupported domain backend report schema: %s\n' "$unsupported_domain_backends_schema" >&2
      exit 1
    }
    [ "$domain_references_schema" = "airlock.domain-references.v1" ] || {
      printf 'unsupported domain references schema: %s\n' "$domain_references_schema" >&2
      exit 1
    }
    [ "$domain_references_status" = passed ] || {
      printf 'domain references did not pass: %s\n' "$domain_references_status" >&2
      exit 1
    }
    [ "$system_schema" = "airlock.system.v1" ] || {
      printf 'unsupported system schema: %s\n' "$system_schema" >&2
      exit 1
    }
    [ "$resolved_system_schema" = "airlock.system.v1" ] || {
      printf 'unsupported resolved system schema: %s\n' "$resolved_system_schema" >&2
      exit 1
    }
    [ "$system_apply_schema" = "airlock.system-apply.v1" ] || {
      printf 'unsupported system apply schema: %s\n' "$system_apply_schema" >&2
      exit 1
    }
    [ "$system_apply_status" = passed ] || {
      printf 'system apply did not pass: %s\n' "$system_apply_status" >&2
      exit 1
    }
    [ "$system_builds_schema" = "airlock.system-builds.v1" ] || {
      printf 'unsupported system builds schema: %s\n' "$system_builds_schema" >&2
      exit 1
    }
    [ "$systemd_actions_schema" = "airlock.systemd-actions.v1" ] || {
      printf 'unsupported systemd actions schema: %s\n' "$systemd_actions_schema" >&2
      exit 1
    }
    [ "$validation_schema" = "airlock.config-validation.v1" ] || {
      printf 'unsupported validation schema: %s\n' "$validation_schema" >&2
      exit 1
    }
    [ "$validation_status" = passed ] || {
      printf 'candidate validation did not pass: %s\n' "$validation_status" >&2
      exit 1
    }
    [ "$apply_commit" = "$candidate_commit" ] || {
      printf 'apply commit %s does not match deployment commit %s\n' \
        "$apply_commit" \
        "$candidate_commit" >&2
      exit 1
    }
    [ "$archive_commit" = "$candidate_commit" ] || {
      printf 'archive commit %s does not match deployment commit %s\n' \
        "$archive_commit" \
        "$candidate_commit" >&2
      exit 1
    }
    [ "$validation_commit" = "$candidate_commit" ] || {
      printf 'validation commit %s does not match deployment commit %s\n' \
        "$validation_commit" \
        "$candidate_commit" >&2
      exit 1
    }

    if [ ! -d "$repo/.git" ]; then
      ${configVaultInit}/bin/airlock-config-vault-init >/dev/null
    fi

    [ -d "$repo/candidates/$candidate_commit" ] || {
      printf 'deployment references missing candidate archive: %s\n' "$candidate_commit" >&2
      exit 1
    }

    deployment_dir="$repo/deployments/$deployment_id"
    [ ! -e "$deployment_dir" ] || {
      printf 'deployment already exists: %s\n' "$deployment_id" >&2
      exit 1
    }

    ${pkgs.coreutils}/bin/mkdir -p "$deployment_dir/logs"
    for file in \
      deployment.json \
      apply.json \
      archive.json \
      setup.json \
      topology.json \
      validated-topology.json \
      template-builds.json \
      nix-domain-builds.json \
      nix-domain-materializations.json \
      unsupported-domain-backends.json \
      domain-references.json \
      system.json \
      resolved-system.json \
      system-plan.json \
      system-apply.json \
      system-builds.json \
      systemd-actions.json \
      validation.json \
      manifest.json; do
      ${pkgs.coreutils}/bin/cp "$tmpdir/$file" "$deployment_dir/$file"
    done
    for log in \
      apply.commands.log \
      archive.stdout.log \
      archive.stderr.log \
      setup.stdout.log \
      setup.stderr.log \
      template-builds.log \
      nix-domain-builds.log \
      system-builds.log \
      system-apply.log \
      profile-activation.log \
      host-system-activation.log \
      systemd-actions.log \
      clone.log \
      checkout.log; do
      ${pkgs.coreutils}/bin/cp "$tmpdir/logs/$log" "$deployment_dir/logs/$log"
    done

    printf '%s\n' "$deployment_id" > "$repo/latest-deployment-attempt"
    if [ "$deployment_status" = passed ]; then
      printf '%s\n' "$deployment_id" > "$repo/latest-deployment"
      printf '%s\n' "$candidate_commit" > "$repo/latest-current"
      ${pkgs.coreutils}/bin/ln -sfn "candidates/$candidate_commit/snapshot" "$repo/current"
    fi

    cd "$repo"
    ${roleGit}/bin/git add "deployments/$deployment_id" latest-deployment-attempt
    if [ "$deployment_status" = passed ]; then
      ${roleGit}/bin/git add latest-deployment latest-current current
    fi
    ${roleGit}/bin/git \
      -c user.name='Airlock dom0 archive' \
      -c user.email='airlock-dom0@localhost' \
      commit -m "record deployment $deployment_id" >/dev/null

    vault_commit="$(${roleGit}/bin/git rev-parse HEAD)"
    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "airlock.config-vault-deployment.v1" \
      --arg status "passed" \
      --arg deployment_status "$deployment_status" \
      --arg deployment_id "$deployment_id" \
      --arg candidate_commit "$candidate_commit" \
      --arg vault_commit "$vault_commit" \
      --arg archive_repo "$repo" \
      '{
        schema: $schema,
        status: $status,
        deployment_status: $deployment_status,
        deployment_id: $deployment_id,
        candidate_commit: $candidate_commit,
        vault_commit: $vault_commit,
        archive_repo: $archive_repo
      }'
  '';

  configArchiveService = pkgs.writeShellScript "airlock-config-archive-candidate-qrexec" ''
    exec ${configVaultArchiveCandidate}/bin/airlock-config-vault-archive-candidate
  '';

  configDeploymentArchiveService = pkgs.writeShellScript "airlock-config-archive-deployment-qrexec" ''
    exec ${configVaultArchiveDeployment}/bin/airlock-config-vault-archive-deployment
  '';

  nixStoreRuntimeCheckService = pkgs.writeShellScript "airlock-nix-store-runtime-check-qrexec" ''
    set -eu

    export PATH=${
      lib.makeBinPath [
        pkgs.coreutils
        pkgs.gnugrep
        pkgs.gnused
        pkgs.util-linux
      ]
    }

    system_path="$(
      tr ' ' '\n' < /proc/cmdline |
        sed -n 's|^airlock.system=||p' |
        head -n 1
    )"
    store_device="$(
      tr ' ' '\n' < /proc/cmdline |
        sed -n 's|^airlock.storedev=||p' |
        head -n 1
    )"
    store_format="$(
      tr ' ' '\n' < /proc/cmdline |
        sed -n 's|^airlock.storefmt=||p' |
        head -n 1
    )"
    store_blob_device="$(
      tr ' ' '\n' < /proc/cmdline |
        sed -n 's|^airlock.storeblobdev=||p' |
        head -n 1
    )"
    [ -n "$store_format" ] || store_format="erofs"

    [ -n "$system_path" ] || {
      echo "missing airlock.system kernel option" >&2
      exit 1
    }
    [ -n "$store_device" ] || {
      echo "missing airlock.storedev kernel option" >&2
      exit 1
    }
    [ -x "$system_path/init" ] || {
      echo "system init is not executable: $system_path/init" >&2
      exit 1
    }

    current_system="$(readlink -f /run/current-system)"
    [ "$current_system" = "$system_path" ] || {
      echo "current system mismatch: $current_system != $system_path" >&2
      exit 1
    }

    mount_source="$(findmnt -rn -T /nix/store -o SOURCE)"
    mount_options="$(findmnt -rn -T /nix/store -o OPTIONS)"

    [ "$mount_source" = "$store_device" ] || {
      echo "/nix/store source mismatch: $mount_source != $store_device" >&2
      exit 1
    }
    if [ "$store_format" = "erofs" ] && [ -n "$store_blob_device" ]; then
      [ -b "$store_blob_device" ] || {
        echo "store blob device is missing: $store_blob_device" >&2
        exit 1
      }
    fi
    case ",$mount_options," in
      *,ro,*) ;;
      *)
        echo "/nix/store is not mounted read-only: $mount_options" >&2
        exit 1
        ;;
    esac

    write_probe="/nix/store/.airlock-runtime-write-probe-$$"
    if touch "$write_probe" 2>/tmp/airlock-runtime-write-probe.err; then
      rm -f "$write_probe"
      echo "/nix/store accepted a write probe" >&2
      exit 1
    fi

    printf 'system=%s\n' "$system_path"
    printf 'store_device=%s\n' "$store_device"
    printf 'store_format=%s\n' "$store_format"
    printf 'store_blob_device=%s\n' "$store_blob_device"
    printf 'store_options=%s\n' "$mount_options"
  '';

  policyProbeService = pkgs.writeShellScript "airlock-policy-probe-qrexec" ''
    set -eu

    role=${lib.escapeShellArg cfg.role}
    qubesdb_read="''${AIRLOCK_QUBESDB_READ:-${pkgs.qubes-core-qubesdb}/bin/qubesdb-read}"
    qdb_role="$("$qubesdb_read" /features/airlock-role 2>/dev/null || true)"
    vm_name="$("$qubesdb_read" /name 2>/dev/null || true)"

    case "$qdb_role" in
      sys-config-vault|sys-config-staging|sys-config-validator) role="$qdb_role" ;;
      "") ;;
      *) role=base ;;
    esac

    case "$vm_name" in
      sys-config-vault) role=sys-config-vault ;;
      sys-config-staging) role=sys-config-staging ;;
      sys-config-validator) role=sys-config-validator ;;
    esac

    printf 'airlock-policy-probe role=%s user=%s\n' "$role" "$(${pkgs.coreutils}/bin/whoami)"
  '';

  vmShellService = pkgs.writeShellScript "airlock-qubes-vmshell" ''
    export PATH=/run/current-system/sw/bin:/run/wrappers/bin:/bin:/usr/bin
    exec ${pkgs.runtimeShell}
  '';

  getAppmenusService = pkgs.writeShellScript "airlock-qubes-get-appmenus" ''
    # Keep this close to upstream qubes.GetAppmenus, but use NixOS paths.
    if [ -r /etc/profile ]; then
      # shellcheck disable=SC1091
      . /etc/profile
    fi

    export PATH=${
      lib.makeBinPath [
        pkgs.coreutils
        pkgs.findutils
        pkgs.gawk
        pkgs.glibc.bin
        pkgs.qubes-core-qubesdb
      ]
    }:$PATH

    if [ -z "''${XDG_DATA_HOME:-}" ]; then
      qrexec_user="$(whoami)"
      if [ "$qrexec_user" = root ]; then
        qrexec_user="$(qubesdb-read /default-user 2>/dev/null || printf '%s\n' ${lib.escapeShellArg cfg.user})"
      fi

      home="$(getent passwd "$qrexec_user" | cut -d: -f6)"
      if [ -z "$home" ]; then
        home="/home/$qrexec_user"
      fi
      XDG_DATA_HOME="$home/.local/share"
    fi

    if [ -z "''${XDG_DATA_DIRS:-}" ]; then
      XDG_DATA_DIRS="/run/current-system/sw/share:/usr/local/share:/usr/share"
    fi

    persistence="$(qubesdb-read /qubes-vm-persistence 2>/dev/null || printf 'full\n')"
    rw_devno=""
    if [ -e /rw ]; then
      rw_devno="$(stat -c %D /rw 2>/dev/null || true)"
    fi

    apps_dirs_to_consider=( "$XDG_DATA_HOME" )
    old_IFS="$IFS"
    IFS=:
    # shellcheck disable=SC2206
    apps_dirs_to_consider+=( $XDG_DATA_DIRS )
    IFS="$old_IFS"

    apps_dirs=()
    for dir in "''${apps_dirs_to_consider[@]}"; do
      case "$dir" in
        /*) ;;
        *) continue ;;
      esac

      if [ "$persistence" = full ]; then
        apps_dirs+=( "$dir/applications" )
      elif [ "$persistence" = rw-only ] &&
        [ -n "$rw_devno" ] &&
        [ -e "$dir" ] &&
        [ "$(stat -c %D "$dir" 2>/dev/null || true)" = "$rw_devno" ]; then
        apps_dirs+=( "$dir/applications" )
      fi
    done

    for app_dir in "''${apps_dirs[@]}"; do
      case "$app_dir" in
        /*) ;;
        *) continue ;;
      esac

      [ -d "$app_dir" ] || continue

      (
        cd "$app_dir"
        find . -name '*.desktop' -exec awk -b '
          BEGINFILE { if (ERRNO) nextfile; entry=""; file = substr(FILENAME, 3); gsub(/\//, "-", file) }
          /^\[/ { if (tolower($0) != "\[desktop entry\]") nextfile }
          /^Exec *=/ { entry = entry file ":Exec=qubes-desktop-run " file "\n"; next }
          /^NoDisplay *= *true$/ { entry=""; nextfile }
          /^Categories *= *Screensaver;$/ { entry=""; nextfile }
          /=/ { entry = entry file ":" $0 "\n" }
          ENDFILE { print entry }
        ' 2>/dev/null '{}' +
      )
    done
  '';

in
{
  imports = [
    ./sys-gui/base.nix
  ];

  options.airlock.qubeGuest = {
    enable = lib.mkEnableOption "a Airlock NixOS Qubes guest domain";

    role = lib.mkOption {
      type = lib.types.enum [
        "base"
        "sys-config-vault"
        "sys-config-staging"
        "sys-config-validator"
        "cmd"
        "sys-firewall"
        "sys-gui"
        "sys-net"
      ];
      default = "base";
      description = "Airlock role-specific userspace to place in the guest domain.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "user";
      description = "Primary unprivileged user in the guest domain.";
    };

    allowQrexecUserSelection = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Allow callers to select the target user for qubes.VMShell qrexec requests.";
    };

    configRepoPath = lib.mkOption {
      type = lib.types.str;
      default = "/home/${cfg.user}/airlock-config";
      description = "Editable config repository path in the sys-config-staging role.";
    };

    configValidationPath = lib.mkOption {
      type = lib.types.str;
      default = "/home/${cfg.user}/airlock-config-validation";
      description = "Directory for sys-config-staging and sys-config-validator validation results.";
    };

    configArchivePath = lib.mkOption {
      type = lib.types.str;
      default = "/home/${cfg.user}/airlock-config-vault";
      description = "Archived candidate repository path in the sys-config-vault role.";
    };

    configNixStoreRoot = lib.mkOption {
      type = lib.types.str;
      default = "/rw/home/${cfg.user}/airlock-config-nix-root";
      description = "Private writable chroot Nix store root for config validation in read-only-store guests.";
    };

    configInputsPath = lib.mkOption {
      type = lib.types.str;
      default = "/rw/home/${cfg.user}/airlock-config-inputs";
      description = "Persistent path where dom0 imports offline config flake config inputs.";
    };

    codexHome = lib.mkOption {
      type = lib.types.str;
      default = "/home/${cfg.user}/.codex";
      description = "Persistent Codex CLI state directory. This maps to CODEX_HOME.";
    };

    cmdPersistentPaths = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ "/home/${cfg.user}/dev" ];
      description = "Persistent user paths initialized by the command-workspace role.";
    };
  };

  config = lib.mkIf cfg.enable {
    airlock.sysGuiGuest.enable = hasSysGuiRole;
    assertions = [
      {
        assertion = cfg.user != "";
        message = "airlock.qubeGuest.user must not be empty.";
      }
      {
        assertion = lib.hasPrefix "/home/" cfg.configRepoPath;
        message = "airlock.qubeGuest.configRepoPath should live under /home for Qubes private storage.";
      }
      {
        assertion = lib.hasPrefix "/home/" cfg.configValidationPath;
        message = "airlock.qubeGuest.configValidationPath should live under /home for Qubes private storage.";
      }
      {
        assertion = lib.hasPrefix "/home/" cfg.configArchivePath;
        message = "airlock.qubeGuest.configArchivePath should live under /home for Qubes private storage.";
      }
      {
        assertion = lib.hasPrefix "/rw/" cfg.configNixStoreRoot;
        message = "airlock.qubeGuest.configNixStoreRoot should live under /rw for Qubes private storage.";
      }
      {
        assertion = lib.hasPrefix "/rw/" cfg.configInputsPath;
        message = "airlock.qubeGuest.configInputsPath should live under /rw for Qubes private storage.";
      }
      {
        assertion = lib.hasPrefix "/home/" cfg.codexHome;
        message = "airlock.qubeGuest.codexHome should live under /home for Qubes private storage.";
      }
      {
        assertion = lib.all (path: lib.hasPrefix "/home/" path) cmdPersistentPaths;
        message = "airlock.qubeGuest.cmdPersistentPaths entries should live under /home for Qubes private storage.";
      }
    ];

    boot.loader.grub.enable = false;
    boot.initrd.systemd.enable = lib.mkDefault true;
    airlock.minimalGuest.nix.enable = lib.mkDefault hasPrivateNixRole;
    boot.initrd.availableKernelModules = [
      "xen-blkfront"
      "xen-fbfront"
      "xen-kbdfront"
      "xen-netfront"
      "xen-pcifront"
      "xen-scsifront"
      "xen-tpmfront"
    ];
    boot.initrd.kernelModules = [
      "xen-blkfront"
      "xen-netfront"
      "dm_mod"
      "dm_snapshot"
    ];
    boot.kernelModules = [
      "xen-evtchn"
      "xen-gntalloc"
      "xen-gntdev"
      "xen-privcmd"
    ]
    ++ lib.optional hasNetworkProviderRole "xen-netback";
    boot.kernelParams = [
      "console=hvc0"
    ];

    fileSystems."/" = {
      device = "/dev/mapper/dmroot";
      fsType = "ext4";
    };

    networking.hostName = lib.mkDefault "airlock-qube";
    networking.useDHCP = lib.mkDefault (!hasNetworkProviderRole);
    networking.firewall.enable = lib.mkIf hasNetworkProviderRole false;
    networking.networkmanager.enable = lib.mkIf hasSysNetRole true;
    networking.networkmanager.settings = lib.mkIf hasSysNetRole {
      main.dns = "default";
      keyfile."unmanaged-devices" = "mac:fe:ff:ff:ff:ff:ff";
      device."wifi.scan-rand-mac-address" = "yes";
      connection = {
        "wifi.cloned-mac-address" = "stable";
        "connection.stable-id" = "\${CONNECTION}/\${BOOT}";
        "ipv6.ip6-privacy" = "2";
      };
    };

    users.mutableUsers = lib.mkDefault false;
    users.allowNoPasswordLogin = lib.mkDefault true;
    users.groups.qubes = { };
    users.groups.tinyproxy = lib.mkIf hasNetworkProviderRole { };
    users.users.${cfg.user} = {
      isNormalUser = true;
      group = "users";
      extraGroups = [ "qubes" ];
    };
    users.users.tinyproxy = lib.mkIf hasNetworkProviderRole {
      isSystemUser = true;
      group = "tinyproxy";
    };

    security.pam.services.qrexec = {
      rootOK = true;
      startSession = true;
      setLoginUid = false;
    };

    systemd.tmpfiles.packages = [
      qubesAgentLinux
      pkgs.qubes-linux-utils
    ];

    systemd.tmpfiles.settings."10-airlock-qubes-guest" = {
      "/run/qubes".d = {
        user = "root";
        group = "qubes";
        mode = "0770";
      };
      "/var/log/qubes".d = {
        user = "root";
        group = "qubes";
        mode = "0770";
      };
      "/var/lib/qubes".d = {
        user = "root";
        group = "root";
        mode = "0755";
      };
    };

    systemd.packages = [
      pkgs.qubes-core-qubesdb.daemon
      pkgs.python3.pkgs.qubes-core-qrexec.domU
      pkgs.qubes-linux-utils
    ]
    ++ lib.optional hasNetworkProviderRole pkgs.qubes-vmm-xen;

    systemd.services.qubes-sysinit = {
      enable = true;
      wantedBy = [ "sysinit.target" ];
      after = [
        "systemd-modules-load.service"
        "qubes-db.service"
      ];
      requires = [ "qubes-db.service" ];
      before = [ "sysinit.target" ];
      path = qubesGuestPath;
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${qubesAgentLinux}/lib/qubes/init/qubes-sysinit.sh";
      };
      unitConfig.DefaultDependencies = false;
    };

    systemd.services.airlock-qubes-db-diagnostics = {
      description = "Print QubesDB guest diagnostics to the console";
      path = guestDiagnosticsPath;
      script = ''
        set +e
        echo "airlock guest diagnostics: qubes-db.service failed"
        systemctl --no-pager status qubes-db.service
        journalctl -b --no-pager -u qubes-db.service -n 120
        echo "airlock guest diagnostics: Xen device nodes"
        ls -l /dev/xen /proc/xen
        echo "airlock guest diagnostics: loaded Xen modules"
        lsmod | grep -E '(^xen|evtchn|gnt|privcmd)'
      '';
      serviceConfig = {
        Type = "oneshot";
        StandardOutput = "journal+console";
        StandardError = "journal+console";
      };
    };

    systemd.services.airlock-qubes-qrexec-diagnostics = {
      description = "Print qrexec guest diagnostics to the console";
      path = guestDiagnosticsPath;
      script = ''
        set +e
        echo "airlock guest diagnostics: qubes-qrexec-agent.service failed"
        systemctl --no-pager status qubes-qrexec-agent.service
        journalctl -b --no-pager -u qubes-qrexec-agent.service -n 120
        echo "airlock guest diagnostics: QubesDB service state"
        systemctl --no-pager status qubes-db.service
      '';
      serviceConfig = {
        Type = "oneshot";
        StandardOutput = "journal+console";
        StandardError = "journal+console";
      };
    };

    systemd.services.airlock-qubes-rw-init-diagnostics = {
      description = "Print Qubes private storage diagnostics to the console";
      path = guestDiagnosticsPath;
      script = ''
        set +e
        echo "airlock guest diagnostics: airlock-qubes-rw-init.service failed"
        systemctl --no-pager status airlock-qubes-rw-init.service
        journalctl -b --no-pager -u airlock-qubes-rw-init.service -n 120
        echo "airlock guest diagnostics: filesystem state"
        ls -ld /rw /home /usr/local /var /var/run /run/qubes /var/lib /var/lib/qubes
        findmnt / /rw /home /usr/local
        echo "airlock guest diagnostics: block devices"
        lsblk -o NAME,MAJ:MIN,SIZE,FSTYPE,MOUNTPOINTS
        blkid /dev/xvdb
      '';
      serviceConfig = {
        Type = "oneshot";
        StandardOutput = "journal+console";
        StandardError = "journal+console";
      };
    };

    systemd.services.airlock-qubes-rw-init = {
      description = "Initialize Qubes private storage mounts for NixOS guests";
      wantedBy = [ "multi-user.target" ];
      wants = [ "dev-xvdb.device" ];
      after = [
        "dev-xvdb.device"
        "qubes-sysinit.service"
      ];
      requires = [ "qubes-sysinit.service" ];
      before = [
        "systemd-user-sessions.service"
        "qubes-qrexec-agent.service"
      ];
      path = qubesGuestPath;
      script = ''
        set -eu

        mkdir -p /rw /home /usr/local
        mkdir -p /var/lib/qubes

        if [ ! -e /dev/xvdb ]; then
          echo "Qubes private device /dev/xvdb is not present" >&2
          exit 1
        fi

        ${qubesAgentLinux}/lib/qubes/init/setup-rwdev.sh

        if ! mountpoint -q /rw; then
          mount -t auto -o defaults,discard,nosuid,nodev /dev/xvdb /rw
        fi

        ${qubesAgentLinux}/lib/qubes/init/setup-rw.sh

        . ${qubesAgentLinux}/lib/qubes/init/functions
        initialize_home /rw/home ifneeded

        if ! mountpoint -q /home; then
          mount --bind /rw/home /home
          mount -o remount,bind,nosuid,nodev /home
        fi

        install -d -m 0700 -o ${cfg.user} -g users /home/${cfg.user}

        mkdir -p /rw/usrlocal
        if ! mountpoint -q /usr/local; then
          mount --bind /rw/usrlocal /usr/local
        fi
      '';
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      unitConfig.OnFailure = [ "airlock-qubes-rw-init-diagnostics.service" ];
    };

    systemd.services.airlock-config-inputs-mount = lib.mkIf hasConfigInputsImportRole {
      description = "Expose imported Airlock config inputs";
      wantedBy = [ "multi-user.target" ];
      wants = [ "airlock-qubes-rw-init.service" ];
      requires = [ "airlock-qubes-rw-init.service" ];
      after = [ "airlock-qubes-rw-init.service" ];
      before = [
        "airlock-config-guest-init.service"
        "qubes-qrexec-agent.service"
      ];
      path = [
        pkgs.coreutils
        pkgs.gnused
        pkgs.jq
        pkgs.util-linux
      ];
      script = ''
        set -eu

        target_root=${lib.escapeShellArg cfg.configInputsPath}
        store_root=${lib.escapeShellArg cfg.configNixStoreRoot}
        public_root=${lib.escapeShellArg configInputsPublicPath}
        manifest="$target_root/manifest.json"

        mkdir -p "$target_root" "$public_root"
        [ -f "$manifest" ] || exit 0

        jq -r '.inputs | to_entries[] | [.key, .value] | @tsv' "$manifest" |
          while IFS="$(printf '\t')" read -r name input_path; do
            case "$name" in
              nixpkgs|nixpkgs-libvirt_10_5|nixos-qubes|airlock-support) ;;
              *)
                printf 'invalid config input name: %s\n' "$name" >&2
                exit 1
                ;;
            esac
            input_store_path="$(printf '%s\n' "$input_path" | sed 's|^\(/nix/store/[^/]*\).*|\1|')"
            input_relative_path="''${input_path#"$input_store_path"}"
            physical_path="$store_root$input_store_path$input_relative_path"
            [ -d "$physical_path" ] || {
              printf 'imported config input path is missing from private Nix store: %s\n' "$physical_path" >&2
              exit 1
            }
            mountpoint_path="$public_root/$name"
            if mountpoint -q "$mountpoint_path"; then
              continue
            fi
            if [ -e "$mountpoint_path" ] && [ ! -d "$mountpoint_path" ]; then
              rm -rf "$mountpoint_path"
            fi
            mkdir -p "$mountpoint_path"
            mount --bind "$physical_path" "$mountpoint_path"
          done
      '';
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
    };

    systemd.services.airlock-config-guest-init-diagnostics = lib.mkIf hasConfigRole {
      description = "Print Airlock config guest diagnostics to the console";
      path = guestDiagnosticsPath;
      script = ''
        set +e
        echo "airlock guest diagnostics: airlock-config-guest-init.service failed"
        systemctl --no-pager status airlock-config-guest-init.service
        journalctl -b --no-pager -u airlock-config-guest-init.service -n 160
        echo "airlock guest diagnostics: Qubes identity"
        qubesdb-read /name
        qubesdb-read /features/airlock-role
        echo "airlock guest diagnostics: filesystem state"
        findmnt / /rw /home /nix /nix/store
        ls -ld /rw /rw/home /home /home/${cfg.user} /nix /nix/store
        echo "airlock guest diagnostics: config paths"
        find /home/${cfg.user} -maxdepth 3 -mindepth 1 -print
      '';
      serviceConfig = {
        Type = "oneshot";
        StandardOutput = "journal+console";
        StandardError = "journal+console";
      };
    };

    systemd.services.airlock-config-guest-init = lib.mkIf hasConfigRole {
      description = "Initialize Airlock config repositories in Qubes private storage";
      wantedBy = [ "multi-user.target" ];
      wants = [
        "airlock-qubes-rw-init.service"
      ]
      ++ lib.optionals hasConfigInputsImportRole [ "airlock-config-inputs-mount.service" ];
      requires = [
        "airlock-qubes-rw-init.service"
      ]
      ++ lib.optionals hasConfigInputsImportRole [ "airlock-config-inputs-mount.service" ];
      after = [
        "airlock-qubes-rw-init.service"
        "qubes-db.service"
      ]
      ++ lib.optionals hasConfigInputsImportRole [ "airlock-config-inputs-mount.service" ];
      before = [ "qubes-qrexec-agent.service" ];
      path = [
        pkgs.coreutils
        pkgs.qubes-core-qubesdb
        pkgs.util-linux
      ];
      script = ''
        set -eu

        expected_role=${lib.escapeShellArg cfg.role}
        vm_name="$(qubesdb-read /name 2>/dev/null || true)"
        qdb_role="$(qubesdb-read /features/airlock-role 2>/dev/null || true)"

        case "$qdb_role" in
          "") ;;
          "$expected_role") ;;
          sys-config-vault|sys-config-staging|sys-config-validator)
            echo "airlock config guest init: QubesDB role mismatch: expected=$expected_role actual=$qdb_role vm=$vm_name" >&2
            exit 1
            ;;
          *)
            echo "airlock config guest init: unsupported QubesDB role: expected=$expected_role actual=$qdb_role vm=$vm_name" >&2
            exit 1
            ;;
        esac

        case "$vm_name" in
          ""|"$expected_role") ;;
          sys-config-vault|sys-config-staging|sys-config-validator)
            echo "airlock config guest init: VM name mismatch: expected=$expected_role actual=$vm_name qdb_role=$qdb_role" >&2
            exit 1
            ;;
          *) ;;
        esac

        run_as_user() {
          ${pkgs.util-linux}/bin/runuser -u ${lib.escapeShellArg cfg.user} -- \
            ${pkgs.coreutils}/bin/env \
              HOME=/home/${lib.escapeShellArg cfg.user} \
              USER=${lib.escapeShellArg cfg.user} \
              LOGNAME=${lib.escapeShellArg cfg.user} \
              XDG_CACHE_HOME=/home/${lib.escapeShellArg cfg.user}/.cache \
              PATH=${lib.escapeShellArg (roleInitPathFor cfg.role)} \
              "$@"
        }

        run_as_user ${configGuestInitCommand}
      '';
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      unitConfig.OnFailure = [ "airlock-config-guest-init-diagnostics.service" ];
    };

    systemd.services.airlock-cmd-guest-init = lib.mkIf hasCmdRole {
      description = "Initialize persistent Airlock command-workspace state";
      wantedBy = [ "multi-user.target" ];
      wants = [ "airlock-qubes-rw-init.service" ];
      requires = [ "airlock-qubes-rw-init.service" ];
      after = [ "airlock-qubes-rw-init.service" ];
      before = [ "qubes-qrexec-agent.service" ];
      path = [
        pkgs.coreutils
        pkgs.util-linux
      ];
      script =
        let
          installPersistentPath =
            path:
            let
              mode = if path == cfg.codexHome then "0700" else "0755";
            in
            ''
              install -d -m ${mode} -o ${lib.escapeShellArg cfg.user} -g users ${lib.escapeShellArg path}
            '';
        in
        ''
          set -eu

          install -d -m 0700 -o ${lib.escapeShellArg cfg.user} -g users ${lib.escapeShellArg cfg.configNixStoreRoot}
        ''
        + lib.concatMapStrings installPersistentPath cmdPersistentPaths;
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
    };

    systemd.services.airlock-sys-net-networkmanager-state = lib.mkIf hasSysNetRole {
      description = "Prepare persistent NetworkManager state for sys-net";
      wants = [ "airlock-qubes-rw-init.service" ];
      requires = [ "airlock-qubes-rw-init.service" ];
      after = [ "airlock-qubes-rw-init.service" ];
      before = [ "NetworkManager.service" ];
      path = [
        pkgs.coreutils
      ];
      script = ''
        set -eu

        target=/rw/config/NM-system-connections
        link=/etc/NetworkManager/system-connections

        install -d -m 0700 "$target"
        install -d -m 0755 /etc/NetworkManager

        if [ -d "$link" ] && [ ! -L "$link" ]; then
          for entry in "$link"/*; do
            [ -e "$entry" ] || continue
            mv -n "$entry" "$target"/
          done
          rmdir "$link" 2>/dev/null || rm -rf "$link"
        elif [ -e "$link" ] && [ ! -L "$link" ]; then
          rm -rf "$link"
        fi

        ln -sfn "$target" "$link"
        chmod 0700 "$target"
      '';
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
    };

    systemd.services.NetworkManager = lib.mkIf hasSysNetRole {
      wants = [
        "airlock-sys-net-networkmanager-state.service"
        "qubes-sysinit.service"
      ];
      requires = [ "airlock-sys-net-networkmanager-state.service" ];
      after = [
        "airlock-sys-net-networkmanager-state.service"
        "qubes-sysinit.service"
      ];
      unitConfig.ConditionPathExists = "/run/qubes-service/network-manager";
    };

    systemd.services.qubes-antispoof = lib.mkIf hasNetworkProviderRole {
      description = "Qubes anti-spoofing firewall rules";
      before = [ "network-pre.target" ];
      requiredBy = [ "network-pre.target" ];
      path = [ pkgs.nftables ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${pkgs.nftables}/bin/nft -f ${qubesAgentLinux}/etc/qubes/qubes-antispoof.nft";
      };
    };

    systemd.services.qubes-iptables = lib.mkIf hasNetworkProviderRole {
      description = "Qubes base firewall settings";
      requires = [ "qubes-antispoof.service" ];
      after = [ "qubes-antispoof.service" ];
      before = [ "network-pre.target" ];
      requiredBy = [ "network-pre.target" ];
      path = networkGuestPath;
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${qubesIptablesStart}";
      };
    };

    systemd.services.qubes-network = lib.mkIf hasNetworkProviderRole {
      description = "Qubes network forwarding setup";
      wantedBy = [ "multi-user.target" ];
      wants = [
        "qubes-iptables.service"
        "qubes-sysinit.service"
      ];
      requires = [ "qubes-iptables.service" ];
      after = [
        "network-pre.target"
        "qubes-iptables.service"
        "qubes-sysinit.service"
      ];
      before = [ "network.target" ];
      path = networkGuestPath;
      unitConfig.ConditionPathExists = "/run/qubes-service/qubes-network";
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${qubesNetworkProxySetup}/bin/network-proxy-setup";
        ExecStop = "${qubesAgentLinux}/lib/qubes/init/network-proxy-stop.sh";
      };
    };

    systemd.services.qubes-firewall = lib.mkIf hasNetworkProviderRole {
      description = "Qubes firewall updater";
      wantedBy = [ "multi-user.target" ];
      wants = [ "qubes-iptables.service" ];
      after = [ "qubes-iptables.service" ];
      before = [ "qubes-network.service" ];
      path = networkGuestPath;
      unitConfig.ConditionPathExists = "/run/qubes-service/qubes-firewall";
      serviceConfig = {
        Type = "notify-reload";
        ExecStart = "${qubesAgentLinux}/bin/qubes-firewall";
      };
    };

    systemd.services.qubes-updates-proxy = lib.mkIf hasNetworkProviderRole {
      description = "Qubes updates proxy";
      wantedBy = [ "multi-user.target" ];
      after = [ "qubes-iptables.service" ];
      unitConfig.ConditionPathExists = "|/run/qubes-service/qubes-updates-proxy";
      serviceConfig = {
        ExecStart = "${pkgs.tinyproxy}/bin/tinyproxy -d -c ${tinyproxyUpdatesConfig}";
        Restart = "on-failure";
        RestartSec = "5s";
        RuntimeDirectory = "tinyproxy-updates";
      };
    };

    systemd.services."qubes-network-uplink@" = lib.mkIf hasNetworkProviderRole {
      description = "Qubes network uplink setup";
      after = [ "network-pre.target" ];
      requires = [ "network-pre.target" ];
      before = [ "network.target" ];
      wants = [ "network.target" ];
      path = networkGuestPath;
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${qubesSetupIp}/bin/setup-ip add %i";
        ExecStop = "${qubesSetupIp}/bin/setup-ip remove %i";
      };
    };

    systemd.services.qubes-network-uplink = lib.mkIf hasNetworkProviderRole {
      description = "Qubes network uplink wait";
      wantedBy = [ "multi-user.target" ];
      wants = [ "network.target" ];
      before = [ "network.target" ];
      path = networkGuestPath;
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${qubesAgentLinux}/lib/qubes/init/network-uplink-wait.sh";
      };
    };

    systemd.services.qubes-db = {
      enable = true;
      wantedBy = [ "sysinit.target" ];
      after = [
        "systemd-modules-load.service"
        "systemd-tmpfiles-setup.service"
      ];
      requires = [ "systemd-tmpfiles-setup.service" ];
      overrideStrategy = "asDropin";
      unitConfig.OnFailure = [ "airlock-qubes-db-diagnostics.service" ];
    };

    systemd.services.qubes-qrexec-agent =
      let
        roleInitDeps =
          lib.optional hasConfigRole "airlock-config-guest-init.service"
          ++ lib.optional hasCmdRole "airlock-cmd-guest-init.service";
        normalAfter = [
          "airlock-qubes-rw-init.service"
          "systemd-user-sessions.service"
        ]
        ++ roleInitDeps;
        networkProviderAfter = [
          "xendriverdomain.service"
          "airlock-qubes-rw-init.service"
        ]
        ++ roleInitDeps;
      in
      {
        enable = true;
        wantedBy = [ "multi-user.target" ];
        wants = [ "airlock-qubes-rw-init.service" ];
        requires = [
          "airlock-qubes-rw-init.service"
        ]
        ++ roleInitDeps;
        after = if hasNetworkProviderRole then lib.mkForce networkProviderAfter else normalAfter;
        before = lib.optionals hasNetworkProviderRole [ "network.target" ];
        aliases = [ "qubes-core-agent.service" ];
        overrideStrategy = "asDropin";
        unitConfig = {
          OnFailure = [ "airlock-qubes-qrexec-diagnostics.service" ];
        };
      };

    systemd.services.xendriverdomain = lib.mkIf hasNetworkProviderRole {
      enable = true;
      wantedBy = [ "multi-user.target" ];
      wants = [ "proc-xen.mount" ];
      requires = [ "proc-xen.mount" ];
      after = [
        "proc-xen.mount"
        "systemd-modules-load.service"
      ];
      path = networkGuestPath;
      overrideStrategy = "asDropin";
      serviceConfig = {
        LogsDirectory = "xen";
        RuntimeDirectory = "xen";
        RuntimeDirectoryMode = "0755";
      };
    };

    systemd.services.qubes-meminfo-writer = {
      enable = true;
      wantedBy = [ "multi-user.target" ];
      before = [ "systemd-user-sessions.service" ];
    };

    environment.etc."NetworkManager/dispatcher.d/qubes-nmhook" = lib.mkIf hasSysNetRole {
      source = qubesNetworkManagerHook;
      mode = "0555";
    };

    environment.etc."NetworkManager/dispatcher.d/30-qubes-external-ip" = lib.mkIf hasSysNetRole {
      source = qubesExternalIpHook;
      mode = "0555";
    };

    environment.etc."systemd/network/80-qubes-vif.link" = lib.mkIf hasSysNetRole {
      source = "${qubesAgentLinux}/lib/systemd/network/80-qubes-vif.link";
      mode = "0444";
    };

    environment.etc."xen" = lib.mkIf hasNetworkProviderRole {
      source = xenDriverDomainEtc;
    };

    environment.etc."qubes/qubes-antispoof.nft" = lib.mkIf hasNetworkProviderRole {
      source = "${qubesAgentLinux}/etc/qubes/qubes-antispoof.nft";
      mode = "0444";
    };

    environment.etc."qubes/qubes-ipv4.nft" = lib.mkIf hasNetworkProviderRole {
      source = "${qubesAgentLinux}/etc/qubes/qubes-ipv4.nft";
      mode = "0444";
    };

    environment.etc."qubes/qubes-ipv6.nft" = lib.mkIf hasNetworkProviderRole {
      source = "${qubesAgentLinux}/etc/qubes/qubes-ipv6.nft";
      mode = "0444";
    };

    environment.etc."qubes/qubes-ipv6-disabled.nft" = lib.mkIf hasNetworkProviderRole {
      source = "${qubesAgentLinux}/etc/qubes/qubes-ipv6-disabled.nft";
      mode = "0444";
    };

    environment.etc."qubes-rpc/qubes.UpdatesProxy" = lib.mkIf hasNetworkProviderRole {
      source = updatesProxyRpcService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/qubes.UpdatesProxy" = lib.mkIf hasNetworkProviderRole {
      text = ''
        skip-service-descriptor=true
        exit-on-service-eof=true
      '';
      mode = "0444";
    };

    environment.etc."qubes-rpc/airlock.Config.CandidateExport" = lib.mkIf hasConfigStagingRole {
      source = configCandidateExportService;
      mode = "0555";
    };

    environment.etc."qubes-rpc/airlock.Config.ImportCurrent" = lib.mkIf hasConfigStagingRole {
      source = configCurrentImportService;
      mode = "0555";
    };

    environment.etc."airlock/guest-tools/airlock-config-staging-validate" =
      lib.mkIf hasConfigStagingRole
        {
          source = configStagingValidate;
        };

    environment.etc."qubes/rpc-config/airlock.Config.CandidateExport" = lib.mkIf hasConfigStagingRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes/rpc-config/airlock.Config.ImportCurrent" = lib.mkIf hasConfigStagingRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/airlock.Config.ValidateCandidate" = lib.mkIf hasConfigValidatorRole {
      source = configCandidateValidateService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/airlock.Config.ValidateCandidate" =
      lib.mkIf hasConfigValidatorRole
        {
          text = "force-user='${cfg.user}'\n";
          mode = "0444";
        };

    environment.etc."qubes-rpc/airlock.Config.InputsImport" = lib.mkIf hasConfigInputsImportRole {
      source = configInputsImportService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/airlock.Config.InputsImport" = lib.mkIf hasConfigInputsImportRole {
      text = "force-user='root'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/airlock.Config.ArchiveCandidate" = lib.mkIf hasConfigVaultRole {
      source = configArchiveService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/airlock.Config.ArchiveCandidate" = lib.mkIf hasConfigVaultRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/airlock.Config.ArchiveDeployment" = lib.mkIf hasConfigVaultRole {
      source = configDeploymentArchiveService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/airlock.Config.ArchiveDeployment" = lib.mkIf hasConfigVaultRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/airlock.Config.ExportCurrent" = lib.mkIf hasConfigVaultRole {
      source = configCurrentExportService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/airlock.Config.ExportCurrent" = lib.mkIf hasConfigVaultRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/airlock.NixStoreRuntimeCheck" = {
      source = nixStoreRuntimeCheckService;
      mode = "0555";
    };

    environment.etc."qubes-rpc/airlock.PolicyProbe" = lib.mkIf hasConfigRole {
      source = policyProbeService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/airlock.PolicyProbe" = lib.mkIf hasConfigRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/qubes.VMShell" = {
      source = vmShellService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/qubes.VMShell" = {
      text = lib.optionalString (!cfg.allowQrexecUserSelection) "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/qubes.GetAppmenus" = {
      source = getAppmenusService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/qubes.GetAppmenus" = {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/airlock.CodexAuthImport" = lib.mkIf hasCmdRole {
      source = codexAuthImportService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/airlock.CodexAuthImport" = lib.mkIf hasCmdRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/airlock.CodexConfigImport" = lib.mkIf hasCmdRole {
      source = codexConfigImportService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/airlock.CodexConfigImport" = lib.mkIf hasCmdRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.systemPackages = [
      pkgs.qubes-core-qubesdb
      qubesAgentLinux
      pkgs.qubes-linux-utils
      pkgs.python3.pkgs.qubes-core-qrexec
      pkgs.python3.pkgs.qubes-core-qrexec.domU
    ]
    ++ (rolePackages.${cfg.role} or [ ]);

    environment.sessionVariables = lib.mkIf hasCmdRole {
      CODEX_HOME = cfg.codexHome;
    };

    nix.settings = lib.mkIf hasPrivateNixRole {
      allowed-users = [ cfg.user ];
      experimental-features = [
        "nix-command"
        "flakes"
      ];
    };

    services.timesyncd.enable = false;
    system.stateVersion = "26.05";
  };
}
