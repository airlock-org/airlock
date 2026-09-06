#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/topology-schema"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-topology-schema.XXXXXX")"
TOPOLOGY_JSON="$RESULT_DIR/topology.json"
SCHEMA="$ROOT/lib/nixome/topology.nix"

trap 'rm -rf "$WORK_DIR"' EXIT

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"
mkdir -p "$WORK_DIR/valid/nixome" "$WORK_DIR/valid/template-source" "$WORK_DIR/valid/kernel-source"
cp "$SCHEMA" "$WORK_DIR/valid/nixome/topology.nix"

cat > "$WORK_DIR/valid/flake.nix" <<'EOF'
{
  description = "Nixome topology schema valid fixture";

  outputs =
    { self }:
    let
      topology = import ./nixome/topology.nix;
    in
    {
      nixome.topology = topology.mkTopology {
        templates."nixome-template" = {
          import = true;
          source = ./template-source;
        };
        kernels."6.12.64-1.fc41".source = ./kernel-source;
        prefs.default_kernel = "";
        domains."test-qube" = {
          template = "nixome-template";
          label = "green";
          prefs.memory = 1024;
          features."nixome-role" = "test";
          services."network-manager" = false;
        };
      };
    };
}
EOF

nix eval --json "$WORK_DIR/valid#nixome.topology" > "$TOPOLOGY_JSON"

jq -e '
  .schema == "nixome.qubes-topology.v1" and
  .templates."nixome-template".import == true and
  .templates."nixome-template".skip_start == true and
  (.templates."nixome-template".source | test("/template-source$")) and
  (.kernels."6.12.64-1.fc41".source | test("/kernel-source$")) and
  .domains."test-qube".class == "AppVM" and
  .domains."test-qube".template == "nixome-template" and
  .domains."test-qube".netvm == null and
  .domains."test-qube".prefs.autostart == false and
  .domains."test-qube".prefs.include_in_backups == false and
  .domains."test-qube".prefs.memory == 1024 and
  .domains."test-qube".services."network-manager" == false
' "$TOPOLOGY_JSON" >/dev/null

expect_failure() {
  name="$1"
  needle="$2"
  config_expr="$3"
  dir="$WORK_DIR/$name"
  log="$RESULT_DIR/$name.log"

  mkdir -p "$dir/nixome"
  cp "$SCHEMA" "$dir/nixome/topology.nix"
  cat > "$dir/flake.nix" <<EOF
{
  description = "Nixome topology schema invalid fixture";

  outputs =
    { self }:
    let
      topology = import ./nixome/topology.nix;
    in
    {
      nixome.topology = topology.mkTopology $config_expr;
    };
}
EOF

  if nix eval --json "$dir#nixome.topology" > "$RESULT_DIR/$name-topology.json" 2> "$log"; then
    printf 'schema accepted invalid config: %s\n' "$name" >&2
    exit 1
  fi

  grep -F "$needle" "$log" >/dev/null || {
    printf 'schema failure did not mention expected text for %s\n' "$name" >&2
    printf 'expected: %s\n' "$needle" >&2
    cat "$log" >&2
    exit 1
  }
}

expect_failure \
  "missing-appvm-template" \
  "nixome.topology.domains.bad.template: must be set for AppVM domains" \
  '{ domains.bad = { label = "green"; }; }'

expect_failure \
  "invalid-label" \
  "nixome.topology.domains.bad.label: expected one of" \
  '{ domains.bad = { template = "nixome-template"; label = "clear"; }; }'

expect_failure \
  "unknown-option" \
  "nixome.topology.domains.bad: unknown option(s): servies" \
  '{ domains.bad = { template = "nixome-template"; servies = { }; }; }'

expect_failure \
  "template-import-without-source" \
  "nixome.topology.templates.nixome-template.source: must be set when import is true" \
  '{ templates."nixome-template".import = true; }'

printf 'topology schema test ok\n'
