#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
WORK_DIR="${NIXOME_CONFIG_APPLY_NIX_DOMAIN_MATERIALIZATION_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-apply-nix-domain-materialization}"
BIN_DIR="$WORK_DIR/bin"
CANDIDATE_DIR="$WORK_DIR/candidate"
LOG_DIR="$WORK_DIR/logs"
CAPABILITIES="$WORK_DIR/xen-capabilities"
SYSTEM_STATE_DIR="$WORK_DIR/state"
SYSTEM_ETC_DIR="$WORK_DIR/etc"
VALIDATED_TAR="$WORK_DIR/config-candidate-validated.tar"
DEPLOYMENT_TAR_CAPTURE="$WORK_DIR/deployment.tar"

rm -rf "$WORK_DIR"
mkdir -p \
  "$BIN_DIR" \
  "$CANDIDATE_DIR/system-source" \
  "$CANDIDATE_DIR/root-source" \
  "$CANDIDATE_DIR/store-source" \
  "$LOG_DIR" \
  "$SYSTEM_STATE_DIR" \
  "$SYSTEM_ETC_DIR"
printf 'control_d\n' > "$CAPABILITIES"

cat > "$CANDIDATE_DIR/flake.nix" <<'EOF'
{
  description = "Nixome Nix-native domain materialization fixture";
  outputs = { self }: { };
}
EOF
printf '#!/bin/sh\nexec /init\n' > "$CANDIDATE_DIR/system-source/init"
chmod 0555 "$CANDIDATE_DIR/system-source/init"
: > "$CANDIDATE_DIR/root-source/root.img"
: > "$CANDIDATE_DIR/store-source/store.img"

(
  cd "$CANDIDATE_DIR"
  git init -b main >/dev/null
  git add flake.nix system-source root-source store-source
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
      "prefs": {
        "autostart": false,
        "include_in_backups": true,
        "kernelopts": "console=hvc0 init=/sbin/init",
        "memory": 1536,
        "virt_mode": "pvh"
      },
      "features": {
        "nixome-managed": "1",
        "nixome-role": "nix-native-work",
        "qrexec": "1"
      },
      "services": {},
      "backend": {
        "type": "nix-closure",
        "template": null,
        "system": {
          "source": "system-source",
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
        "root": {
          "mode": "qubes-root-image",
          "source": "root-source",
          "source_ref": null,
          "read_only": false
        },
        "store": {
          "mode": "closure-image",
          "source": "store-source",
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
[ -n "$topology" ]

jq -e \
  --arg state "$SYSTEM_STATE_DIR_FIXTURE" \
  --arg commit "$CANDIDATE_COMMIT_FIXTURE" \
  '
    .domains."nix-native-work".backend.type == "nix-closure" and
    .domains."nix-native-work".backend.system.source_ref == null and
    .domains."nix-native-work".backend.root.source_ref == null and
    .domains."nix-native-work".backend.store.source_ref == null and
    (.domains."nix-native-work".backend.system.source | startswith($state + "/sources/" + $commit + "/nix-domain-nix-native-work-system")) and
    (.domains."nix-native-work".backend.root.source | startswith($state + "/sources/" + $commit + "/nix-domain-nix-native-work-root")) and
    (.domains."nix-native-work".backend.store.source | startswith($state + "/sources/" + $commit + "/nix-domain-nix-native-work-store"))
  ' "$topology" >/dev/null

jq -c -n \
  --arg schema "nixome.setup-config.v1" \
  '{
    schema: $schema,
    status: "passed",
    mode: "apply",
    reason: "fixture setup passed",
    templates_changed: 0,
    kernels_changed: 0,
    domains_changed: 1,
    prefs_changed: 0,
    features_changed: 0,
    services_changed: 0,
    qrexec_policy_changed: 0,
    root_images_changed: 1,
    live_actions: [],
    restart_actions: [],
    unsupported_domains: [],
    unsupported_domains_count: 0,
    restart_required: false
  }' > "$NIXOME_LOG_DIR/setup-config.json"
cat "$NIXOME_LOG_DIR/setup-config.json"
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
    deployment_status: "passed",
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

PATH="$BIN_DIR:$PATH" \
  CANDIDATE_COMMIT_FIXTURE="$CANDIDATE_COMMIT" \
  DEPLOYMENT_TAR_CAPTURE_FIXTURE="$DEPLOYMENT_TAR_CAPTURE" \
  SYSTEM_STATE_DIR_FIXTURE="$SYSTEM_STATE_DIR" \
  VALIDATED_TAR_FIXTURE="$VALIDATED_TAR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  NIXOME_USE_HOST_NIX_STORE=1 \
  NIXOME_SYSTEM_STATE_DIR="$SYSTEM_STATE_DIR" \
  NIXOME_SYSTEM_ETC_DIR="$SYSTEM_ETC_DIR" \
  bash "$ROOT/scripts/apply-config-candidate.sh" --apply \
  > "$WORK_DIR/stdout.log" 2> "$WORK_DIR/stderr.log"

jq -e \
  --arg commit "$CANDIDATE_COMMIT" \
  --arg state "$SYSTEM_STATE_DIR" \
  '
    .schema == "nixome.config-apply.v1" and
    .status == "passed" and
    .candidate_commit == $commit and
    .reason == "validated config candidate applied" and
    .setup.status == "passed" and
    .system_apply_result.status == "passed" and
    .nix_domain_builds_result.schema == "nixome.nix-domain-builds.v1" and
    .nix_domain_materializations_result.schema == "nixome.nix-domain-materializations.v1" and
    .nix_domain_materializations_result.domains[0].domain == "nix-native-work" and
    (.nix_domain_materializations_result.domains[0].root.source | startswith($state + "/sources/" + $commit + "/nix-domain-nix-native-work-root")) and
    .unsupported_domain_backends_result.schema == "nixome.unsupported-domain-backends.v1" and
    .unsupported_domain_backends_result.domains == [] and
    .deployment.status == "passed" and
    .deployment.nix_domain_materializations.domains[0].domain == "nix-native-work" and
    .deployment_archive.status == "passed"
  ' "$LOG_DIR/config-apply.json" >/dev/null

jq -e \
  --arg commit "$CANDIDATE_COMMIT" \
  '
    .schema == "nixome.config-deployment.v1" and
    .status == "passed" and
    .candidate_commit == $commit and
    .nix_domain_builds.schema == "nixome.nix-domain-builds.v1" and
    .nix_domain_materializations.schema == "nixome.nix-domain-materializations.v1"
  ' "$SYSTEM_STATE_DIR/current-deployment.json" >/dev/null

tar -tf "$DEPLOYMENT_TAR_CAPTURE" | sort > "$WORK_DIR/deployment-members.txt"
grep -Fx nix-domain-builds.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx nix-domain-materializations.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx unsupported-domain-backends.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx logs/nix-domain-builds.log "$WORK_DIR/deployment-members.txt" >/dev/null

mkdir -p "$WORK_DIR/deployment-extract"
tar -C "$WORK_DIR/deployment-extract" -xf "$DEPLOYMENT_TAR_CAPTURE" \
  nix-domain-builds.json \
  nix-domain-materializations.json \
  unsupported-domain-backends.json \
  deployment.json

jq -e '.schema == "nixome.nix-domain-builds.v1"' \
  "$WORK_DIR/deployment-extract/nix-domain-builds.json" >/dev/null
jq -e '
  .schema == "nixome.nix-domain-materializations.v1" and
  .domains[0].domain == "nix-native-work" and
  .domains[0].root.mode == "qubes-root-image" and
  .domains[0].store.mode == "closure-image"
' "$WORK_DIR/deployment-extract/nix-domain-materializations.json" >/dev/null
jq -e '
  .schema == "nixome.unsupported-domain-backends.v1" and
  .domains == []
' "$WORK_DIR/deployment-extract/unsupported-domain-backends.json" >/dev/null
jq -e '
  .artifacts.nix_domain_builds == "nix-domain-builds.json" and
  .artifacts.nix_domain_materializations == "nix-domain-materializations.json" and
  .artifacts.unsupported_domain_backends == "unsupported-domain-backends.json"
' "$WORK_DIR/deployment-extract/deployment.json" >/dev/null

printf 'config apply Nix-native domain materialization test ok\n'
