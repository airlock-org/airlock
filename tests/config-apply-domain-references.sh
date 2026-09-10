#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
WORK_DIR="${NIXOME_CONFIG_APPLY_DOMAIN_REFERENCES_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-apply-domain-references}"
CANDIDATE_DIR="$WORK_DIR/candidate"
VALIDATED_TAR="$WORK_DIR/config-candidate-validated.tar"
CAPABILITIES="$WORK_DIR/xen-capabilities"

rm -rf "$WORK_DIR"
mkdir -p "$CANDIDATE_DIR"
printf 'control_d\n' > "$CAPABILITIES"

cat > "$CANDIDATE_DIR/flake.nix" <<'EOF'
{
  outputs = { self }: { };
}
EOF

(
  cd "$CANDIDATE_DIR"
  git init -b main >/dev/null
  git add flake.nix
  git \
    -c user.name='Nixome Test' \
    -c user.email='nixome-test@localhost' \
    commit -m 'fixture candidate' >/dev/null
)

CANDIDATE_COMMIT="$(git -C "$CANDIDATE_DIR" rev-parse HEAD)"
git -C "$CANDIDATE_DIR" bundle create "$WORK_DIR/config.bundle" HEAD refs/heads/main >/dev/null

cat > "$WORK_DIR/topology.json" <<'EOF'
{
  "schema": "nixome.qubes-topology.v1",
  "templates": {},
  "kernels": {},
  "domains": {
    "dev": {
      "class": "StandaloneVM",
      "template": null,
      "label": "green",
      "netvm": "sys-firewall",
      "prefs": {},
      "features": {
        "nixome-role": "dev"
      },
      "services": {},
      "backend": {
        "type": "qubes-standalone",
        "template": null
      }
    }
  },
  "prefs": {},
  "qrexec_policy": {
    "rules": []
  }
}
EOF

cat > "$WORK_DIR/system.json" <<'EOF'
{
  "schema": "nixome.system.v1",
  "dom0": {
    "profile": {
      "source": null,
      "source_ref": null,
      "activation_command": "bin/nixome-dom0-profile-activate",
      "activation_action": "none",
      "message": ""
    },
    "packages": [],
    "systemd": {
      "services": {}
    },
    "nixos": {
      "system": {
        "enable": false,
        "reason": "",
        "source": null,
        "source_ref": null,
        "switch_action": "switch"
      },
      "kernel": {
        "enable": false,
        "reason": "",
        "source": null,
        "source_ref": null
      },
      "xen": {
        "enable": false,
        "reason": "",
        "source": null,
        "source_ref": null
      },
      "bootloader": {
        "enable": false,
        "reason": "",
        "source": null,
        "source_ref": null
      },
      "qubes": {
        "enable": false,
        "reason": "",
        "source": null,
        "source_ref": null
      }
    }
  },
  "plan": {
    "live_actions": [],
    "reboot_actions": [],
    "reboot_required": false,
    "reboot_reasons": []
  }
}
EOF

jq -c -n \
  --arg schema "nixome.config-validation.v1" \
  --arg status "passed" \
  --arg commit "$CANDIDATE_COMMIT" \
  '{schema: $schema, status: $status, commit: $commit, checks: []}' \
  > "$WORK_DIR/validation.json"
jq -c -n \
  --arg schema "nixome.config-candidate.v1" \
  --arg commit "$CANDIDATE_COMMIT" \
  --arg branch "main" \
  '{schema: $schema, commit: $commit, branch: $branch, source_role: "config-staging"}' \
  > "$WORK_DIR/manifest.json"
tar -C "$WORK_DIR" -cf "$VALIDATED_TAR" config.bundle validation.json manifest.json topology.json system.json

run_case() {
  case_name="$1"
  sys_firewall_present="$2"
  expect_status="$3"
  case_dir="$WORK_DIR/$case_name"
  bin_dir="$case_dir/bin"
  log_dir="$case_dir/logs"
  state_dir="$case_dir/state"
  etc_dir="$case_dir/etc"
  setup_capture="$case_dir/setup-topology.json"

  mkdir -p "$bin_dir" "$log_dir" "$state_dir" "$etc_dir"

  cat > "$bin_dir/nixome-config-archive-candidate" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cp "$VALIDATED_TAR_FIXTURE" "$NIXOME_LOG_DIR/config-candidate-validated.tar"
jq -c -n \
  --arg schema "nixome.config-archive.v1" \
  --arg status "passed" \
  --arg candidate_commit "$CANDIDATE_COMMIT_FIXTURE" \
  '{
    schema: $schema,
    status: $status,
    candidate_commit: $candidate_commit,
    validation_commit: $candidate_commit,
    validation: {
      schema: "nixome.config-validation.v1",
      status: "passed",
      commit: $candidate_commit
    },
    vault_archive: {
      schema: "nixome.config-vault-archive.v1",
      status: "passed",
      candidate_commit: $candidate_commit
    }
  }' > "$NIXOME_LOG_DIR/config-archive.json"
printf 'archive fixture ok\n'
EOF

  cat > "$bin_dir/nixome-setup-config" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
topology=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --topology)
      shift
      topology="$1"
      ;;
  esac
  shift
done
[ -n "$topology" ] || exit 2
cp "$topology" "$SETUP_CAPTURE_FIXTURE"
jq -e '.domains.dev.netvm == "sys-firewall"' "$topology" >/dev/null
jq -c -n \
  --arg schema "nixome.setup-config.v1" \
  '{schema: $schema, status: "passed", domains_changed: 0, templates_changed: 0}' \
  > "$NIXOME_LOG_DIR/setup-config.json"
printf 'setup fixture ok\n'
EOF

  cat > "$bin_dir/qrexec-client" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cat >/dev/null
jq -c -n \
  --arg schema "nixome.config-vault-deployment.v1" \
  --arg status "passed" \
  --arg candidate_commit "$CANDIDATE_COMMIT_FIXTURE" \
  '{
    schema: $schema,
    status: $status,
    deployment_status: "passed",
    candidate_commit: $candidate_commit,
    vault_commit: "fixture-vault"
  }'
EOF

  cat > "$bin_dir/qvm-ls" <<EOF
#!/usr/bin/env bash
if [ "\$*" = "--raw-data --fields NAME --no-spinner" ]; then
  printf 'dom0\n'
  printf 'dev\n'
  if [ "$sys_firewall_present" = "1" ]; then
    printf 'sys-firewall\n'
  fi
  exit 0
fi
printf 'unexpected qvm-ls args: %s\n' "\$*" >&2
exit 1
EOF

  cat > "$bin_dir/systemctl" <<'EOF'
#!/usr/bin/env bash
if [ "$1" = "is-active" ]; then
  exit 0
fi
exit 1
EOF

  chmod 0755 \
    "$bin_dir/nixome-config-archive-candidate" \
    "$bin_dir/nixome-setup-config" \
    "$bin_dir/qrexec-client" \
    "$bin_dir/qvm-ls" \
    "$bin_dir/systemctl"

  set +e
  PATH="$bin_dir:$PATH" \
    CANDIDATE_COMMIT_FIXTURE="$CANDIDATE_COMMIT" \
    SETUP_CAPTURE_FIXTURE="$setup_capture" \
    VALIDATED_TAR_FIXTURE="$VALIDATED_TAR" \
    NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
    NIXOME_LOG_DIR="$log_dir" \
    NIXOME_USE_HOST_NIX_STORE=1 \
    NIXOME_SYSTEM_STATE_DIR="$state_dir" \
    NIXOME_SYSTEM_ETC_DIR="$etc_dir" \
    bash "$ROOT/scripts/apply-config-candidate.sh" --apply \
    > "$case_dir/stdout.log" 2> "$case_dir/stderr.log"
  rc="$?"
  set -e

  if [ "$expect_status" = "passed" ]; then
    [ "$rc" -eq 0 ]
    [ -r "$setup_capture" ]
    jq -e '
      .schema == "nixome.config-apply.v1" and
      .status == "passed" and
      .domain_references_result.status == "passed" and
      any(.domain_references_result.references[]; .kind == "netvm" and .domain == "dev" and .target == "sys-firewall")
    ' "$log_dir/config-apply.json" >/dev/null
  else
    [ "$rc" -ne 0 ]
    [ ! -e "$setup_capture" ]
    jq -e '
      .schema == "nixome.config-apply.v1" and
      .status == "failed" and
      .reason == "topology references missing domains" and
      .domain_references_result.status == "failed" and
      .domain_references_result.missing == [
        {
          "kind": "netvm",
          "domain": "dev",
          "field": "netvm",
          "target": "sys-firewall"
        }
      ]
    ' "$log_dir/config-apply.json" >/dev/null
  fi
}

run_case missing 0 failed
run_case present 1 passed

printf 'config apply domain references test ok\n'
