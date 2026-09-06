#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
WORK_DIR="${NIXOME_CONFIG_VALIDATOR_CANDIDATE_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-validator-candidate}"
CANDIDATE_DIR="$WORK_DIR/candidate"
VALIDATION_DIR="$WORK_DIR/validation"
STAGED_TAR="$WORK_DIR/config-candidate-staged.tar"
VALIDATED_TAR="$WORK_DIR/config-candidate-validated.tar"
VALIDATOR_STDERR="$WORK_DIR/validator.stderr.log"
MEMBERS="$WORK_DIR/validated-members.txt"

rm -rf "$WORK_DIR"
mkdir -p "$CANDIDATE_DIR/dom0-profile/bin" "$CANDIDATE_DIR/dom0-profile/share/nixome" "$VALIDATION_DIR"

cat > "$CANDIDATE_DIR/dom0-profile/share/nixome/profile.json" <<'EOF'
{"schema":"nixome.dom0-profile.v1","message":"validator-test","built_by":"config-validator-candidate-test"}
EOF

cat > "$CANDIDATE_DIR/dom0-profile/bin/nixome-dom0-profile-activate" <<'EOF'
#!/bin/sh
set -eu
exit 0
EOF
chmod 0755 "$CANDIDATE_DIR/dom0-profile/bin/nixome-dom0-profile-activate"

cat > "$CANDIDATE_DIR/flake.nix" <<'EOF'
{
  description = "Nixome config validator fixture";

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
          message = "validator-test";
        };
        packages = [ ];
        systemd.services = { };
        nixos = {
          system = {
            enable = false;
            reason = "dom0 system change requires reboot";
            source = null;
            source_ref = null;
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
        live_actions = [ ];
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
    -c user.name='Nixome Validator Test' \
    -c user.email='nixome-validator-test@localhost' \
    commit -m 'validator fixture' >/dev/null
  commit="$(git rev-parse HEAD)"
  git bundle create "$WORK_DIR/config.bundle" HEAD >/dev/null
  jq -c -n \
    --arg schema "nixome.config-candidate.v1" \
    --arg commit "$commit" \
    '{ schema: $schema, commit: $commit }' > "$WORK_DIR/manifest.json"
)

tar -C "$WORK_DIR" -cf "$STAGED_TAR" config.bundle manifest.json

validator_cmd="$(
  nix build \
    --no-link \
    --print-out-paths \
    "$ROOT#nixosConfigurations.qube-template-config-system.config.environment.etc.\"qubes-rpc/nixome.Config.ValidateCandidate\".source"
)"

NIXOME_CONFIG_VALIDATION_DIR="$VALIDATION_DIR" \
  nixome_run_store_path "$validator_cmd" \
  < "$STAGED_TAR" \
  > "$VALIDATED_TAR" \
  2> "$VALIDATOR_STDERR"

tar -tf "$VALIDATED_TAR" | sort > "$MEMBERS"
grep -Fx config.bundle "$MEMBERS" >/dev/null
grep -Fx manifest.json "$MEMBERS" >/dev/null
grep -Fx validation.json "$MEMBERS" >/dev/null
grep -Fx topology.json "$MEMBERS" >/dev/null
grep -Fx system.json "$MEMBERS" >/dev/null

tar -C "$WORK_DIR" -xf "$VALIDATED_TAR" validation.json topology.json system.json

jq -e '
  .schema == "nixome.config-validation.v1" and
  .status == "passed" and
  any(.checks[]; .name == "nixome-topology-json" and .status == "passed") and
  any(.checks[]; .name == "nixome-system-json" and .status == "passed")
' "$WORK_DIR/validation.json" >/dev/null

jq -e '.schema == "nixome.qubes-topology.v1"' "$WORK_DIR/topology.json" >/dev/null
jq -e '.schema == "nixome.system.v1"' "$WORK_DIR/system.json" >/dev/null

printf 'config validator candidate tar ok\n'
