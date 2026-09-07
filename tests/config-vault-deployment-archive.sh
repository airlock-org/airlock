#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"

WORK_DIR="${NIXOME_CONFIG_VAULT_DEPLOYMENT_ARCHIVE_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-vault-deployment-archive}"
VAULT_REPO="$WORK_DIR/vault"
DEPLOYMENT_DIR="$WORK_DIR/deployment"
DEPLOYMENT_TAR="$WORK_DIR/deployment.tar"
RESULT_JSON="$WORK_DIR/result.json"
RESULT_ERR="$WORK_DIR/result.stderr.log"
CANDIDATE_COMMIT="0123456789abcdef0123456789abcdef01234567"
DEPLOYMENT_ID="20260907T000000Z-0123456789ab"

rm -rf "$WORK_DIR"
mkdir -p "$VAULT_REPO/candidates/$CANDIDATE_COMMIT" "$DEPLOYMENT_DIR/logs"

git -C "$VAULT_REPO" init -b main >/dev/null
printf 'candidate placeholder\n' > "$VAULT_REPO/candidates/$CANDIDATE_COMMIT/README.md"
git -C "$VAULT_REPO" add "candidates/$CANDIDATE_COMMIT/README.md"
git -C "$VAULT_REPO" \
  -c user.name='Nixome Vault Deployment Test' \
  -c user.email='nixome-vault-deployment-test@localhost' \
  commit -m 'archive candidate fixture' >/dev/null

jq -c -n \
  --arg schema "nixome.config-deployment.v1" \
  --arg status "passed" \
  --arg reason "test deployment" \
  --arg deployment_id "$DEPLOYMENT_ID" \
  --arg deployed_at "20260907T000000Z" \
  --arg candidate_commit "$CANDIDATE_COMMIT" \
  '{
    schema: $schema,
    status: $status,
    reason: $reason,
    deployment_id: $deployment_id,
    deployed_at: $deployed_at,
    deployed_by: "dom0",
    staging: "config-staging",
    validator: "config-validator",
    vault: "config-vault",
    candidate_commit: $candidate_commit,
    artifacts: {
      topology: "topology.json",
      system: "system.json",
      resolved_system: "resolved-system.json",
      system_plan: "system-plan.json",
      system_apply: "system-apply.json",
      system_builds: "system-builds.json",
      systemd_actions: "systemd-actions.json",
      apply: "apply.json",
      archive: "archive.json",
      setup: "setup.json",
      template_builds: "template-builds.json",
      nix_domain_builds: "nix-domain-builds.json",
      nix_domain_materializations: "nix-domain-materializations.json",
      unsupported_domain_backends: "unsupported-domain-backends.json",
      validated_topology: "validated-topology.json",
      validation: "validation.json",
      manifest: "manifest.json"
    }
  }' > "$DEPLOYMENT_DIR/deployment.json"

jq -c -n \
  --arg schema "nixome.config-apply.v1" \
  --arg status "passed" \
  --arg candidate_commit "$CANDIDATE_COMMIT" \
  '{schema: $schema, status: $status, reason: "test apply", candidate_commit: $candidate_commit}' \
  > "$DEPLOYMENT_DIR/apply.json"

jq -c -n \
  --arg schema "nixome.config-archive.v1" \
  --arg candidate_commit "$CANDIDATE_COMMIT" \
  '{schema: $schema, status: "passed", reason: "test archive", candidate_commit: $candidate_commit}' \
  > "$DEPLOYMENT_DIR/archive.json"

jq -c -n \
  --arg schema "nixome.setup-config.v1" \
  '{schema: $schema, status: "passed", mode: "apply", reason: "test setup"}' \
  > "$DEPLOYMENT_DIR/setup.json"

jq -c -n \
  --arg schema "nixome.qubes-topology.v1" \
  '{schema: $schema, templates: {}, kernels: {}, domains: {}, prefs: {}, qrexec_policy: {rules: []}}' \
  > "$DEPLOYMENT_DIR/topology.json"
cp "$DEPLOYMENT_DIR/topology.json" "$DEPLOYMENT_DIR/validated-topology.json"

jq -c -n \
  --arg schema "nixome.template-builds.v1" \
  '{schema: $schema, builds: []}' \
  > "$DEPLOYMENT_DIR/template-builds.json"
jq -c -n \
  --arg schema "nixome.nix-domain-builds.v1" \
  '{schema: $schema, builds: []}' \
  > "$DEPLOYMENT_DIR/nix-domain-builds.json"
jq -c -n \
  --arg schema "nixome.nix-domain-materializations.v1" \
  '{schema: $schema, domains: []}' \
  > "$DEPLOYMENT_DIR/nix-domain-materializations.json"
jq -c -n \
  --arg schema "nixome.unsupported-domain-backends.v1" \
  '{schema: $schema, domains: []}' \
  > "$DEPLOYMENT_DIR/unsupported-domain-backends.json"

jq -c -n \
  --arg schema "nixome.system.v1" \
  '{
    schema: $schema,
    dom0: {
      profile: {source: null, source_ref: null, activation_command: "bin/nixome-dom0-profile-activate", activation_action: "none", message: ""},
      packages: [],
      systemd: {services: {}},
      nixos: {
        system: {enable: false, reason: "", source: null, source_ref: null, switch_action: "switch"},
        kernel: {enable: false, reason: "", source: null, source_ref: null},
        xen: {enable: false, reason: "", source: null, source_ref: null},
        bootloader: {enable: false, reason: "", source: null, source_ref: null},
        qubes: {enable: false, reason: "", source: null, source_ref: null}
      }
    },
    plan: {live_actions: [], reboot_actions: [], reboot_required: false, reboot_reasons: []}
  }' > "$DEPLOYMENT_DIR/system.json"
cp "$DEPLOYMENT_DIR/system.json" "$DEPLOYMENT_DIR/resolved-system.json"
jq -c '.plan' "$DEPLOYMENT_DIR/system.json" > "$DEPLOYMENT_DIR/system-plan.json"

jq -c -n \
  --arg schema "nixome.system-apply.v1" \
  '{schema: $schema, status: "passed", reason: "test system apply"}' \
  > "$DEPLOYMENT_DIR/system-apply.json"
jq -c -n \
  --arg schema "nixome.system-builds.v1" \
  '{schema: $schema, builds: []}' \
  > "$DEPLOYMENT_DIR/system-builds.json"
jq -c -n \
  --arg schema "nixome.systemd-actions.v1" \
  '{schema: $schema, status: "passed", reason: "test systemd actions", actions: []}' \
  > "$DEPLOYMENT_DIR/systemd-actions.json"
jq -c -n \
  --arg schema "nixome.config-validation.v1" \
  --arg commit "$CANDIDATE_COMMIT" \
  '{schema: $schema, status: "passed", reason: "test validation", commit: $commit, checks: []}' \
  > "$DEPLOYMENT_DIR/validation.json"
jq -c -n \
  --arg schema "nixome.config-candidate.v1" \
  --arg commit "$CANDIDATE_COMMIT" \
  '{schema: $schema, commit: $commit, branch: "main"}' \
  > "$DEPLOYMENT_DIR/manifest.json"

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
  printf '%s\n' "$log" > "$DEPLOYMENT_DIR/logs/$log"
done

tar -C "$DEPLOYMENT_DIR" -cf "$DEPLOYMENT_TAR" \
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
  system.json \
  resolved-system.json \
  system-plan.json \
  system-apply.json \
  system-builds.json \
  systemd-actions.json \
  validation.json \
  manifest.json \
  logs/apply.commands.log \
  logs/archive.stdout.log \
  logs/archive.stderr.log \
  logs/setup.stdout.log \
  logs/setup.stderr.log \
  logs/template-builds.log \
  logs/nix-domain-builds.log \
  logs/system-builds.log \
  logs/system-apply.log \
  logs/profile-activation.log \
  logs/host-system-activation.log \
  logs/systemd-actions.log \
  logs/clone.log \
  logs/checkout.log

archive_cmd="$(
  nix build \
    --no-link \
    --print-out-paths \
    "$ROOT#nixosConfigurations.qube-template-config-system.config.environment.etc.\"qubes-rpc/nixome.Config.ArchiveDeployment\".source"
)"

NIXOME_CONFIG_ARCHIVE_PATH="$VAULT_REPO" \
  nixome_run_store_path "$archive_cmd" \
  < "$DEPLOYMENT_TAR" \
  > "$RESULT_JSON" \
  2> "$RESULT_ERR"

jq -e \
  --arg deployment_id "$DEPLOYMENT_ID" \
  --arg candidate_commit "$CANDIDATE_COMMIT" \
  '
    .schema == "nixome.config-vault-deployment.v1" and
    .status == "passed" and
    .deployment_status == "passed" and
    .deployment_id == $deployment_id and
    .candidate_commit == $candidate_commit
  ' "$RESULT_JSON" >/dev/null

[ -f "$VAULT_REPO/deployments/$DEPLOYMENT_ID/systemd-actions.json" ]
[ -f "$VAULT_REPO/deployments/$DEPLOYMENT_ID/nix-domain-builds.json" ]
[ -f "$VAULT_REPO/deployments/$DEPLOYMENT_ID/nix-domain-materializations.json" ]
[ -f "$VAULT_REPO/deployments/$DEPLOYMENT_ID/unsupported-domain-backends.json" ]
[ -f "$VAULT_REPO/deployments/$DEPLOYMENT_ID/logs/host-system-activation.log" ]
[ -f "$VAULT_REPO/deployments/$DEPLOYMENT_ID/logs/systemd-actions.log" ]
grep -qx "$DEPLOYMENT_ID" "$VAULT_REPO/latest-deployment"
git -C "$VAULT_REPO" status --short --branch | grep -qx '## main'

printf 'config vault deployment archive ok\n'
