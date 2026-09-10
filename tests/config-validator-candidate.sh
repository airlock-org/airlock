#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/lib/nixome-env.sh"
WORK_DIR="${NIXOME_CONFIG_VALIDATOR_CANDIDATE_DIR:-${NIXOME_LOG_DIR:-$ROOT/.nixome-logs}/config-validator-candidate}"
CANDIDATE_DIR="$WORK_DIR/candidate"
PORTABLE_SOURCE="$WORK_DIR/portable-seed-input-source"
PORTABLE_SOURCE_LINK="$WORK_DIR/portable-seed-input"
VALIDATION_DIR="$WORK_DIR/validation"
STAGED_TAR="$WORK_DIR/config-candidate-staged.tar"
VALIDATED_TAR="$WORK_DIR/config-candidate-validated.tar"
VALIDATOR_STDERR="$WORK_DIR/validator.stderr.log"
MEMBERS="$WORK_DIR/validated-members.txt"
FAIL_CANDIDATE_DIR="$WORK_DIR/failing-check-candidate"
FAIL_STAGED_TAR="$WORK_DIR/config-candidate-failing-check.tar"
FAIL_VALIDATED_TAR="$WORK_DIR/config-candidate-failing-check-validated.tar"
FAIL_VALIDATOR_STDERR="$WORK_DIR/validator-failing-check.stderr.log"

if [ -e "$WORK_DIR/nix-root" ]; then
  chmod -R u+w "$WORK_DIR/nix-root" 2>/dev/null || true
fi
rm -rf "$WORK_DIR"
mkdir -p "$CANDIDATE_DIR/dom0-profile/bin" "$CANDIDATE_DIR/dom0-profile/share/nixome" "$VALIDATION_DIR"
mkdir -p "$PORTABLE_SOURCE"
printf 'portable-validator-test\n' > "$PORTABLE_SOURCE/marker"
ln -s "$PORTABLE_SOURCE" "$PORTABLE_SOURCE_LINK"

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

  inputs.portable-source = {
    url = "path:__PORTABLE_SOURCE_LINK__";
    flake = false;
  };

  outputs = { self, portable-source }: let
    mkFixturePackage = name: builtins.derivation {
      inherit name;
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "mkdir -p $out"
      ];
    };
  in {
    packages.x86_64-linux.nix-work-system = mkFixturePackage "nix-work-system";
    packages.x86_64-linux.nix-work-store-closure = mkFixturePackage "nix-work-store-closure";

    nixome.topology = {
      schema = "nixome.qubes-topology.v1";
      templates = { };
      kernels = { };
      domains = {
        nix-native-work = {
          class = "StandaloneVM";
          template = null;
          label = "green";
          netvm = null;
          prefs = { };
          features = { };
          services = { };
          backend = {
            type = "nix-closure";
            template = null;
            system = {
              source = null;
              source_ref = {
                type = "flake-output";
                attr = "packages.x86_64-linux.nix-work-system";
                subpath = "";
              };
            };
            kernel = {
              name = "6.12.64-1.fc41";
              source = null;
              source_ref = null;
            };
            initrd = {
              name = null;
              source = null;
              source_ref = null;
            };
            modules = {
              name = null;
              source = null;
              source_ref = null;
            };
            store = {
              mode = "shared-closure";
              source = null;
              source_ref = {
                type = "flake-output";
                attr = "packages.x86_64-linux.nix-work-store-closure";
                subpath = "";
              };
              mountpoint = "/nix/store";
              device = "/dev/xvde";
              blob_device = "/dev/xvdf";
              view_backend = "erofs-objectmap";
              read_only = true;
            };
            state = {
              persistent = "nix-native-work-private";
              volatile = null;
            };
          };
        };
      };
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
          message = builtins.readFile "${portable-source}/marker";
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
        reboot_actions = [ ];
        reboot_required = false;
        reboot_reasons = [ ];
      };
    };
  };
}
EOF
sed -i "s|__PORTABLE_SOURCE_LINK__|$PORTABLE_SOURCE_LINK|g" "$CANDIDATE_DIR/flake.nix"

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
    "$ROOT#nixosConfigurations.nixome-domain-config-validator.config.environment.etc.\"qubes-rpc/nixome.Config.ValidateCandidate\".source"
)"

NIXOME_CONFIG_VALIDATION_DIR="$VALIDATION_DIR" \
  NIXOME_CONFIG_NIX_STORE_ROOT="$WORK_DIR/nix-root" \
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
  any(.checks[]; .name == "nix-template-source-refs-dry-run" and .status == "passed") and
  any(.checks[]; .name == "nixome-topology-json" and .status == "passed") and
  any(.checks[]; .name == "nixome-system-json" and .status == "passed") and
  any(.checks[]; .name == "nix-flake-check" and .status == "passed")
' "$WORK_DIR/validation.json" >/dev/null

jq -e '.schema == "nixome.qubes-topology.v1"' "$WORK_DIR/topology.json" >/dev/null
jq -e '.schema == "nixome.system.v1" and .dom0.profile.message == "portable-validator-test\n"' \
  "$WORK_DIR/system.json" >/dev/null
topology_refs_log="$(jq -r '.checks[] | select(.name == "nix-template-source-refs-dry-run").log' "$WORK_DIR/validation.json")"
grep -F 'checking nix-domain=nix-native-work.system attr=packages.x86_64-linux.nix-work-system' \
  "$topology_refs_log" >/dev/null
grep -F 'checking nix-domain=nix-native-work.store attr=packages.x86_64-linux.nix-work-store-closure' \
  "$topology_refs_log" >/dev/null

mkdir -p "$FAIL_CANDIDATE_DIR"
cat > "$FAIL_CANDIDATE_DIR/flake.nix" <<'EOF'
{
  description = "Nixome config validator failing check fixture";

  outputs = { self }: {
    checks.x86_64-linux.failing = derivation {
      name = "nixome-failing-check";
      system = "x86_64-linux";
      builder = "/bin/sh";
      args = [
        "-c"
        "echo failing check >&2; exit 23"
      ];
    };

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
          source = null;
          source_ref = null;
          activation_command = "bin/nixome-dom0-profile-activate";
          activation_action = "none";
          message = "";
        };
        packages = [ ];
        systemd.services = { };
        nixos = {
          system = {
            enable = false;
            reason = "no host system change";
            source = null;
            source_ref = null;
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
  cd "$FAIL_CANDIDATE_DIR"
  git init -b main >/dev/null
  git add flake.nix
  git \
    -c user.name='Nixome Validator Test' \
    -c user.email='nixome-validator-test@localhost' \
    commit -m 'failing flake check fixture' >/dev/null
  fail_commit="$(git rev-parse HEAD)"
  git bundle create "$WORK_DIR/failing-check.bundle" HEAD >/dev/null
  jq -c -n \
    --arg schema "nixome.config-candidate.v1" \
    --arg commit "$fail_commit" \
    '{ schema: $schema, commit: $commit }' > "$WORK_DIR/failing-check-manifest.json"
)

tar -C "$WORK_DIR" -cf "$FAIL_STAGED_TAR" \
  --transform 's|failing-check.bundle|config.bundle|' \
  --transform 's|failing-check-manifest.json|manifest.json|' \
  failing-check.bundle \
  failing-check-manifest.json

fail_commit="$(git -C "$FAIL_CANDIDATE_DIR" rev-parse HEAD)"
if NIXOME_CONFIG_VALIDATION_DIR="$VALIDATION_DIR" \
  NIXOME_CONFIG_NIX_STORE_ROOT="$WORK_DIR/nix-root" \
  nixome_run_store_path "$validator_cmd" \
  < "$FAIL_STAGED_TAR" \
  > "$FAIL_VALIDATED_TAR" \
  2> "$FAIL_VALIDATOR_STDERR"; then
  printf 'expected failing flake check candidate to fail validation\n' >&2
  exit 1
fi

jq -e \
  --arg commit "$fail_commit" \
  '
    .schema == "nixome.config-validation.v1" and
    .status == "failed" and
    .commit == $commit and
    any(.checks[]; .name == "nix-flake-check" and .status == "failed") and
    any(.checks[]; .name == "nix-flake-check-no-build" and .status == "passed")
  ' "$VALIDATION_DIR/$fail_commit.json" >/dev/null

printf 'config validator candidate tar ok\n'
