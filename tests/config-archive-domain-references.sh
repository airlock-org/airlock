#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
WORK_DIR="${NIXOME_CONFIG_ARCHIVE_DOMAIN_REFERENCES_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-archive-domain-references}"
CANDIDATE_DIR="$WORK_DIR/candidate"
STAGED_TAR="$WORK_DIR/config-candidate-staged.tar"
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

jq -c -n \
  --arg schema "nixome.config-candidate.v1" \
  --arg commit "$CANDIDATE_COMMIT" \
  --arg branch "main" \
  '{schema: $schema, commit: $commit, branch: $branch, source_role: "dev"}' \
  > "$WORK_DIR/manifest.json"

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

tar -C "$WORK_DIR" -cf "$STAGED_TAR" config.bundle manifest.json
tar -C "$WORK_DIR" -cf "$VALIDATED_TAR" config.bundle validation.json manifest.json topology.json system.json

run_case() {
  case_name="$1"
  sys_firewall_present="$2"
  sys_net_present="$3"
  expect_status="$4"
  expect_reason="${5:-}"
  case_dir="$WORK_DIR/$case_name"
  bin_dir="$case_dir/bin"
  log_dir="$case_dir/logs"

  mkdir -p "$bin_dir" "$log_dir"

  cat > "$bin_dir/qrexec-client" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
case "$*" in
  *"nixome.Config.ValidateCandidate"*)
    cat >/dev/null
    cat "$VALIDATED_TAR_FIXTURE"
    ;;
  *)
    printf 'unexpected qrexec-client args: %s\n' "$*" >&2
    exit 1
    ;;
esac
EOF

  cat > "$bin_dir/qvm-check" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "${1:-}" = "config-validator" ]
EOF

  cat > "$bin_dir/qvm-prefs" <<EOF
#!/usr/bin/env bash
set -euo pipefail
case "\${1:-}:\${2:-}" in
  config-validator:netvm)
    printf '\n'
    ;;
  config-validator:provides_network)
    printf 'False\n'
    ;;
  sys-firewall:provides_network)
    [ "$sys_firewall_present" = "1" ] || exit 1
    printf 'True\n'
    ;;
  sys-firewall:netvm)
    [ "$sys_firewall_present" = "1" ] || exit 1
    printf 'sys-net\n'
    ;;
  sys-net:provides_network)
    [ "$sys_net_present" = "1" ] || exit 1
    printf 'True\n'
    ;;
  sys-net:netvm)
    [ "$sys_net_present" = "1" ] || exit 1
    printf '\n'
    ;;
  *)
    printf 'unexpected qvm-prefs args: %s\n' "\$*" >&2
    exit 1
    ;;
esac
EOF

  cat > "$bin_dir/qvm-start" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
[ "${1:-}" = "config-validator" ]
EOF

  cat > "$bin_dir/qvm-ls" <<EOF
#!/usr/bin/env bash
  if [ "\$*" = "--raw-data --fields NAME --no-spinner" ]; then
  printf 'dom0\n'
  printf 'dev\n'
  if [ "$sys_firewall_present" = "1" ]; then
    printf 'sys-firewall\n'
  fi
  if [ "$sys_net_present" = "1" ]; then
    printf 'sys-net\n'
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
    "$bin_dir/qrexec-client" \
    "$bin_dir/qvm-check" \
    "$bin_dir/qvm-prefs" \
    "$bin_dir/qvm-start" \
    "$bin_dir/qvm-ls" \
    "$bin_dir/systemctl"

  set +e
  PATH="$bin_dir:$PATH" \
    VALIDATED_TAR_FIXTURE="$VALIDATED_TAR" \
    NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
    NIXOME_LOG_DIR="$log_dir" \
    bash "$ROOT/scripts/archive-config-candidate.sh" \
      --candidate-tar "$STAGED_TAR" \
      --validate-only \
    > "$case_dir/stdout.log" 2> "$case_dir/stderr.log"
  rc="$?"
  set -e

  if [ "$expect_status" = "passed" ]; then
    [ "$rc" -eq 0 ]
    jq -e '
      .schema == "nixome.config-archive.v1" and
      .status == "passed" and
      .domain_references_result.status == "passed" and
      any(.domain_references_result.references[]; .kind == "netvm" and .domain == "dev" and .target == "sys-firewall")
    ' "$log_dir/config-archive.json" >/dev/null
  else
    [ "$rc" -ne 0 ]
    jq -e --arg reason "$expect_reason" '
      .schema == "nixome.config-archive.v1" and
      .status == "failed" and
      .reason == $reason and
      .domain_references_result.status == "failed"
    ' "$log_dir/config-archive.json" >/dev/null

    if [ "$expect_reason" = "topology references missing domains" ]; then
      jq -e '
      .domain_references_result.missing == [
        {
          "kind": "netvm",
          "domain": "dev",
          "field": "netvm",
          "target": "sys-firewall"
        }
      ]
      ' "$log_dir/config-archive.json" >/dev/null
    else
      jq -e '
        any(.domain_references_result.missing[];
          .kind == "stock-network-chain" and
          .domain == "sys-firewall" and
          .field == "netvm" and
          .target == "sys-net" and
          .reason == "stock sys-firewall upstream sys-net is missing")
      ' "$log_dir/config-archive.json" >/dev/null
    fi
  fi
}

run_case missing-sys-firewall 0 0 failed "topology references missing domains"
run_case missing-sys-net 1 0 failed "topology references unusable network domains"
run_case present 1 1 passed

printf 'config archive domain references test ok\n'
