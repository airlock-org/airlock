#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
WORK_DIR="${NIXOME_CONFIG_APPLY_UNSUPPORTED_DOMAIN_BACKEND_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-apply-unsupported-domain-backend}"
BIN_DIR="$WORK_DIR/bin"
CANDIDATE_DIR="$WORK_DIR/candidate"
LOG_DIR="$WORK_DIR/logs"
CAPABILITIES="$WORK_DIR/xen-capabilities"
SYSTEM_STATE_DIR="$WORK_DIR/state"
SYSTEM_ETC_DIR="$WORK_DIR/etc"
VALIDATED_TAR="$WORK_DIR/config-candidate-validated.tar"
DEPLOYMENT_TAR_CAPTURE="$WORK_DIR/deployment.tar"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$CANDIDATE_DIR" "$LOG_DIR" "$SYSTEM_STATE_DIR" "$SYSTEM_ETC_DIR"
printf 'control_d\n' > "$CAPABILITIES"

cat > "$CANDIDATE_DIR/flake.nix" <<'EOF'
{
  description = "Unsupported Nixome domain backend fixture";
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
    "nix-native-work": {
      "class": "StandaloneVM",
      "template": null,
      "label": "green",
      "netvm": null,
      "prefs": {},
      "features": {},
      "services": {},
      "backend": {
        "type": "nix-closure",
        "template": null,
        "system": {
          "source": "/nix/store/test-system",
          "source_ref": null
        },
        "kernel": {
          "name": "6.12.64-1.fc41",
          "source": null,
          "source_ref": null
        },
        "initrd": {
          "name": null,
          "source": null,
          "source_ref": null
        },
        "modules": {
          "name": null,
          "source": null,
          "source_ref": null
        },
        "store": {
          "mode": "closure-image",
          "source": "/nix/store/test-store.img",
          "source_ref": null,
          "mountpoint": "/nix/store",
          "read_only": true
        },
        "state": {
          "persistent": "nix-native-work-private",
          "volatile": null
        }
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

cat > "$BIN_DIR/nixome-config-archive-candidate" <<'EOF'
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

cat > "$BIN_DIR/nixome-setup-config" <<'EOF'
#!/usr/bin/env bash
printf 'setup-config should not run for unsupported domain backends\n' >&2
exit 97
EOF

cat > "$BIN_DIR/qrexec-client" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cat > "$DEPLOYMENT_TAR_CAPTURE_FIXTURE"
jq -c -n \
  --arg schema "nixome.config-vault-deployment.v1" \
  --arg status "passed" \
  --arg candidate_commit "$CANDIDATE_COMMIT_FIXTURE" \
  '{
    schema: $schema,
    status: $status,
    deployment_status: "failed",
    candidate_commit: $candidate_commit,
    vault_commit: "fixture-vault"
  }'
EOF

cat > "$BIN_DIR/systemctl" <<'EOF'
#!/usr/bin/env bash
if [ "$1" = "is-active" ]; then
  exit 0
fi
exit 1
EOF

chmod 0755 \
  "$BIN_DIR/nixome-config-archive-candidate" \
  "$BIN_DIR/nixome-setup-config" \
  "$BIN_DIR/qrexec-client" \
  "$BIN_DIR/systemctl"

if PATH="$BIN_DIR:$PATH" \
  CANDIDATE_COMMIT_FIXTURE="$CANDIDATE_COMMIT" \
  DEPLOYMENT_TAR_CAPTURE_FIXTURE="$DEPLOYMENT_TAR_CAPTURE" \
  VALIDATED_TAR_FIXTURE="$VALIDATED_TAR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  NIXOME_USE_HOST_NIX_STORE=1 \
  NIXOME_SYSTEM_STATE_DIR="$SYSTEM_STATE_DIR" \
  NIXOME_SYSTEM_ETC_DIR="$SYSTEM_ETC_DIR" \
  bash "$ROOT/scripts/apply-config-candidate.sh" --apply \
  > "$WORK_DIR/stdout.log" 2> "$WORK_DIR/stderr.log"; then
  printf 'config apply unexpectedly accepted unsupported domain backend\n' >&2
  exit 1
fi

jq -e \
  --arg commit "$CANDIDATE_COMMIT" \
  '
    .schema == "nixome.config-apply.v1" and
    .status == "failed" and
    .candidate_commit == $commit and
    .reason == "nix-closure domain materializer is not implemented yet: nix-native-work" and
    .setup == null and
    .system_apply_result == null and
    .unsupported_domain_backends_result.schema == "nixome.unsupported-domain-backends.v1" and
    .unsupported_domain_backends_result.domains[0].domain == "nix-native-work" and
    .deployment.status == "failed" and
    .deployment.unsupported_domain_backends.domains[0].backend == "nix-closure" and
    .deployment_archive.status == "passed"
  ' "$LOG_DIR/config-apply.json" >/dev/null

[ ! -e "$LOG_DIR/setup-config.json" ]

jq -e \
  --arg commit "$CANDIDATE_COMMIT" \
  '
    .schema == "nixome.config-deployment.v1" and
    .status == "failed" and
    .candidate_commit == $commit and
    .reason == "nix-closure domain materializer is not implemented yet: nix-native-work"
  ' "$SYSTEM_STATE_DIR/last-deployment.json" >/dev/null
[ ! -e "$SYSTEM_STATE_DIR/current-deployment.json" ]

tar -tf "$DEPLOYMENT_TAR_CAPTURE" | sort > "$WORK_DIR/deployment-members.txt"
grep -Fx unsupported-domain-backends.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx setup.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx template-builds.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx system-builds.json "$WORK_DIR/deployment-members.txt" >/dev/null

mkdir -p "$WORK_DIR/deployment-extract"
tar -C "$WORK_DIR/deployment-extract" -xf \
  "$DEPLOYMENT_TAR_CAPTURE" \
  deployment.json \
  unsupported-domain-backends.json
jq -e '
  .schema == "nixome.unsupported-domain-backends.v1" and
  .domains == [
    {
      "domain": "nix-native-work",
      "backend": "nix-closure",
      "reason": "nix-closure domain materializer is not implemented yet"
    }
  ]
' "$WORK_DIR/deployment-extract/unsupported-domain-backends.json" >/dev/null
jq -e '
  .artifacts.unsupported_domain_backends == "unsupported-domain-backends.json" and
  .unsupported_domain_backends.domains[0].domain == "nix-native-work"
' "$WORK_DIR/deployment-extract/deployment.json" >/dev/null

printf 'config apply unsupported domain backend test ok\n'
