#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${NIXOME_CONFIG_SYSTEM_APPLY_BOOT_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-system-apply-boot}"
BIN_DIR="$WORK_DIR/bin"
CANDIDATE_DIR="$WORK_DIR/candidate"
LOG_DIR="$WORK_DIR/logs"
CAPABILITIES="$WORK_DIR/xen-capabilities"
HOST_SYSTEM_SOURCE="$CANDIDATE_DIR/dom0-system"
SYSTEM_STATE_DIR="$WORK_DIR/state"
SYSTEM_ETC_DIR="$WORK_DIR/etc"
SYSTEM_PROFILE="$WORK_DIR/nixome-system-profile"
HOST_SYSTEM_PROFILE="$WORK_DIR/host-system-profile"
HOST_CURRENT_SYSTEM_LINK="$WORK_DIR/current-system"
VALIDATED_TAR="$WORK_DIR/config-candidate-validated.tar"
DEPLOYMENT_TAR_CAPTURE="$WORK_DIR/deployment.tar"

rm -rf "$WORK_DIR"
mkdir -p \
  "$BIN_DIR" \
  "$CANDIDATE_DIR/nixome" \
  "$LOG_DIR" \
  "$HOST_SYSTEM_SOURCE/bin" \
  "$SYSTEM_ETC_DIR"
printf 'control_d\n' > "$CAPABILITIES"
cp "$ROOT/lib/nixome/system.nix" "$CANDIDATE_DIR/nixome/system-schema.nix"

cat > "$HOST_SYSTEM_SOURCE/bin/switch-to-configuration" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

printf '%s\n' "$1" > "$NIXOME_SYSTEM_STATE_DIR/host-switch-action"
printf '%s\n' "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" > "$NIXOME_SYSTEM_STATE_DIR/host-switch-source"
if [ "$1" != "boot" ]; then
  ln -sfn "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" "$NIXOME_HOST_CURRENT_SYSTEM_LINK"
fi
EOF
chmod 0755 "$HOST_SYSTEM_SOURCE/bin/switch-to-configuration"

cat > "$CANDIDATE_DIR/flake.nix" <<'EOF'
{
  outputs =
    { self }:
    let
      systemSchema = import ./nixome/system-schema.nix;
    in
    {
      nixome.topology = {
        schema = "nixome.qubes-topology.v1";
        templates = { };
        kernels = { };
        domains = { };
        prefs = { };
        qrexec_policy.rules = [ ];
      };

      nixome.system = systemSchema.mkSystem {
        dom0.nixos.system = {
          source = "__HOST_SYSTEM_SOURCE__";
          switchAction = "boot";
          reason = "install next dom0 boot generation";
        };
      };
    };
}
EOF
sed -i "s|__HOST_SYSTEM_SOURCE__|dom0-system|g" "$CANDIDATE_DIR/flake.nix"

(
  cd "$CANDIDATE_DIR"
  git init -b main >/dev/null
  git add dom0-system/bin/switch-to-configuration flake.nix nixome/system-schema.nix
  git \
    -c user.name='Nixome Test' \
    -c user.email='nixome-test@localhost' \
    commit -m 'fixture boot candidate' >/dev/null
)

CANDIDATE_COMMIT="$(git -C "$CANDIDATE_DIR" rev-parse HEAD)"
EXPECTED_HOST_SYSTEM_SOURCE="$SYSTEM_STATE_DIR/sources/$CANDIDATE_COMMIT/dom0.nixos.system"
git -C "$CANDIDATE_DIR" bundle create "$WORK_DIR/config.bundle" HEAD refs/heads/main >/dev/null
nix eval --json "$CANDIDATE_DIR#nixome.topology" > "$WORK_DIR/topology.json"
nix eval --json "$CANDIDATE_DIR#nixome.system" > "$WORK_DIR/system.json"

jq -c -n \
  --arg schema "nixome.config-validation.v1" \
  --arg status "passed" \
  --arg commit "$CANDIDATE_COMMIT" \
  '{schema: $schema, status: $status, commit: $commit, checks: []}' \
  > "$WORK_DIR/validation.json"
jq -c -n \
  --arg schema "nixome.config-candidate.v1" \
  --arg commit "$CANDIDATE_COMMIT" \
  '{schema: $schema, commit: $commit, branch: "main", source_role: "config-staging"}' \
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
EOF

cat > "$BIN_DIR/nixome-setup-config" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
jq -c -n \
  --arg schema "nixome.setup-config.v1" \
  --arg status "passed" \
  '{schema: $schema, status: $status, domains_changed: 0, templates_changed: 0}' \
  > "$NIXOME_LOG_DIR/setup-config.json"
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
set -euo pipefail

case "$1" in
is-active)
  exit 0
  ;;
esac

printf 'unexpected systemctl args:' >&2
printf ' [%s]' "$@" >&2
printf '\n' >&2
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
  VALIDATED_TAR_FIXTURE="$VALIDATED_TAR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  NIXOME_USE_HOST_NIX_STORE=1 \
  NIXOME_SYSTEM_STATE_DIR="$SYSTEM_STATE_DIR" \
  NIXOME_SYSTEM_ETC_DIR="$SYSTEM_ETC_DIR" \
  NIXOME_SYSTEM_PROFILE="$SYSTEM_PROFILE" \
  NIXOME_HOST_SYSTEM_PROFILE="$HOST_SYSTEM_PROFILE" \
  NIXOME_HOST_CURRENT_SYSTEM_LINK="$HOST_CURRENT_SYSTEM_LINK" \
  bash "$ROOT/scripts/apply-config-candidate.sh" --apply \
  > "$WORK_DIR/stdout.log" 2> "$WORK_DIR/stderr.log"

jq -e \
  --arg commit "$CANDIDATE_COMMIT" \
  --arg expected_host_system_source "$EXPECTED_HOST_SYSTEM_SOURCE" \
  '
    .schema == "nixome.config-apply.v1" and
    .status == "passed" and
    .candidate_commit == $commit and
    .system_apply_result.status == "passed" and
    .system_apply_result.profile_status == "skipped" and
    .system_apply_result.host_system_status == "passed" and
    .system_apply_result.host_system_source == $expected_host_system_source and
    .system_apply_result.host_system_switch_action == "boot" and
    .system_apply_result.host_current_system == "" and
    .system_apply_result.reboot_required == true and
    .system_apply_result.reboot_reasons == ["install next dom0 boot generation"] and
    .deployment.status == "passed" and
    .deployment.system_apply.status == "passed" and
    .deployment.system_apply.reboot_required == true
  ' "$LOG_DIR/config-apply.json" >/dev/null

jq -e '
  .schema == "nixome.system.v1" and
  .dom0.nixos.system.switch_action == "boot" and
  .plan.reboot_required == true and
  (.plan.reboot_actions | length) == 1 and
  .plan.reboot_actions[0].type == "dom0-nixos-system"
' "$SYSTEM_STATE_DIR/last-system.json" >/dev/null
jq -e '
  .schema == "nixome.config-deployment.v1" and
  .status == "passed" and
  .system_apply.reboot_required == true
' "$SYSTEM_STATE_DIR/current-deployment.json" >/dev/null
grep -qx boot "$SYSTEM_STATE_DIR/current-host-system-switch-action"
grep -qx boot "$SYSTEM_STATE_DIR/host-switch-action"
grep -qx "$EXPECTED_HOST_SYSTEM_SOURCE" "$SYSTEM_STATE_DIR/current-host-system"
grep -qx "$EXPECTED_HOST_SYSTEM_SOURCE" "$SYSTEM_STATE_DIR/host-switch-source"
[ "$(readlink "$HOST_SYSTEM_PROFILE")" = "$EXPECTED_HOST_SYSTEM_SOURCE" ]
[ ! -e "$HOST_CURRENT_SYSTEM_LINK" ]

printf 'config system apply boot test ok\n'
