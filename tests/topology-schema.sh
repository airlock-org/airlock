#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
LOG_DIR="${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}"
RESULT_DIR="$LOG_DIR/topology-schema"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/nixome-topology-schema.XXXXXX")"
TOPOLOGY_JSON="$RESULT_DIR/topology.json"
SCHEMA="$ROOT/lib/nixome/topology.nix"

trap 'rm -rf "$WORK_DIR"' EXIT

rm -rf "$RESULT_DIR"
mkdir -p "$RESULT_DIR"
mkdir -p \
  "$WORK_DIR/valid/nixome" \
  "$WORK_DIR/valid/template-source" \
  "$WORK_DIR/valid/kernel-source" \
  "$WORK_DIR/valid/system-source"
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
        templates."nixome-built-template" = {
          import = true;
          source = {
            build = "packages.x86_64-linux.nixome-built-template";
            subpath = "nixome-built-template";
          };
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
        domains."legacy-fedora" = {
          backend = {
            type = "qubes-template";
            template = "nixome-template";
          };
          label = "yellow";
          netvm = null;
        };
        domains."nix-work" = {
          backend = {
            type = "nix-closure";
            system.source = ./system-source;
            kernel.name = "6.12.64-1.fc41";
            root.sourceRef.build = "packages.x86_64-linux.nix-work-root-image";
            root.sourceRef.subpath = "root.img";
            store.sourceRef.build = "packages.x86_64-linux.nix-work-store-image";
          };
          label = "blue";
          prefs.memory = 2048;
        };
        qrexecPolicy.rules = [
          {
            service = "nixome.PolicyProbe";
            source = "dom0";
            target = "test-qube";
            action = "allow";
          }
        ];
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
  .templates."nixome-template".source_ref == null and
  .templates."nixome-built-template".import == true and
  .templates."nixome-built-template".source == null and
  .templates."nixome-built-template".source_ref == {
    "type": "flake-output",
    "attr": "packages.x86_64-linux.nixome-built-template",
    "subpath": "nixome-built-template"
  } and
  (.kernels."6.12.64-1.fc41".source | test("/kernel-source$")) and
  .domains."test-qube".class == "AppVM" and
  .domains."test-qube".template == "nixome-template" and
  .domains."test-qube".backend.type == "qubes-template" and
  .domains."test-qube".backend.template == "nixome-template" and
  .domains."test-qube".netvm == null and
  .domains."test-qube".prefs.autostart == false and
  .domains."test-qube".prefs.include_in_backups == false and
  .domains."test-qube".prefs.memory == 1024 and
  .domains."test-qube".services."network-manager" == false and
  .domains."legacy-fedora".class == "AppVM" and
  .domains."legacy-fedora".template == "nixome-template" and
  .domains."legacy-fedora".label == "yellow" and
  .domains."legacy-fedora".backend.type == "qubes-template" and
  .domains."legacy-fedora".backend.template == "nixome-template" and
  .domains."nix-work".class == "StandaloneVM" and
  .domains."nix-work".template == null and
  .domains."nix-work".label == "blue" and
  .domains."nix-work".prefs.memory == 2048 and
  .domains."nix-work".backend.type == "nix-closure" and
  (.domains."nix-work".backend.system.source | test("/system-source$")) and
  .domains."nix-work".backend.system.source_ref == null and
  .domains."nix-work".backend.kernel.name == "6.12.64-1.fc41" and
  .domains."nix-work".backend.kernel.source == null and
  .domains."nix-work".backend.kernel.source_ref == null and
  .domains."nix-work".backend.initrd.name == null and
  .domains."nix-work".backend.modules.name == null and
  .domains."nix-work".backend.root.mode == "qubes-root-image" and
  .domains."nix-work".backend.root.source == null and
  .domains."nix-work".backend.root.source_ref == {
    "type": "flake-output",
    "attr": "packages.x86_64-linux.nix-work-root-image",
    "subpath": "root.img"
  } and
  .domains."nix-work".backend.root.read_only == false and
  .domains."nix-work".backend.store.mode == "closure-image" and
  .domains."nix-work".backend.store.source == null and
  .domains."nix-work".backend.store.source_ref == {
    "type": "flake-output",
    "attr": "packages.x86_64-linux.nix-work-store-image",
    "subpath": ""
  } and
  .domains."nix-work".backend.store.mountpoint == "/nix/store" and
  .domains."nix-work".backend.store.device == "/dev/xvde" and
  .domains."nix-work".backend.store.read_only == true and
  .domains."nix-work".backend.state.persistent == "nix-work-private" and
  .domains."nix-work".backend.state.volatile == null and
  .qrexec_policy.rules == [
    {
      "service": "nixome.PolicyProbe",
      "argument": "*",
      "source": "dom0",
      "target": "test-qube",
      "action": "allow"
    },
    {
      "service": "nixome.PolicyProbe",
      "argument": "*",
      "source": "@anyvm",
      "target": "@anyvm",
      "action": "deny"
    }
  ]
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

expect_failure \
  "template-source-and-source-ref" \
  "source and sourceRef cannot both be set" \
  '{ templates."nixome-template" = { import = true; source = ./template-source; sourceRef.build = "packages.x86_64-linux.template"; }; }'

expect_failure \
  "template-invalid-build-ref" \
  "invalid flake output reference" \
  '{ templates."nixome-template" = { import = true; source.build = "../bad"; }; }'

expect_failure \
  "non-nixome-qrexec-service" \
  "invalid qrexec service" \
  '{ qrexecPolicy.rules = [ { service = "qubes.Filecopy"; source = "dom0"; target = "vault"; action = "allow"; } ]; }'

expect_failure \
  "reserved-qrexec-service" \
  "reserved bootstrap qrexec service" \
  '{ qrexecPolicy.rules = [ { service = "nixome.Config.ArchiveCandidate"; source = "dom0"; target = "config-vault"; action = "allow"; } ]; }'

expect_failure \
  "qrexec-allow-any-source" \
  "allow rules must use dom0 or a concrete source domain" \
  '{ qrexecPolicy.rules = [ { service = "nixome.PolicyProbe"; source = "@anyvm"; target = "config-vault"; action = "allow"; } ]; }'

expect_failure \
  "qrexec-target-dom0" \
  "topology-managed qrexec policy cannot target dom0" \
  '{ qrexecPolicy.rules = [ { service = "nixome.PolicyProbe"; source = "config-vault"; target = "dom0"; action = "allow"; } ]; }'

expect_failure \
  "nix-closure-with-template" \
  "template must not be set for nix-closure domains" \
  '{ domains.bad = { template = "nixome-template"; backend.type = "nix-closure"; }; }'

expect_failure \
  "nix-closure-missing-system" \
  "nixome.topology.domains.bad.backend.system.source: must be set for nix-closure domains" \
  '{ domains.bad.backend = { type = "nix-closure"; kernel.name = "6.12.64-1.fc41"; root.sourceRef.build = "packages.x86_64-linux.bad-root"; store.sourceRef.build = "packages.x86_64-linux.bad-store"; }; }'

expect_failure \
  "nix-closure-writable-store" \
  "nix-closure store views must be read-only" \
  '{ domains.bad.backend = { type = "nix-closure"; system.sourceRef.build = "nixosConfigurations.bad.config.system.build.toplevel"; kernel.name = "6.12.64-1.fc41"; root.sourceRef.build = "packages.x86_64-linux.bad-root"; store = { sourceRef.build = "packages.x86_64-linux.bad-store"; readOnly = false; }; }; }'

expect_failure \
  "nix-closure-unsupported-store-device" \
  "nixome.topology.domains.bad.backend.store.device: currently must be /dev/xvde" \
  '{ domains.bad.backend = { type = "nix-closure"; system.sourceRef.build = "nixosConfigurations.bad.config.system.build.toplevel"; kernel.name = "6.12.64-1.fc41"; root.sourceRef.build = "packages.x86_64-linux.bad-root"; store = { sourceRef.build = "packages.x86_64-linux.bad-store"; device = "/dev/xvdf"; }; }; }'

expect_failure \
  "nix-closure-missing-root" \
  "nixome.topology.domains.bad.backend.root.source: must be set for qubes-root-image materialization" \
  '{ domains.bad.backend = { type = "nix-closure"; system.sourceRef.build = "nixosConfigurations.bad.config.system.build.toplevel"; kernel.name = "6.12.64-1.fc41"; store.sourceRef.build = "packages.x86_64-linux.bad-store"; }; }'

expect_failure \
  "nix-closure-readonly-root" \
  "qubes-root-image materialization imports a writable Qubes root volume" \
  '{ domains.bad.backend = { type = "nix-closure"; system.sourceRef.build = "nixosConfigurations.bad.config.system.build.toplevel"; kernel.name = "6.12.64-1.fc41"; root = { sourceRef.build = "packages.x86_64-linux.bad-root"; readOnly = true; }; store.sourceRef.build = "packages.x86_64-linux.bad-store"; }; }'

printf 'topology schema test ok\n'
