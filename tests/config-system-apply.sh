#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
WORK_DIR="${NIXOME_CONFIG_SYSTEM_APPLY_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-system-apply}"
BIN_DIR="$WORK_DIR/bin"
CANDIDATE_DIR="$WORK_DIR/candidate"
LOG_DIR="$WORK_DIR/logs"
CAPABILITIES="$WORK_DIR/xen-capabilities"
PROFILE_SOURCE="$CANDIDATE_DIR/dom0-profile"
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
  "$CANDIDATE_DIR" \
  "$LOG_DIR" \
  "$PROFILE_SOURCE/bin" \
  "$PROFILE_SOURCE/share/nixome" \
  "$HOST_SYSTEM_SOURCE/bin" \
  "$HOST_SYSTEM_SOURCE/etc"
printf 'control_d\n' > "$CAPABILITIES"

cat > "$PROFILE_SOURCE/share/nixome/profile.json" <<'EOF'
{"schema":"nixome.dom0-profile.v1","message":"profile-from-test","built_by":"config-system-apply-test"}
EOF

cat > "$PROFILE_SOURCE/bin/nixome-dom0-profile-activate" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

install -d -m 0755 "$NIXOME_SYSTEM_STATE_DIR" "$NIXOME_SYSTEM_ETC_DIR"
cp "$(dirname "$0")/../share/nixome/profile.json" "$NIXOME_SYSTEM_STATE_DIR/current-profile.json"
cp "$NIXOME_SYSTEM_JSON" "$NIXOME_SYSTEM_STATE_DIR/current-system.json"
cp "$NIXOME_SYSTEM_PLAN_JSON" "$NIXOME_SYSTEM_STATE_DIR/current-plan.json"
cp "$(dirname "$0")/../share/nixome/profile.json" "$NIXOME_SYSTEM_ETC_DIR/system-profile.json"
printf 'activated\n' > "$NIXOME_SYSTEM_STATE_DIR/profile-activated"
EOF
chmod 0755 "$PROFILE_SOURCE/bin/nixome-dom0-profile-activate"

cat > "$HOST_SYSTEM_SOURCE/etc/inside-config-message" <<'EOF'
profile-from-test
EOF

cat > "$HOST_SYSTEM_SOURCE/bin/switch-to-configuration" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [ "${NIXOME_FAIL_HOST_SWITCH:-0}" = "1" ]; then
  printf 'fixture host switch failed\n' >&2
  exit 42
fi

printf '%s\n' "$1" > "$NIXOME_SYSTEM_STATE_DIR/host-switch-action"
printf '%s\n' "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" > "$NIXOME_SYSTEM_STATE_DIR/host-switch-source"
ln -sfn "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)" "$NIXOME_HOST_CURRENT_SYSTEM_LINK"
cp "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)/etc/inside-config-message" "$NIXOME_SYSTEM_ETC_DIR/inside-config-applied-message"
EOF
chmod 0755 "$HOST_SYSTEM_SOURCE/bin/switch-to-configuration"

cat > "$CANDIDATE_DIR/flake.nix" <<'EOF'
{
  outputs =
    { self }:
    {
      nixome.topology = {
        schema = "nixome.qubes-topology.v1";
        templates = { };
        kernels = { };
        domains = { };
        prefs = { };
        qrexec_policy.rules = [ ];
      };

      nixome.system = {
        schema = "nixome.system.v1";
        dom0 = {
          profile = {
            source = "__PROFILE_SOURCE__";
            source_ref = null;
            activation_command = "bin/nixome-dom0-profile-activate";
            activation_action = "run";
            message = "profile-from-test";
          };
          packages = [
            { name = "nixome-dom0-profile"; }
          ];
          systemd.services = {
            "nixome-test-restart.service" = {
              action = "restart";
              reason = "restart fixture service";
            };
            "nixome-test-reload.service" = {
              action = "reload";
              reason = "reload fixture service";
            };
          };
          nixos = {
            system = {
              enable = true;
              reason = "dom0 system change requires reboot";
              source = "__HOST_SYSTEM_SOURCE__";
              source_ref = null;
              switch_action = "switch";
            };
            kernel = {
              enable = false;
              reason = "dom0 kernel change requires reboot";
              source = null;
              source_ref = null;
            };
            xen = {
              enable = false;
              reason = "dom0 xen change requires reboot";
              source = null;
              source_ref = null;
            };
            bootloader = {
              enable = false;
              reason = "dom0 bootloader change requires reboot";
              source = null;
              source_ref = null;
            };
            qubes = {
              enable = false;
              reason = "dom0 qubes change requires reboot";
              source = null;
              source_ref = null;
            };
          };
        };
        plan = {
          live_actions = [
          {
            type = "dom0-profile";
              action = "run";
              source = "__PROFILE_SOURCE__";
              source_ref = null;
            activation_command = "bin/nixome-dom0-profile-activate";
            message = "profile-from-test";
          }
          {
            type = "dom0-nixos-system";
            action = "switch";
            source = "__HOST_SYSTEM_SOURCE__";
            source_ref = null;
            reason = "dom0 system change requires reboot";
          }
          {
            type = "systemd-service";
            name = "nixome-test-restart.service";
            action = "restart";
            reason = "restart fixture service";
          }
          {
            type = "systemd-service";
            name = "nixome-test-reload.service";
            action = "reload";
            reason = "reload fixture service";
          }
        ];
          reboot_required = false;
          reboot_reasons = [ ];
        };
      };
    };
}
EOF
sed -i "s|__PROFILE_SOURCE__|dom0-profile|g" "$CANDIDATE_DIR/flake.nix"
sed -i "s|__HOST_SYSTEM_SOURCE__|dom0-system|g" "$CANDIDATE_DIR/flake.nix"

(
  cd "$CANDIDATE_DIR"
  git init -b main >/dev/null
  git add \
    dom0-profile/bin/nixome-dom0-profile-activate \
    dom0-profile/share/nixome/profile.json \
    dom0-system/bin/switch-to-configuration \
    dom0-system/etc/inside-config-message \
    flake.nix
  git \
    -c user.name='Nixome Test' \
    -c user.email='nixome-test@localhost' \
    commit -m 'fixture candidate' >/dev/null
)

CANDIDATE_COMMIT="$(git -C "$CANDIDATE_DIR" rev-parse HEAD)"
EXPECTED_PROFILE_SOURCE="$SYSTEM_STATE_DIR/sources/$CANDIDATE_COMMIT/dom0.profile"
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
jq -c -n \
  --arg schema "nixome.setup-config.v1" \
  --arg status "passed" \
  '{schema: $schema, status: $status, domains_changed: 0, templates_changed: 0}' \
  > "$NIXOME_LOG_DIR/setup-config.json"
printf 'setup fixture ok\n'
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

mkdir -p "$NIXOME_SYSTEM_STATE_DIR"

case "$1" in
is-active)
  exit 0
  ;;
restart|reload)
  printf '%s %s\n' "$1" "$2" >> "$NIXOME_SYSTEM_STATE_DIR/systemctl-calls"
  printf '%s %s ok\n' "$1" "$2"
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
    .system_apply_result.profile_status == "passed" and
    .system_apply_result.host_system_status == "passed" and
    .system_apply_result.systemd_status == "passed" and
    .system_apply_result.host_system_source == $expected_host_system_source and
    .system_apply_result.host_system_switch_action == "switch" and
    .system_apply_result.reboot_required == false and
    .systemd_actions_result.status == "passed" and
    .deployment.status == "passed" and
    .deployment.system_apply.status == "passed" and
    .deployment.systemd_actions.status == "passed"
  ' "$LOG_DIR/config-apply.json" >/dev/null

jq -e '
  .schema == "nixome.dom0-profile.v1" and
  .message == "profile-from-test"
' "$SYSTEM_STATE_DIR/current-profile.json" >/dev/null

jq -e \
  --arg expected_host_system_source "$EXPECTED_HOST_SYSTEM_SOURCE" \
  '
  .schema == "nixome.system.v1" and
  .dom0.profile.message == "profile-from-test" and
  .dom0.nixos.system.enable == true and
  .dom0.nixos.system.source == $expected_host_system_source and
  .dom0.nixos.system.switch_action == "switch" and
  .dom0.systemd.services."nixome-test-restart.service".action == "restart" and
  .dom0.systemd.services."nixome-test-reload.service".action == "reload" and
  .plan.reboot_required == false
' "$SYSTEM_STATE_DIR/current-system.json" >/dev/null

grep -qx activated "$SYSTEM_STATE_DIR/profile-activated"
grep -Fx "restart nixome-test-restart.service" "$SYSTEM_STATE_DIR/systemctl-calls" >/dev/null
grep -Fx "reload nixome-test-reload.service" "$SYSTEM_STATE_DIR/systemctl-calls" >/dev/null
[ "$(readlink "$SYSTEM_PROFILE")" = "$EXPECTED_PROFILE_SOURCE" ]
[ -x "$EXPECTED_PROFILE_SOURCE/bin/nixome-dom0-profile-activate" ]
[ "$(readlink "$HOST_SYSTEM_PROFILE")" = "$EXPECTED_HOST_SYSTEM_SOURCE" ]
[ "$(readlink "$HOST_CURRENT_SYSTEM_LINK")" = "$EXPECTED_HOST_SYSTEM_SOURCE" ]
[ -x "$EXPECTED_HOST_SYSTEM_SOURCE/bin/switch-to-configuration" ]
grep -qx switch "$SYSTEM_STATE_DIR/host-switch-action"
grep -qx "$EXPECTED_HOST_SYSTEM_SOURCE" "$SYSTEM_STATE_DIR/host-switch-source"
grep -qx profile-from-test "$SYSTEM_ETC_DIR/inside-config-applied-message"

tar -tf "$DEPLOYMENT_TAR_CAPTURE" | sort > "$WORK_DIR/deployment-members.txt"
grep -Fx system.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx resolved-system.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx system-plan.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx system-apply.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx system-builds.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx systemd-actions.json "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx logs/profile-activation.log "$WORK_DIR/deployment-members.txt" >/dev/null
grep -Fx logs/systemd-actions.log "$WORK_DIR/deployment-members.txt" >/dev/null

mkdir -p "$WORK_DIR/deployment-extract"
tar -C "$WORK_DIR/deployment-extract" -xf "$DEPLOYMENT_TAR_CAPTURE" deployment.json systemd-actions.json
jq -e '
  .schema == "nixome.systemd-actions.v1" and
  .status == "passed" and
  any(.actions[]; .name == "nixome-test-restart.service" and .action == "restart" and .status == "passed") and
  any(.actions[]; .name == "nixome-test-reload.service" and .action == "reload" and .status == "passed")
' "$WORK_DIR/deployment-extract/systemd-actions.json" >/dev/null
jq -e '
  .artifacts.systemd_actions == "systemd-actions.json" and
  .systemd_actions.status == "passed"
' "$WORK_DIR/deployment-extract/deployment.json" >/dev/null

FAIL_LOG_DIR="$WORK_DIR/fail-logs"
FAIL_SYSTEM_STATE_DIR="$WORK_DIR/fail-state"
FAIL_SYSTEM_ETC_DIR="$WORK_DIR/fail-etc"
FAIL_SYSTEM_PROFILE="$WORK_DIR/fail-nixome-system-profile"
FAIL_HOST_SYSTEM_PROFILE="$WORK_DIR/fail-host-system-profile"
FAIL_HOST_CURRENT_SYSTEM_LINK="$WORK_DIR/fail-current-system"
FAIL_DEPLOYMENT_TAR_CAPTURE="$WORK_DIR/deployment-failed.tar"
mkdir -p "$FAIL_LOG_DIR"

if PATH="$BIN_DIR:$PATH" \
  CANDIDATE_COMMIT_FIXTURE="$CANDIDATE_COMMIT" \
  DEPLOYMENT_TAR_CAPTURE_FIXTURE="$FAIL_DEPLOYMENT_TAR_CAPTURE" \
  VALIDATED_TAR_FIXTURE="$VALIDATED_TAR" \
  NIXOME_FAIL_HOST_SWITCH=1 \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$FAIL_LOG_DIR" \
  NIXOME_USE_HOST_NIX_STORE=1 \
  NIXOME_SYSTEM_STATE_DIR="$FAIL_SYSTEM_STATE_DIR" \
  NIXOME_SYSTEM_ETC_DIR="$FAIL_SYSTEM_ETC_DIR" \
  NIXOME_SYSTEM_PROFILE="$FAIL_SYSTEM_PROFILE" \
  NIXOME_HOST_SYSTEM_PROFILE="$FAIL_HOST_SYSTEM_PROFILE" \
  NIXOME_HOST_CURRENT_SYSTEM_LINK="$FAIL_HOST_CURRENT_SYSTEM_LINK" \
  bash "$ROOT/scripts/apply-config-candidate.sh" --apply \
  > "$WORK_DIR/fail-stdout.log" 2> "$WORK_DIR/fail-stderr.log"; then
  printf 'expected failing host system switch to fail config apply\n' >&2
  exit 1
fi

jq -e '
  .schema == "nixome.config-apply.v1" and
  .status == "failed" and
  .reason == "system apply failed" and
  .system_apply_result.status == "failed" and
  .system_apply_result.profile_status == "passed" and
  .system_apply_result.host_system_status == "skipped" and
  .system_apply_result.reason == "dom0 NixOS switch-to-configuration switch failed" and
  .deployment.status == "failed" and
  .deployment.system_apply.status == "failed" and
  .deployment_archive.status == "passed"
' "$FAIL_LOG_DIR/config-apply.json" >/dev/null

mkdir -p "$WORK_DIR/deployment-failed-extract"
tar -C "$WORK_DIR/deployment-failed-extract" -xf "$FAIL_DEPLOYMENT_TAR_CAPTURE" deployment.json system-apply.json systemd-actions.json
jq -e '
  .schema == "nixome.config-deployment.v1" and
  .status == "failed" and
  .reason == "system apply failed" and
  .system_apply.status == "failed"
' "$WORK_DIR/deployment-failed-extract/deployment.json" >/dev/null
jq -e '
  .schema == "nixome.system-apply.v1" and
  .status == "failed" and
  .reason == "dom0 NixOS switch-to-configuration switch failed"
' "$WORK_DIR/deployment-failed-extract/system-apply.json" >/dev/null
jq -e '
  .schema == "nixome.systemd-actions.v1" and
  .status == "skipped"
' "$WORK_DIR/deployment-failed-extract/systemd-actions.json" >/dev/null

printf 'config system apply test ok\n'
