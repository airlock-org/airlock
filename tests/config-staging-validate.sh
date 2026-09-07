#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
WORK_DIR="${NIXOME_CONFIG_STAGING_VALIDATE_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-staging-validate}"
CANDIDATE_DIR="$WORK_DIR/candidate"
VALIDATION_DIR="$WORK_DIR/validation"
STDOUT_LOG="$WORK_DIR/stdout.log"
STDERR_LOG="$WORK_DIR/stderr.log"
DIRTY_STDOUT_LOG="$WORK_DIR/dirty-stdout.log"
DIRTY_STDERR_LOG="$WORK_DIR/dirty-stderr.log"

rm -rf "$WORK_DIR"
mkdir -p "$CANDIDATE_DIR/dom0-profile/bin" "$CANDIDATE_DIR/dom0-profile/share/nixome" "$VALIDATION_DIR"

cat > "$CANDIDATE_DIR/dom0-profile/bin/nixome-dom0-profile-activate" <<'EOF'
#!/bin/sh
set -eu
exit 0
EOF
chmod 0755 "$CANDIDATE_DIR/dom0-profile/bin/nixome-dom0-profile-activate"

cat > "$CANDIDATE_DIR/dom0-profile/share/nixome/profile.json" <<'EOF'
{"schema":"nixome.dom0-profile.v1","message":"staging-validate-test"}
EOF

cat > "$CANDIDATE_DIR/flake.nix" <<'EOF'
{
  description = "Nixome staging validation fixture";

  outputs = { self }: {
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
          source = "dom0-profile";
          source_ref = null;
          activation_command = "bin/nixome-dom0-profile-activate";
          activation_action = "none";
          message = "staging-validate-test";
        };
        packages = [ ];
        systemd.services = { };
        nixos = {
          system = {
            enable = false;
            reason = "no host switch in staging validation fixture";
            source = null;
            source_ref = null;
            switch_action = "switch";
          };
          kernel = {
            enable = false;
            reason = "no kernel change";
            source = null;
            source_ref = null;
          };
          xen = {
            enable = false;
            reason = "no xen change";
            source = null;
            source_ref = null;
          };
          bootloader = {
            enable = false;
            reason = "no bootloader change";
            source = null;
            source_ref = null;
          };
          qubes = {
            enable = false;
            reason = "no qubes package change";
            source = null;
            source_ref = null;
          };
        };
      };
      plan = {
        live_actions = [ ];
        reboot_actions = [ ];
        reboot_required = false;
        reboot_reasons = [ ];
      };
    };
  };
}
EOF

(
  cd "$CANDIDATE_DIR"
  git init -b main >/dev/null
  git add dom0-profile/bin/nixome-dom0-profile-activate dom0-profile/share/nixome/profile.json flake.nix
  git \
    -c user.name='Nixome Staging Test' \
    -c user.email='nixome-staging-test@localhost' \
    commit -m 'staging validation fixture' >/dev/null
)

commit="$(git -C "$CANDIDATE_DIR" rev-parse HEAD)"
validator_package="$(
  nix build \
    --no-link \
    --print-out-paths \
    "$ROOT#nixosConfigurations.nixome-domain-config-staging.config.environment.etc.\"nixome/guest-tools/nixome-config-staging-validate\".source"
)"
validator_tool="$validator_package/bin/nixome-config-staging-validate"

NIXOME_CONFIG_REPO_PATH="$CANDIDATE_DIR" \
  NIXOME_CONFIG_VALIDATION_DIR="$VALIDATION_DIR" \
  nixome_run_store_path "$validator_tool" \
  > "$STDOUT_LOG" \
  2> "$STDERR_LOG"

jq -e \
  --arg commit "$commit" \
  --arg repo "$CANDIDATE_DIR" \
  --arg topology "$VALIDATION_DIR/topology-$commit.json" \
  --arg system "$VALIDATION_DIR/system-$commit.json" \
  --arg validated "$VALIDATION_DIR/validated-candidate-$commit.tar" \
  '
    .schema == "nixome.config-validation.v1" and
    .status == "passed" and
    .commit == $commit and
    .repo == $repo and
    .validated_candidate == $validated and
    any(.checks[]; .name == "nix-eval-nixome-topology" and .status == "passed" and .output == $topology) and
    any(.checks[]; .name == "nixome-topology-json" and .status == "passed" and .path == $topology) and
    any(.checks[]; .name == "nix-eval-nixome-system" and .status == "passed" and .output == $system) and
    any(.checks[]; .name == "nixome-system-json" and .status == "passed" and .path == $system) and
    any(.checks[]; .name == "nix-system-source-refs-dry-run" and .status == "passed") and
    any(.checks[]; .name == "nix-flake-check-no-build" and .status == "passed")
  ' "$STDOUT_LOG" >/dev/null

jq -e '.schema == "nixome.config-validation.v1" and .status == "passed"' \
  "$VALIDATION_DIR/$commit.json" >/dev/null
jq -e '.schema == "nixome.qubes-topology.v1" and .domains == {}' \
  "$VALIDATION_DIR/topology-$commit.json" >/dev/null
jq -e '.schema == "nixome.system.v1" and .dom0.profile.message == "staging-validate-test"' \
  "$VALIDATION_DIR/system-$commit.json" >/dev/null
tar -tf "$VALIDATION_DIR/validated-candidate-$commit.tar" | sort > "$WORK_DIR/validated-members.txt"
grep -Fx config.bundle "$WORK_DIR/validated-members.txt" >/dev/null
grep -Fx manifest.json "$WORK_DIR/validated-members.txt" >/dev/null
grep -Fx validation.json "$WORK_DIR/validated-members.txt" >/dev/null
grep -Fx topology.json "$WORK_DIR/validated-members.txt" >/dev/null
grep -Fx system.json "$WORK_DIR/validated-members.txt" >/dev/null

printf 'dirty\n' >> "$CANDIDATE_DIR/README.md"
if NIXOME_CONFIG_REPO_PATH="$CANDIDATE_DIR" \
  NIXOME_CONFIG_VALIDATION_DIR="$VALIDATION_DIR" \
  nixome_run_store_path "$validator_tool" \
  > "$DIRTY_STDOUT_LOG" \
  2> "$DIRTY_STDERR_LOG"; then
  printf 'expected dirty staging validation to fail\n' >&2
  exit 1
fi

jq -e '
  .schema == "nixome.config-validation.v1" and
  .status == "failed" and
  .validator_role == "config-staging-local" and
  any(.checks[]; .name == "git-clean" and .status == "failed")
' "$DIRTY_STDOUT_LOG" >/dev/null

printf 'config staging validate test ok\n'
