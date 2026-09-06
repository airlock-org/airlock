#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${NIXOME_CONFIG_APPLY_TEMPLATE_BUILD_REF_DIR:-$ROOT/.nixome-logs/config-apply-template-build-ref}"
BIN_DIR="$WORK_DIR/bin"
CANDIDATE_DIR="$WORK_DIR/candidate"
LOG_DIR="$WORK_DIR/logs"
CAPABILITIES="$WORK_DIR/xen-capabilities"
VALIDATED_TAR="$WORK_DIR/config-candidate-validated.tar"
SETUP_CAPTURE="$WORK_DIR/setup-topology.json"

rm -rf "$WORK_DIR"
mkdir -p "$BIN_DIR" "$CANDIDATE_DIR" "$LOG_DIR"
printf 'control_d\n' > "$CAPABILITIES"

nixpkgs_path="$(nix eval --raw "$ROOT#nixome.seedInputs.nixpkgs")"

cat > "$CANDIDATE_DIR/flake.nix" <<'EOF'
{
  description = "Nixome config apply template build-ref fixture";

  inputs.nixpkgs.url = "__NIXOME_NIXPKGS_URL__";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in
    {
      packages.${system}.test-template =
        pkgs.runCommand "test-template"
          {
            preferLocalBuild = true;
            allowSubstitutes = false;
          }
          ''
            mkdir -p "$out/test-template"
            printf 'root image fixture\n' > "$out/test-template/root.img"
            printf 'qrexec=1\nvirt-mode=pvh\n' > "$out/test-template/template.conf"
          '';

      nixome.topology = {
        schema = "nixome.qubes-topology.v1";
        templates.test-template = {
          import = true;
          source = null;
          source_ref = {
            type = "flake-output";
            attr = "packages.x86_64-linux.test-template";
            subpath = "test-template";
          };
          skip_start = true;
        };
        kernels = { };
        domains = { };
        prefs = { };
        qrexec_policy.rules = [ ];
      };
    };
}
EOF
sed -i "s|__NIXOME_NIXPKGS_URL__|path:$nixpkgs_path|g" "$CANDIDATE_DIR/flake.nix"

(
  cd "$CANDIDATE_DIR"
  git init -b main >/dev/null
  git add flake.nix
  nix \
    --extra-experimental-features nix-command \
    --extra-experimental-features flakes \
    flake lock \
    --impure \
    --offline
  git add flake.lock
  git \
    -c user.name='Nixome Test' \
    -c user.email='nixome-test@localhost' \
    commit -m 'fixture candidate' >/dev/null
)

CANDIDATE_COMMIT="$(git -C "$CANDIDATE_DIR" rev-parse HEAD)"
git -C "$CANDIDATE_DIR" bundle create "$WORK_DIR/config.bundle" HEAD refs/heads/main >/dev/null

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
nix \
  --extra-experimental-features nix-command \
  --extra-experimental-features flakes \
  eval \
  --impure \
  --offline \
  --no-write-lock-file \
  --json "$CANDIDATE_DIR#nixome.topology" > "$WORK_DIR/topology.json"
tar -C "$WORK_DIR" -cf "$VALIDATED_TAR" config.bundle validation.json manifest.json topology.json

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

[ -n "$topology" ] || exit 2
cp "$topology" "$SETUP_CAPTURE_FIXTURE"
jq -e '
  .templates."test-template".source != null and
  .templates."test-template".source_ref == null and
  (.templates."test-template".source | test("^/nix/store/")) and
  (.templates."test-template".source | test("/test-template$"))
' "$topology" >/dev/null
jq -c -n \
  --arg schema "nixome.setup-config.v1" \
  --arg status "passed" \
  '{schema: $schema, status: $status, domains_changed: 0, templates_changed: 1}' \
  > "$NIXOME_LOG_DIR/setup-config.json"
printf 'setup fixture ok\n'
EOF

cat > "$BIN_DIR/qrexec-client" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cat >/dev/null
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
  SETUP_CAPTURE_FIXTURE="$SETUP_CAPTURE" \
  VALIDATED_TAR_FIXTURE="$VALIDATED_TAR" \
  NIXOME_XEN_CAPABILITIES="$CAPABILITIES" \
  NIXOME_LOG_DIR="$LOG_DIR" \
  bash "$ROOT/scripts/apply-config-candidate.sh" --apply \
  > "$WORK_DIR/stdout.log" 2> "$WORK_DIR/stderr.log"

jq -e \
  --arg commit "$CANDIDATE_COMMIT" \
  '
    .schema == "nixome.config-apply.v1" and
    .status == "passed" and
    .candidate_commit == $commit and
    .setup.status == "passed" and
    .deployment.status == "passed"
  ' "$LOG_DIR/config-apply.json" >/dev/null

jq -e '
  .schema == "nixome.template-builds.v1" and
  (.builds | length) == 1 and
  .builds[0].template == "test-template" and
  .builds[0].attr == "packages.x86_64-linux.test-template" and
  .builds[0].status == "passed" and
  (.builds[0].source_path | test("^/nix/store/.*/test-template$"))
' "$LOG_DIR/config-apply-template-builds.json" >/dev/null

jq -e '
  .templates."test-template".source != null and
  .templates."test-template".source_ref == null
' "$SETUP_CAPTURE" >/dev/null

printf 'config apply template build-ref test ok\n'
