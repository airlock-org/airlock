{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.nixome.qubeGuest;
  hasConfigVaultRole = cfg.role == "config-vault" || cfg.role == "config-system";
  hasConfigModRole = cfg.role == "config-mod" || cfg.role == "config-system";
  qubesAgentLinux = pkgs.qubes-core-agent-linux;

  qubesGuestPath = [
    pkgs.coreutils
    pkgs.diffutils
    pkgs.e2fsprogs
    pkgs.findutils
    pkgs.gawk
    pkgs.gnugrep
    pkgs.kmod
    pkgs.qubes-core-qubesdb
    pkgs.qubes-linux-utils
    pkgs.systemd
    pkgs.util-linux
  ];

  guestDiagnosticsPath = [
    pkgs.coreutils
    pkgs.gnugrep
    pkgs.kmod
    pkgs.systemd
    pkgs.util-linux
  ];

  rolePackages = rec {
    base = [ ];
    config-vault = [
      configVaultInit
      configVaultArchiveCandidate
      pkgs.gawk
      pkgs.git
      pkgs.gnutar
      pkgs.jq
    ];
    config-mod = [
      configModExportCandidate
      configModInit
      configModValidate
      pkgs.gawk
      pkgs.git
      pkgs.gnutar
      pkgs.jq
      pkgs.nix
    ];
    config-system = config-vault ++ config-mod;
  };

  configVaultInit = pkgs.writeShellScriptBin "nixome-config-vault-init" ''
        set -eu

        repo="${cfg.configArchivePath}"
        if [ -d "$repo/.git" ]; then
          printf 'config archive already exists: %s\n' "$repo"
          exit 0
        fi

        mkdir -p "$repo"
        cd "$repo"
        ${pkgs.git}/bin/git init -b main
        mkdir -p candidates deployments
        : > candidates/.gitkeep
        : > deployments/.gitkeep
        cat > README.md <<'EOF'
    # Nixome Configuration Vault

    This repository archives Nixome configuration candidates and deployment
    records written by dom0.
    EOF
        ${pkgs.git}/bin/git add README.md candidates/.gitkeep deployments/.gitkeep
        ${pkgs.git}/bin/git \
          -c user.name='Nixome Config Vault' \
          -c user.email='nixome-config-vault@localhost' \
          commit -m 'init nixome config archive'
  '';

  configModInit = pkgs.writeShellScriptBin "nixome-config-mod-init" ''
        set -eu

        repo="${cfg.configRepoPath}"
        if [ -d "$repo/.git" ]; then
          printf 'config mod repo already exists: %s\n' "$repo"
          exit 0
        fi

        mkdir -p "$repo"
        cd "$repo"
        ${pkgs.git}/bin/git init -b main
        cat > README.md <<'EOF'
    # Nixome Machine Configuration

    This is the editable Nixome configuration candidate. dom0 archives a
    validated commit into config-vault before attempting deployment.
    EOF
        cat > flake.nix <<'EOF'
    {
      description = "Nixome machine configuration candidate";

      outputs = { self }: {
        nixome.metadata = {
          schema = "nixome.config-repo.v1";
          purpose = "candidate";
        };
      };
    }
    EOF
        ${pkgs.git}/bin/git add README.md flake.nix
        ${pkgs.git}/bin/git \
          -c user.name='Nixome Config Mod' \
          -c user.email='nixome-config-mod@localhost' \
          commit -m 'init nixome config candidate'
  '';

  configModValidate = pkgs.writeShellScriptBin "nixome-config-mod-validate" ''
    set -eu

    repo="${cfg.configRepoPath}"
    validation_dir="${cfg.configValidationPath}"

    if [ ! -d "$repo/.git" ]; then
      printf 'missing Git repo at %s; run nixome-config-mod-init first\n' "$repo" >&2
      exit 1
    fi

    mkdir -p "$validation_dir/logs"
    cd "$repo"

    commit="$(${pkgs.git}/bin/git rev-parse HEAD)"
    result_file="$validation_dir/$commit.json"
    result_tmp="$result_file.tmp"
    flake_log="$validation_dir/logs/flake-check-$commit.log"
    dirty="$(${pkgs.git}/bin/git status --porcelain)"

    write_validation() {
      status="$1"
      reason="$2"
      flake_status="$3"

      ${pkgs.jq}/bin/jq -c -n \
        --arg schema "nixome.config-validation.v1" \
        --arg status "$status" \
        --arg reason "$reason" \
        --arg repo "$repo" \
        --arg commit "$commit" \
        --arg dirty "$dirty" \
        --arg flake_status "$flake_status" \
        --arg flake_log "$flake_log" \
        '{
          schema: $schema,
          status: $status,
          reason: $reason,
          repo: $repo,
          commit: $commit,
          checks: [
            {
              name: "git-clean",
              status: (if $dirty == "" then "passed" else "failed" end),
              details: $dirty
            },
            {
              name: "nix-flake-check-no-build",
              status: $flake_status,
              log: $flake_log
            }
          ]
        }' > "$result_tmp"
      mv "$result_tmp" "$result_file"
      cat "$result_file"
    }

    if [ -n "$dirty" ]; then
      write_validation failed "config mod worktree is dirty; commit changes before export" skipped
      exit 1
    fi

    flake_status=skipped
    reason="validation passed"
    status=passed
    if [ -f flake.nix ]; then
      if ${pkgs.nix}/bin/nix \
        --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        flake check --no-build . > "$flake_log" 2>&1; then
        flake_status=passed
      else
        flake_status=failed
        reason="nix flake check --no-build failed"
        status=failed
      fi
    else
      printf 'no flake.nix present; skipping nix flake check\n' > "$flake_log"
    fi

    write_validation "$status" "$reason" "$flake_status"
    [ "$status" = passed ]
  '';

  configModExportCandidate = pkgs.writeShellScriptBin "nixome-config-mod-export-candidate" ''
    set -eu

    repo="${cfg.configRepoPath}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    ${configModValidate}/bin/nixome-config-mod-validate > "$tmpdir/validation.json"
    status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/validation.json")"
    commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/validation.json")"

    if [ "$status" != passed ]; then
      cat "$tmpdir/validation.json" >&2
      exit 1
    fi

    cd "$repo"
    head_commit="$(${pkgs.git}/bin/git rev-parse HEAD)"
    if [ "$head_commit" != "$commit" ]; then
      printf 'validation commit %s does not match HEAD %s\n' "$commit" "$head_commit" >&2
      exit 1
    fi

    ${pkgs.git}/bin/git bundle create "$tmpdir/config.bundle" HEAD >/dev/null
    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "nixome.config-candidate.v1" \
      --arg commit "$commit" \
      --arg validation_schema "$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/validation.json")" \
      '{schema: $schema, commit: $commit, validation_schema: $validation_schema}' \
      > "$tmpdir/manifest.json"

    exec ${pkgs.gnutar}/bin/tar -C "$tmpdir" -cf - config.bundle validation.json manifest.json
  '';

  configCandidateExportService = pkgs.writeShellScript "nixome-config-candidate-export-qrexec" ''
    exec ${configModExportCandidate}/bin/nixome-config-mod-export-candidate
  '';

  configVaultArchiveCandidate = pkgs.writeShellScriptBin "nixome-config-vault-archive-candidate" ''
    set -eu

    repo="${cfg.configArchivePath}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    candidate_tar="$tmpdir/candidate.tar"
    ${pkgs.coreutils}/bin/cat > "$candidate_tar"
    ${pkgs.gnutar}/bin/tar -tf "$candidate_tar" |
      ${pkgs.gawk}/bin/awk '
        $0 != "config.bundle" && $0 != "validation.json" && $0 != "manifest.json" {
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

    validation_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/validation.json")"
    validation_status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/validation.json")"
    validation_commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/validation.json")"
    [ "$validation_schema" = "nixome.config-validation.v1" ] || {
      printf 'unsupported validation schema: %s\n' "$validation_schema" >&2
      exit 1
    }
    [ "$validation_status" = passed ] || {
      printf 'validation status is not passed: %s\n' "$validation_status" >&2
      exit 1
    }

    verify_repo="$tmpdir/verify.git"
    ${pkgs.git}/bin/git init --bare -b main "$verify_repo" >/dev/null
    ${pkgs.git}/bin/git -C "$verify_repo" bundle verify "$tmpdir/config.bundle" > "$tmpdir/bundle-verify.log" 2>&1
    candidate_commit="$(
      ${pkgs.git}/bin/git -C "$verify_repo" bundle list-heads "$tmpdir/config.bundle" |
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

    if [ ! -d "$repo/.git" ]; then
      ${configVaultInit}/bin/nixome-config-vault-init >/dev/null
    fi

    archive_dir="$repo/candidates/$candidate_commit"
    ${pkgs.coreutils}/bin/mkdir -p "$archive_dir"
    ${pkgs.coreutils}/bin/cp "$tmpdir/config.bundle" "$archive_dir/config.bundle"
    ${pkgs.coreutils}/bin/cp "$tmpdir/validation.json" "$archive_dir/validation.json"
    ${pkgs.coreutils}/bin/cp "$tmpdir/bundle-verify.log" "$archive_dir/bundle-verify.log"
    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "nixome.config-candidate-archive-record.v1" \
      --arg candidate_commit "$candidate_commit" \
      --arg validation_schema "$validation_schema" \
      --arg source "dom0" \
      '{
        schema: $schema,
        candidate_commit: $candidate_commit,
        validation_schema: $validation_schema,
        archived_by: $source
      }' > "$archive_dir/metadata.json"
    printf '%s\n' "$candidate_commit" > "$repo/latest-candidate"

    cd "$repo"
    ${pkgs.git}/bin/git add "candidates/$candidate_commit" latest-candidate
    archive_status=current
    if ! ${pkgs.git}/bin/git diff --cached --quiet; then
      ${pkgs.git}/bin/git \
        -c user.name='Nixome dom0 archive' \
        -c user.email='nixome-dom0@localhost' \
        commit -m "archive candidate ''${candidate_commit:0:12}" >/dev/null
      archive_status=archived
    fi

    vault_commit="$(${pkgs.git}/bin/git rev-parse HEAD)"
    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "nixome.config-vault-archive.v1" \
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

  configArchiveService = pkgs.writeShellScript "nixome-config-archive-candidate-qrexec" ''
    exec ${configVaultArchiveCandidate}/bin/nixome-config-vault-archive-candidate
  '';

  vmShellService = pkgs.writeShellScript "nixome-qubes-vmshell" ''
    export PATH=/run/current-system/sw/bin:/run/wrappers/bin:/bin:/usr/bin
    exec ${pkgs.runtimeShell}
  '';

in
{
  options.nixome.qubeGuest = {
    enable = lib.mkEnableOption "a Nixome NixOS Qubes guest template";

    role = lib.mkOption {
      type = lib.types.enum [
        "base"
        "config-vault"
        "config-mod"
        "config-system"
      ];
      default = "base";
      description = "Nixome role-specific userspace to place in the guest template.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "user";
      description = "Primary unprivileged user in the guest template.";
    };

    configRepoPath = lib.mkOption {
      type = lib.types.str;
      default = "/home/${cfg.user}/nixome-config";
      description = "Editable config repository path in the config-mod role.";
    };

    configValidationPath = lib.mkOption {
      type = lib.types.str;
      default = "/home/${cfg.user}/nixome-config-validation";
      description = "Directory for config-mod validation results.";
    };

    configArchivePath = lib.mkOption {
      type = lib.types.str;
      default = "/home/${cfg.user}/nixome-config-vault";
      description = "Archived candidate repository path in the config-vault role.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.user != "";
        message = "nixome.qubeGuest.user must not be empty.";
      }
      {
        assertion = lib.hasPrefix "/home/" cfg.configRepoPath;
        message = "nixome.qubeGuest.configRepoPath should live under /home for Qubes private storage.";
      }
      {
        assertion = lib.hasPrefix "/home/" cfg.configValidationPath;
        message = "nixome.qubeGuest.configValidationPath should live under /home for Qubes private storage.";
      }
      {
        assertion = lib.hasPrefix "/home/" cfg.configArchivePath;
        message = "nixome.qubeGuest.configArchivePath should live under /home for Qubes private storage.";
      }
    ];

    boot.loader.grub.enable = false;
    boot.initrd.systemd.enable = lib.mkDefault true;
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
    ];
    boot.kernelParams = [
      "console=hvc0"
    ];

    fileSystems."/" = {
      device = "/dev/mapper/dmroot";
      fsType = "ext4";
    };

    networking.hostName = lib.mkDefault "nixome-qube";
    networking.useDHCP = lib.mkDefault true;

    users.mutableUsers = lib.mkDefault false;
    users.allowNoPasswordLogin = lib.mkDefault true;
    users.groups.qubes = { };
    users.users.${cfg.user} = {
      isNormalUser = true;
      group = "users";
      extraGroups = [ "qubes" ];
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

    systemd.tmpfiles.settings."10-nixome-qubes-guest" = {
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
    ];

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

    systemd.services.nixome-qubes-db-diagnostics = {
      description = "Print QubesDB guest diagnostics to the console";
      path = guestDiagnosticsPath;
      script = ''
        set +e
        echo "nixome guest diagnostics: qubes-db.service failed"
        systemctl --no-pager status qubes-db.service
        journalctl -b --no-pager -u qubes-db.service -n 120
        echo "nixome guest diagnostics: Xen device nodes"
        ls -l /dev/xen /proc/xen
        echo "nixome guest diagnostics: loaded Xen modules"
        lsmod | grep -E '(^xen|evtchn|gnt|privcmd)'
      '';
      serviceConfig = {
        Type = "oneshot";
        StandardOutput = "journal+console";
        StandardError = "journal+console";
      };
    };

    systemd.services.nixome-qubes-qrexec-diagnostics = {
      description = "Print qrexec guest diagnostics to the console";
      path = guestDiagnosticsPath;
      script = ''
        set +e
        echo "nixome guest diagnostics: qubes-qrexec-agent.service failed"
        systemctl --no-pager status qubes-qrexec-agent.service
        journalctl -b --no-pager -u qubes-qrexec-agent.service -n 120
        echo "nixome guest diagnostics: QubesDB service state"
        systemctl --no-pager status qubes-db.service
      '';
      serviceConfig = {
        Type = "oneshot";
        StandardOutput = "journal+console";
        StandardError = "journal+console";
      };
    };

    systemd.services.nixome-qubes-rw-init-diagnostics = {
      description = "Print Qubes private storage diagnostics to the console";
      path = guestDiagnosticsPath;
      script = ''
        set +e
        echo "nixome guest diagnostics: nixome-qubes-rw-init.service failed"
        systemctl --no-pager status nixome-qubes-rw-init.service
        journalctl -b --no-pager -u nixome-qubes-rw-init.service -n 120
        echo "nixome guest diagnostics: filesystem state"
        ls -ld /rw /home /usr/local /var /var/run /run/qubes /var/lib /var/lib/qubes
        findmnt / /rw /home /usr/local
        echo "nixome guest diagnostics: block devices"
        lsblk -o NAME,MAJ:MIN,SIZE,FSTYPE,MOUNTPOINTS
        blkid /dev/xvdb
      '';
      serviceConfig = {
        Type = "oneshot";
        StandardOutput = "journal+console";
        StandardError = "journal+console";
      };
    };

    systemd.services.nixome-qubes-rw-init = {
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

        mkdir -p /rw/usrlocal
        if ! mountpoint -q /usr/local; then
          mount --bind /rw/usrlocal /usr/local
        fi
      '';
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
      };
      unitConfig.OnFailure = [ "nixome-qubes-rw-init-diagnostics.service" ];
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
      unitConfig.OnFailure = [ "nixome-qubes-db-diagnostics.service" ];
    };

    systemd.services.qubes-qrexec-agent = {
      enable = true;
      wantedBy = [ "multi-user.target" ];
      wants = [ "nixome-qubes-rw-init.service" ];
      requires = [ "nixome-qubes-rw-init.service" ];
      after = [
        "nixome-qubes-rw-init.service"
        "systemd-user-sessions.service"
      ];
      aliases = [ "qubes-core-agent.service" ];
      overrideStrategy = "asDropin";
      unitConfig.OnFailure = [ "nixome-qubes-qrexec-diagnostics.service" ];
    };

    systemd.services.qubes-meminfo-writer = {
      enable = true;
      wantedBy = [ "multi-user.target" ];
      before = [ "systemd-user-sessions.service" ];
    };

    environment.etc."qubes-rpc/nixome.Config.CandidateExport" = lib.mkIf hasConfigModRole {
      source = configCandidateExportService;
      mode = "0555";
    };

    environment.etc."qubes-rpc/nixome.Config.ArchiveCandidate" = lib.mkIf hasConfigVaultRole {
      source = configArchiveService;
      mode = "0555";
    };

    environment.etc."qubes-rpc/qubes.VMShell" = {
      source = vmShellService;
      mode = "0555";
    };

    environment.systemPackages = [
      pkgs.qubes-core-qubesdb
      qubesAgentLinux
      pkgs.qubes-linux-utils
      pkgs.python3.pkgs.qubes-core-qrexec
      pkgs.python3.pkgs.qubes-core-qrexec.domU
    ]
    ++ (rolePackages.${cfg.role} or [ ]);

    nix.settings = lib.mkIf hasConfigModRole {
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
