{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.nixome.qubeGuest;
  hasConfigVaultRole = cfg.role == "config-vault" || cfg.role == "config-system";
  hasConfigStagingRole = cfg.role == "config-staging" || cfg.role == "config-system";
  hasConfigValidatorRole = cfg.role == "config-validator" || cfg.role == "config-system";
  hasConfigRole = hasConfigVaultRole || hasConfigStagingRole || hasConfigValidatorRole;
  qubesAgentLinux = pkgs.qubes-core-agent-linux;

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

  roleInitPath = lib.makeBinPath [
    pkgs.coreutils
    pkgs.gawk
    pkgs.git
    pkgs.gnutar
    pkgs.jq
    pkgs.nix
    pkgs.qubes-core-qubesdb
    pkgs.util-linux
  ];

  rolePackages = rec {
    base = [ ];
    config-vault = [
      configVaultInit
      configVaultArchiveCandidate
      configVaultArchiveDeployment
      pkgs.gawk
      pkgs.git
      pkgs.gnutar
      pkgs.jq
    ];
    config-staging = [
      configStagingExportCandidate
      configStagingInit
      configStagingValidate
      pkgs.gawk
      pkgs.git
      pkgs.gnutar
      pkgs.jq
      pkgs.nix
    ];
    config-validator = [
      configValidatorValidateCandidate
      configValidatorInit
      pkgs.gawk
      pkgs.git
      pkgs.gnutar
      pkgs.jq
      pkgs.nix
    ];
    config-system = config-vault ++ config-staging ++ config-validator;
  };

  configStagingSeedFlake = builtins.readFile ../seeds/config-staging/flake.nix;
  configStagingTopologySchema = builtins.readFile ../lib/nixome/topology.nix;

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

  configStagingInit = pkgs.writeShellScriptBin "nixome-config-staging-init" ''
        set -eu

        repo="${cfg.configRepoPath}"
        if [ -d "$repo/.git" ]; then
          printf 'config staging repo already exists: %s\n' "$repo"
          exit 0
        fi

        mkdir -p "$repo"
        cd "$repo"
        ${pkgs.git}/bin/git init -b main
        mkdir -p nixome
        cat > README.md <<'EOF'
    # Nixome Machine Configuration

    This is the editable Nixome configuration candidate. dom0 archives a
    validated commit into config-vault before attempting deployment.
    EOF
        cat > flake.nix <<'EOF'
    ${configStagingSeedFlake}
    EOF
        cat > nixome/topology.nix <<'EOF'
    ${configStagingTopologySchema}
    EOF
        ${pkgs.git}/bin/git add README.md flake.nix nixome/topology.nix
        ${pkgs.git}/bin/git \
          -c user.name='Nixome Config Staging' \
          -c user.email='nixome-config-staging@localhost' \
          commit -m 'init nixome config candidate'
  '';

  configStagingValidate = pkgs.writeShellScriptBin "nixome-config-staging-validate" ''
    set -eu

    repo="${cfg.configRepoPath}"
    validation_dir="${cfg.configValidationPath}"

    if [ ! -d "$repo/.git" ]; then
      printf 'missing Git repo at %s; run nixome-config-staging-init first\n' "$repo" >&2
      exit 1
    fi

    mkdir -p "$validation_dir/logs"
    cd "$repo"

    commit="$(${pkgs.git}/bin/git rev-parse HEAD)"
    result_file="$validation_dir/$commit.json"
    result_tmp="$result_file.tmp"
    flake_log="$validation_dir/logs/flake-check-$commit.log"
    topology_json="$validation_dir/topology-$commit.json"
    topology_eval_log="$validation_dir/logs/nix-eval-topology-$commit.log"
    topology_log="$validation_dir/logs/topology-json-$commit.log"
    dirty="$(${pkgs.git}/bin/git status --porcelain)"

    write_validation() {
      status="$1"
      reason="$2"
      topology_eval_status="$3"
      topology_status="$4"
      flake_status="$5"

      ${pkgs.jq}/bin/jq -c -n \
        --arg schema "nixome.config-validation.v1" \
        --arg status "$status" \
        --arg reason "$reason" \
        --arg repo "$repo" \
        --arg commit "$commit" \
        --arg dirty "$dirty" \
        --arg topology_eval_status "$topology_eval_status" \
        --arg topology_status "$topology_status" \
        --arg topology_json "$topology_json" \
        --arg topology_eval_log "$topology_eval_log" \
        --arg topology_log "$topology_log" \
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
              name: "nix-eval-nixome-topology",
              status: $topology_eval_status,
              attr: "nixome.topology",
              output: $topology_json,
              log: $topology_eval_log
            },
            {
              name: "nixome-topology-json",
              status: $topology_status,
              path: $topology_json,
              log: $topology_log
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
      write_validation failed "config staging worktree is dirty; commit changes before export" skipped skipped skipped
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
        mark_failed "nix flake check --no-build failed"
      fi

      if ${pkgs.nix}/bin/nix \
        --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        eval .#nixome.topology --json > "$topology_json" 2> "$topology_eval_log"; then
        topology_eval_status=passed
      else
        topology_eval_status=failed
        : > "$topology_json"
        mark_failed "nix eval .#nixome.topology failed"
      fi

      if [ "$topology_eval_status" = passed ]; then
        if ${pkgs.jq}/bin/jq -e '
          .schema == "nixome.qubes-topology.v1" and
          (.templates | type == "object") and
          (.kernels | type == "object") and
          (.prefs | type == "object") and
          (.domains | type == "object")
        ' "$topology_json" > "$topology_log" 2>&1; then
          topology_status=passed
        else
          topology_status=failed
          mark_failed "evaluated nixome.topology JSON validation failed"
        fi
      else
        printf 'skipped because nix eval .#nixome.topology did not pass\n' > "$topology_log"
      fi
    else
      printf 'no flake.nix present; cannot evaluate nixome.topology\n' > "$flake_log"
      printf 'no flake.nix present; cannot evaluate nixome.topology\n' > "$topology_eval_log"
      printf 'skipped because flake.nix is missing\n' > "$topology_log"
      flake_status=failed
      mark_failed "flake.nix is missing"
    fi

    write_validation "$status" "$reason" "$topology_eval_status" "$topology_status" "$flake_status"
    [ "$status" = passed ]
  '';

  configStagingExportCandidate = pkgs.writeShellScriptBin "nixome-config-staging-export-candidate" ''
    set -eu

    repo="${cfg.configRepoPath}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    if [ ! -d "$repo/.git" ]; then
      printf 'missing Git repo at %s; run nixome-config-staging-init first\n' "$repo" >&2
      exit 1
    fi

    cd "$repo"
    dirty="$(${pkgs.git}/bin/git status --porcelain)"
    if [ -n "$dirty" ]; then
      printf 'config staging worktree is dirty; commit changes before export\n' >&2
      printf '%s\n' "$dirty" >&2
      exit 1
    fi

    commit="$(${pkgs.git}/bin/git rev-parse HEAD)"
    branch="$(${pkgs.git}/bin/git symbolic-ref --short HEAD)"
    ${pkgs.git}/bin/git bundle create "$tmpdir/config.bundle" HEAD "refs/heads/$branch" >/dev/null
    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "nixome.config-candidate.v1" \
      --arg commit "$commit" \
      --arg branch "$branch" \
      --arg source_role "config-staging" \
      '{schema: $schema, commit: $commit, branch: $branch, source_role: $source_role}' \
      > "$tmpdir/manifest.json"

    exec ${pkgs.gnutar}/bin/tar -C "$tmpdir" -cf - config.bundle manifest.json
  '';

  configCandidateExportService = pkgs.writeShellScript "nixome-config-candidate-export-qrexec" ''
    exec ${configStagingExportCandidate}/bin/nixome-config-staging-export-candidate
  '';

  configValidatorInit = pkgs.writeShellScriptBin "nixome-config-validator-init" ''
    set -eu

    validation_dir="${cfg.configValidationPath}"
    mkdir -p "$validation_dir/logs"
    printf 'config validator workspace ready: %s\n' "$validation_dir"
  '';

  configValidatorValidateCandidate = pkgs.writeShellScriptBin "nixome-config-validator-validate-candidate" ''
    set -eu

    validation_dir="${cfg.configValidationPath}"
    tmpdir="$(${pkgs.coreutils}/bin/mktemp -d)"
    trap '${pkgs.coreutils}/bin/rm -rf "$tmpdir"' EXIT

    candidate_tar="$tmpdir/candidate.tar"
    ${pkgs.coreutils}/bin/cat > "$candidate_tar"
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
    [ "$manifest_schema" = "nixome.config-candidate.v1" ] || {
      printf 'unsupported candidate manifest schema: %s\n' "$manifest_schema" >&2
      exit 1
    }
    [ -n "$manifest_commit" ] && [ "$manifest_commit" != null ] || {
      printf 'candidate manifest is missing commit\n' >&2
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
    [ "$candidate_commit" = "$manifest_commit" ] || {
      printf 'candidate commit %s does not match manifest commit %s\n' "$candidate_commit" "$manifest_commit" >&2
      exit 1
    }

    worktree="$tmpdir/worktree"
    ${pkgs.git}/bin/git clone "$tmpdir/config.bundle" "$worktree" > "$tmpdir/git-clone.log" 2>&1
    cd "$worktree"
    checked_commit="$(${pkgs.git}/bin/git rev-parse HEAD)"
    dirty="$(${pkgs.git}/bin/git status --porcelain)"
    [ "$checked_commit" = "$candidate_commit" ] || {
      printf 'cloned commit %s does not match candidate commit %s\n' "$checked_commit" "$candidate_commit" >&2
      exit 1
    }

    mkdir -p "$validation_dir/logs"
    result_file="$validation_dir/$candidate_commit.json"
    result_tmp="$result_file.tmp"
    flake_log="$validation_dir/logs/flake-check-$candidate_commit.log"
    topology_eval_log="$validation_dir/logs/nix-eval-topology-$candidate_commit.log"
    topology_log="$validation_dir/logs/topology-json-$candidate_commit.log"
    local_flake_log="$tmpdir/flake-check.log"
    local_topology_eval_log="$tmpdir/nix-eval-topology.log"
    local_topology_log="$tmpdir/topology-json.log"
    generated_topology="$tmpdir/topology.json"

    write_validation() {
      status="$1"
      reason="$2"
      topology_eval_status="$3"
      topology_status="$4"
      topology_path="$5"
      topology_eval_log_path="$6"
      topology_log_path="$7"
      flake_status="$8"

      ${pkgs.jq}/bin/jq -c -n \
        --arg schema "nixome.config-validation.v1" \
        --arg status "$status" \
        --arg reason "$reason" \
        --arg validator_role "config-validator" \
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
        --arg flake_status "$flake_status" \
        --arg flake_log "$flake_log" \
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
              name: "nix-eval-nixome-topology",
              status: $topology_eval_status,
              attr: "nixome.topology",
              output: $topology_path,
              log: $topology_eval_log
            },
            {
              name: "nixome-topology-json",
              status: $topology_status,
              path: $topology_path,
              log: $topology_log
            },
            {
              name: "nix-flake-check-no-build",
              status: $flake_status,
              log: $flake_log
            }
          ]
        }' > "$result_tmp"
      mv "$result_tmp" "$result_file"
      ${pkgs.coreutils}/bin/cp "$result_file" "$tmpdir/validation.json"
    }

    if [ -n "$dirty" ]; then
      write_validation failed "validated candidate worktree is dirty" skipped skipped "topology.json" "$topology_eval_log" "$topology_log" skipped
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
    flake_status=skipped
    reason="validation passed"
    status=passed

    if [ -f flake.nix ]; then
      if ${pkgs.nix}/bin/nix \
        --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        flake check --no-build . > "$local_flake_log" 2>&1; then
        flake_status=passed
      else
        flake_status=failed
        mark_failed "nix flake check --no-build failed"
      fi
      ${pkgs.coreutils}/bin/cp "$local_flake_log" "$flake_log"
    else
      printf 'no flake.nix present; cannot evaluate nixome.topology\n' > "$flake_log"
      flake_status=failed
      mark_failed "flake.nix is missing"
    fi

    if [ -f flake.nix ]; then
      if ${pkgs.nix}/bin/nix \
        --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        eval .#nixome.topology --json > "$generated_topology" 2> "$local_topology_eval_log"; then
        topology_eval_status=passed
      else
        topology_eval_status=failed
        : > "$generated_topology"
        mark_failed "nix eval .#nixome.topology failed"
      fi
      ${pkgs.coreutils}/bin/cp "$local_topology_eval_log" "$topology_eval_log"
    else
      printf 'no flake.nix present; cannot evaluate nixome.topology\n' > "$topology_eval_log"
    fi

    if [ "$topology_eval_status" = passed ]; then
      if ${pkgs.jq}/bin/jq -e '
        .schema == "nixome.qubes-topology.v1" and
        (.templates | type == "object") and
        (.kernels | type == "object") and
        (.prefs | type == "object") and
        (.domains | type == "object")
      ' "$generated_topology" > "$local_topology_log" 2>&1; then
        topology_status=passed
      else
        topology_status=failed
        mark_failed "evaluated nixome.topology JSON validation failed"
      fi
      ${pkgs.coreutils}/bin/cp "$local_topology_log" "$topology_log"
    else
      printf 'skipped because nix eval .#nixome.topology did not pass\n' > "$topology_log"
    fi

    write_validation "$status" "$reason" "$topology_eval_status" "$topology_status" "topology.json" "$topology_eval_log" "$topology_log" "$flake_status"
    if [ "$status" != passed ]; then
      cat "$tmpdir/validation.json" >&2
      exit 1
    fi

    exec ${pkgs.gnutar}/bin/tar -C "$tmpdir" -cf - config.bundle validation.json manifest.json topology.json
  '';

  configCandidateValidateService = pkgs.writeShellScript "nixome-config-candidate-validate-qrexec" ''
    exec ${configValidatorValidateCandidate}/bin/nixome-config-validator-validate-candidate
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
            $0 != "config.bundle" && $0 != "validation.json" && $0 != "manifest.json" && $0 != "topology.json" {
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

        manifest_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/manifest.json")"
        manifest_commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/manifest.json")"
        validation_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/validation.json")"
        validation_status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/validation.json")"
        validation_commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/validation.json")"
        [ "$manifest_schema" = "nixome.config-candidate.v1" ] || {
          printf 'unsupported candidate manifest schema: %s\n' "$manifest_schema" >&2
          exit 1
        }
        [ "$validation_schema" = "nixome.config-validation.v1" ] || {
          printf 'unsupported validation schema: %s\n' "$validation_schema" >&2
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

        snapshot_tmp="$tmpdir/snapshot"
        ${pkgs.git}/bin/git clone "$tmpdir/config.bundle" "$snapshot_tmp" > "$tmpdir/snapshot-git-clone.log" 2>&1
        ${pkgs.git}/bin/git -C "$snapshot_tmp" checkout --detach "$candidate_commit" > "$tmpdir/snapshot-git-checkout.log" 2>&1
        ${pkgs.coreutils}/bin/rm -rf "$snapshot_tmp/.git"

        if [ ! -d "$repo/.git" ]; then
          ${configVaultInit}/bin/nixome-config-vault-init >/dev/null
        fi

        archive_dir="$repo/candidates/$candidate_commit"
        snapshot_dir="$archive_dir/snapshot"
        ${pkgs.coreutils}/bin/mkdir -p "$archive_dir"
        ${pkgs.coreutils}/bin/cp "$tmpdir/config.bundle" "$archive_dir/config.bundle"
        ${pkgs.coreutils}/bin/cp "$tmpdir/manifest.json" "$archive_dir/manifest.json"
        ${pkgs.coreutils}/bin/cp "$tmpdir/validation.json" "$archive_dir/validation.json"
        ${pkgs.coreutils}/bin/cp "$tmpdir/topology.json" "$archive_dir/topology.json"
        ${pkgs.coreutils}/bin/cp "$tmpdir/bundle-verify.log" "$archive_dir/bundle-verify.log"
        ${pkgs.coreutils}/bin/cp "$tmpdir/snapshot-git-clone.log" "$archive_dir/snapshot-git-clone.log"
        ${pkgs.coreutils}/bin/cp "$tmpdir/snapshot-git-checkout.log" "$archive_dir/snapshot-git-checkout.log"
        ${pkgs.coreutils}/bin/rm -rf "$snapshot_dir"
        ${pkgs.coreutils}/bin/cp -a "$snapshot_tmp" "$snapshot_dir"
        ${pkgs.jq}/bin/jq -c -n \
          --arg schema "nixome.config-candidate-archive-record.v1" \
          --arg candidate_commit "$candidate_commit" \
          --arg manifest_schema "$manifest_schema" \
          --arg validation_schema "$validation_schema" \
          --arg snapshot "$archive_dir/snapshot" \
          --arg topology "$archive_dir/topology.json" \
          --arg source "dom0" \
          '{
            schema: $schema,
            candidate_commit: $candidate_commit,
            manifest_schema: $manifest_schema,
            validation_schema: $validation_schema,
            snapshot: $snapshot,
            topology: $topology,
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

  configVaultArchiveDeployment = pkgs.writeShellScriptBin "nixome-config-vault-archive-deployment" ''
    set -eu

    repo="${cfg.configArchivePath}"
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
        $0 != "validation.json" &&
        $0 != "manifest.json" &&
        $0 != "logs/apply.commands.log" &&
        $0 != "logs/archive.stdout.log" &&
        $0 != "logs/archive.stderr.log" &&
        $0 != "logs/setup.stdout.log" &&
        $0 != "logs/setup.stderr.log" &&
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
    validation_schema="$(${pkgs.jq}/bin/jq -r '.schema' "$tmpdir/validation.json")"
    validation_status="$(${pkgs.jq}/bin/jq -r '.status' "$tmpdir/validation.json")"
    validation_commit="$(${pkgs.jq}/bin/jq -r '.commit' "$tmpdir/validation.json")"

    [ "$deployment_schema" = "nixome.config-deployment.v1" ] || {
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
    [ "$apply_schema" = "nixome.config-apply.v1" ] || {
      printf 'unsupported apply schema: %s\n' "$apply_schema" >&2
      exit 1
    }
    [ "$archive_schema" = "nixome.config-archive.v1" ] || {
      printf 'unsupported archive schema: %s\n' "$archive_schema" >&2
      exit 1
    }
    [ "$archive_status" = passed ] || {
      printf 'candidate archive did not pass: %s\n' "$archive_status" >&2
      exit 1
    }
    [ "$setup_schema" = "nixome.setup-config.v1" ] || {
      printf 'unsupported setup schema: %s\n' "$setup_schema" >&2
      exit 1
    }
    [ "$topology_schema" = "nixome.qubes-topology.v1" ] || {
      printf 'unsupported topology schema: %s\n' "$topology_schema" >&2
      exit 1
    }
    [ "$validation_schema" = "nixome.config-validation.v1" ] || {
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
      ${configVaultInit}/bin/nixome-config-vault-init >/dev/null
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
      clone.log \
      checkout.log; do
      ${pkgs.coreutils}/bin/cp "$tmpdir/logs/$log" "$deployment_dir/logs/$log"
    done

    printf '%s\n' "$deployment_id" > "$repo/latest-deployment-attempt"
    if [ "$deployment_status" = passed ]; then
      printf '%s\n' "$deployment_id" > "$repo/latest-deployment"
    fi

    cd "$repo"
    ${pkgs.git}/bin/git add "deployments/$deployment_id" latest-deployment-attempt
    if [ "$deployment_status" = passed ]; then
      ${pkgs.git}/bin/git add latest-deployment
    fi
    ${pkgs.git}/bin/git \
      -c user.name='Nixome dom0 archive' \
      -c user.email='nixome-dom0@localhost' \
      commit -m "record deployment $deployment_id" >/dev/null

    vault_commit="$(${pkgs.git}/bin/git rev-parse HEAD)"
    ${pkgs.jq}/bin/jq -c -n \
      --arg schema "nixome.config-vault-deployment.v1" \
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

  configArchiveService = pkgs.writeShellScript "nixome-config-archive-candidate-qrexec" ''
    exec ${configVaultArchiveCandidate}/bin/nixome-config-vault-archive-candidate
  '';

  configDeploymentArchiveService = pkgs.writeShellScript "nixome-config-archive-deployment-qrexec" ''
    exec ${configVaultArchiveDeployment}/bin/nixome-config-vault-archive-deployment
  '';

  vmShellService = pkgs.writeShellScript "nixome-qubes-vmshell" ''
    export PATH=/run/current-system/sw/bin:/run/wrappers/bin:/bin:/usr/bin
    exec ${pkgs.runtimeShell}
  '';

  getAppmenusService = pkgs.writeShellScript "nixome-qubes-get-appmenus" ''
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
  options.nixome.qubeGuest = {
    enable = lib.mkEnableOption "a Nixome NixOS Qubes guest template";

    role = lib.mkOption {
      type = lib.types.enum [
        "base"
        "config-vault"
        "config-staging"
        "config-validator"
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
      description = "Editable config repository path in the config-staging role.";
    };

    configValidationPath = lib.mkOption {
      type = lib.types.str;
      default = "/home/${cfg.user}/nixome-config-validation";
      description = "Directory for config-staging and config-validator validation results.";
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
      unitConfig.OnFailure = [ "nixome-qubes-rw-init-diagnostics.service" ];
    };

    systemd.services.nixome-config-guest-init = lib.mkIf hasConfigRole {
      description = "Initialize Nixome config repositories in Qubes private storage";
      wantedBy = [ "multi-user.target" ];
      wants = [ "nixome-qubes-rw-init.service" ];
      requires = [ "nixome-qubes-rw-init.service" ];
      after = [
        "nixome-qubes-rw-init.service"
        "qubes-db.service"
      ];
      before = [ "qubes-qrexec-agent.service" ];
      path = [
        pkgs.coreutils
        pkgs.qubes-core-qubesdb
        pkgs.util-linux
      ];
      script = ''
        set -eu

        role=${lib.escapeShellArg cfg.role}
        vm_name="$(qubesdb-read /name 2>/dev/null || true)"
        qdb_role="$(qubesdb-read /features/nixome-role 2>/dev/null || true)"

        case "$qdb_role" in
          config-vault|config-staging|config-validator|config-system) role="$qdb_role" ;;
          "") ;;
          *) role=base ;;
        esac

        case "$vm_name" in
          config-vault) role=config-vault ;;
          config-staging) role=config-staging ;;
          config-validator) role=config-validator ;;
        esac

        run_as_user() {
          ${pkgs.util-linux}/bin/runuser -u ${lib.escapeShellArg cfg.user} -- \
            ${pkgs.coreutils}/bin/env PATH=${lib.escapeShellArg roleInitPath} "$@"
        }

        case "$role" in
          config-vault)
            run_as_user ${configVaultInit}/bin/nixome-config-vault-init
            ;;
          config-staging)
            run_as_user ${configStagingInit}/bin/nixome-config-staging-init
            ;;
          config-validator)
            run_as_user ${configValidatorInit}/bin/nixome-config-validator-init
            ;;
          config-system)
            run_as_user ${configVaultInit}/bin/nixome-config-vault-init
            run_as_user ${configStagingInit}/bin/nixome-config-staging-init
            run_as_user ${configValidatorInit}/bin/nixome-config-validator-init
            ;;
          *)
            echo "nixome config guest init: no config repo initialization for role=$role vm=$vm_name qdb_role=$qdb_role"
            ;;
        esac
      '';
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
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
      unitConfig.OnFailure = [ "nixome-qubes-db-diagnostics.service" ];
    };

    systemd.services.qubes-qrexec-agent = {
      enable = true;
      wantedBy = [ "multi-user.target" ];
      wants = [ "nixome-qubes-rw-init.service" ];
      requires = [
        "nixome-qubes-rw-init.service"
      ]
      ++ lib.optional hasConfigRole "nixome-config-guest-init.service";
      after = [
        "nixome-qubes-rw-init.service"
        "systemd-user-sessions.service"
      ]
      ++ lib.optional hasConfigRole "nixome-config-guest-init.service";
      aliases = [ "qubes-core-agent.service" ];
      overrideStrategy = "asDropin";
      unitConfig.OnFailure = [ "nixome-qubes-qrexec-diagnostics.service" ];
    };

    systemd.services.qubes-meminfo-writer = {
      enable = true;
      wantedBy = [ "multi-user.target" ];
      before = [ "systemd-user-sessions.service" ];
    };

    environment.etc."qubes-rpc/nixome.Config.CandidateExport" = lib.mkIf hasConfigStagingRole {
      source = configCandidateExportService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/nixome.Config.CandidateExport" = lib.mkIf hasConfigStagingRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/nixome.Config.ValidateCandidate" = lib.mkIf hasConfigValidatorRole {
      source = configCandidateValidateService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/nixome.Config.ValidateCandidate" =
      lib.mkIf hasConfigValidatorRole
        {
          text = "force-user='${cfg.user}'\n";
          mode = "0444";
        };

    environment.etc."qubes-rpc/nixome.Config.ArchiveCandidate" = lib.mkIf hasConfigVaultRole {
      source = configArchiveService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/nixome.Config.ArchiveCandidate" = lib.mkIf hasConfigVaultRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/nixome.Config.ArchiveDeployment" = lib.mkIf hasConfigVaultRole {
      source = configDeploymentArchiveService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/nixome.Config.ArchiveDeployment" = lib.mkIf hasConfigVaultRole {
      text = "force-user='${cfg.user}'\n";
      mode = "0444";
    };

    environment.etc."qubes-rpc/qubes.VMShell" = {
      source = vmShellService;
      mode = "0555";
    };

    environment.etc."qubes/rpc-config/qubes.VMShell" = {
      text = "force-user='${cfg.user}'\n";
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

    environment.systemPackages = [
      pkgs.qubes-core-qubesdb
      qubesAgentLinux
      pkgs.qubes-linux-utils
      pkgs.python3.pkgs.qubes-core-qrexec
      pkgs.python3.pkgs.qubes-core-qrexec.domU
    ]
    ++ (rolePackages.${cfg.role} or [ ]);

    nix.settings = lib.mkIf (hasConfigStagingRole || hasConfigValidatorRole) {
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
