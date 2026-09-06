#!/usr/bin/env bash
set -euo pipefail

LOG_DIR="${NIXOME_LOG_DIR:-/var/log/nixome}"
RESULT_JSON="$LOG_DIR/config-vms-smoke.json"
COMMAND_LOG="$LOG_DIR/config-vms-smoke.commands.log"
QVM_LS_LOG="$LOG_DIR/config-vms-qvm-ls.log"
STAGING_UNAME_LOG="$LOG_DIR/config-vms-config-staging-uname.log"
VAULT_UNAME_LOG="$LOG_DIR/config-vms-config-vault-uname.log"
VALIDATOR_UNAME_LOG="$LOG_DIR/config-vms-config-validator-uname.log"
CREATED_QUBE_UNAME_LOG="$LOG_DIR/config-vms-created-qube-uname.log"
STAGING_USER_LOG="$LOG_DIR/config-vms-config-staging-user.log"
VAULT_USER_LOG="$LOG_DIR/config-vms-config-vault-user.log"
VALIDATOR_USER_LOG="$LOG_DIR/config-vms-config-validator-user.log"
CREATED_QUBE_USER_LOG="$LOG_DIR/config-vms-created-qube-user.log"
STAGING_TEMPLATE_LOG="$LOG_DIR/config-vms-config-staging-template.log"
VAULT_TEMPLATE_LOG="$LOG_DIR/config-vms-config-vault-template.log"
VALIDATOR_TEMPLATE_LOG="$LOG_DIR/config-vms-config-validator-template.log"
CREATED_QUBE_TEMPLATE_LOG="$LOG_DIR/config-vms-created-qube-template.log"
STAGING_APPMENUS_LOG="$LOG_DIR/config-vms-config-staging-appmenus.log"
VAULT_APPMENUS_LOG="$LOG_DIR/config-vms-config-vault-appmenus.log"
VALIDATOR_APPMENUS_LOG="$LOG_DIR/config-vms-config-validator-appmenus.log"
STAGING_INIT_LOG="$LOG_DIR/config-vms-config-staging-init.log"
VALIDATOR_INIT_LOG="$LOG_DIR/config-vms-config-validator-init.log"
STAGING_UPDATE_SCRIPT="$LOG_DIR/config-vms-config-staging-update.sh"
STAGING_UPDATE_LOG="$LOG_DIR/config-vms-config-staging-update.log"
STAGING_REPO_STATUS_LOG="$LOG_DIR/config-vms-config-staging-repo-status.log"
VAULT_REPO_STATUS_LOG="$LOG_DIR/config-vms-config-vault-repo-status.log"
DOM0_SYSTEM_JSON_LOG="$LOG_DIR/config-vms-dom0-system.json"
DOM0_SYSTEM_PLAN_LOG="$LOG_DIR/config-vms-dom0-system-plan.json"
DOM0_SYSTEM_PROFILE_JSON_LOG="$LOG_DIR/config-vms-dom0-profile.json"
DOM0_SYSTEM_APPLY_LOG="$LOG_DIR/config-vms-dom0-system-apply.json"
DOM0_SYSTEM_PROFILE_LINK_LOG="$LOG_DIR/config-vms-dom0-system-profile-link.log"
ARCHIVE_LOG="$LOG_DIR/config-vms-archive.log"
ARCHIVE_JSON="$LOG_DIR/config-archive.json"
APPLY_LOG="$LOG_DIR/config-vms-apply.log"
APPLY_JSON="$LOG_DIR/config-apply.json"
APPLIED_TOPOLOGY_LOG="$LOG_DIR/config-vms-applied-topology.json"
QREXEC_POLICY_FILE_LOG="$LOG_DIR/config-vms-qrexec-policy.log"
QREXEC_POLICY_PROBE_LOG="$LOG_DIR/config-vms-qrexec-policy-probe.log"
APPLIED_FEATURE_LOG="$LOG_DIR/config-vms-applied-feature.log"
CREATED_QUBE_FEATURE_LOG="$LOG_DIR/config-vms-created-qube-feature.log"
VAULT_ARCHIVE_TREE_LOG="$LOG_DIR/config-vms-config-vault-archive-tree.log"
VAULT_DEPLOYMENT_TREE_LOG="$LOG_DIR/config-vms-config-vault-deployment-tree.log"
JOURNAL_LOG="$LOG_DIR/config-vms-journal.log"
LIBXL_LOG="$LOG_DIR/config-vms-libxl.log"
QVM_PREFS_LOG="$LOG_DIR/config-vms-qvm-prefs.log"

STAGING="${NIXOME_CONFIG_STAGING_VM:-config-staging}"
VAULT="${NIXOME_CONFIG_VAULT_VM:-config-vault}"
VALIDATOR="${NIXOME_CONFIG_VALIDATOR_VM:-config-validator}"
QUBE_USER="${NIXOME_QUBE_USER:-user}"
DOM0_USER="${NIXOME_DOM0_USER:-${SUDO_USER:-nixome}}"
APPLY_MESSAGE="${NIXOME_CONFIG_APPLY_MESSAGE:-smoke-applied-$(date -u +%Y%m%dT%H%M%SZ)}"
CREATED_QUBE="${NIXOME_CONFIG_CREATED_QUBE:-nixome-work-from-config}"
STAGING_CONSOLE_LOG="$LOG_DIR/config-vms-$STAGING-console.log"
VAULT_CONSOLE_LOG="$LOG_DIR/config-vms-$VAULT-console.log"
VALIDATOR_CONSOLE_LOG="$LOG_DIR/config-vms-$VALIDATOR-console.log"
CREATED_QUBE_CONSOLE_LOG="$LOG_DIR/config-vms-$CREATED_QUBE-console.log"
result_written=0

mkdir -p "$LOG_DIR"
: > "$COMMAND_LOG"

exec > >(tee -a "$COMMAND_LOG") 2>&1
trap write_unexpected_failure EXIT

json_string() {
  printf '%s' "$1" | jq -Rs . | tr -d '\n'
}

json_file_string_or_null() {
  file="$1"

  if [ -r "$file" ]; then
    jq -Rs . < "$file" | tr -d '\n'
  else
    printf 'null'
  fi
}

json_file_value_or_null() {
  file="$1"

  if [ -r "$file" ] && jq -e . "$file" >/dev/null 2>&1; then
    jq -c . "$file" | tr -d '\n'
  else
    printf 'null'
  fi
}

write_result() {
  status="$1"
  reason="$2"

  {
    printf '{'
    printf '"schema":"nixome.config-vms-smoke.v1"'
    printf ',"status":'
    json_string "$status"
    printf ',"reason":'
    json_string "$reason"
    printf ',"staging":'
    json_string "$STAGING"
    printf ',"vault":'
    json_string "$VAULT"
    printf ',"validator":'
    json_string "$VALIDATOR"
    printf ',"staging_uname":'
    json_file_string_or_null "$STAGING_UNAME_LOG"
    printf ',"vault_uname":'
    json_file_string_or_null "$VAULT_UNAME_LOG"
    printf ',"validator_uname":'
    json_file_string_or_null "$VALIDATOR_UNAME_LOG"
    printf ',"created_qube":'
    json_string "$CREATED_QUBE"
    printf ',"created_qube_uname":'
    json_file_string_or_null "$CREATED_QUBE_UNAME_LOG"
    printf ',"staging_user":'
    json_file_string_or_null "$STAGING_USER_LOG"
    printf ',"vault_user":'
    json_file_string_or_null "$VAULT_USER_LOG"
    printf ',"validator_user":'
    json_file_string_or_null "$VALIDATOR_USER_LOG"
    printf ',"created_qube_user":'
    json_file_string_or_null "$CREATED_QUBE_USER_LOG"
    printf ',"staging_template":'
    json_file_string_or_null "$STAGING_TEMPLATE_LOG"
    printf ',"vault_template":'
    json_file_string_or_null "$VAULT_TEMPLATE_LOG"
    printf ',"validator_template":'
    json_file_string_or_null "$VALIDATOR_TEMPLATE_LOG"
    printf ',"created_qube_template":'
    json_file_string_or_null "$CREATED_QUBE_TEMPLATE_LOG"
    printf ',"staging_appmenus_log":'
    json_file_string_or_null "$STAGING_APPMENUS_LOG"
    printf ',"vault_appmenus_log":'
    json_file_string_or_null "$VAULT_APPMENUS_LOG"
    printf ',"validator_appmenus_log":'
    json_file_string_or_null "$VALIDATOR_APPMENUS_LOG"
    printf ',"staging_init_log":'
    json_file_string_or_null "$STAGING_INIT_LOG"
    printf ',"validator_init_log":'
    json_file_string_or_null "$VALIDATOR_INIT_LOG"
    printf ',"staging_update_log":'
    json_file_string_or_null "$STAGING_UPDATE_LOG"
    printf ',"staging_update_script":'
    json_string "$STAGING_UPDATE_SCRIPT"
    printf ',"staging_repo_status":'
    json_file_string_or_null "$STAGING_REPO_STATUS_LOG"
    printf ',"vault_repo_status":'
    json_file_string_or_null "$VAULT_REPO_STATUS_LOG"
    printf ',"dom0_system":'
    json_file_value_or_null "$DOM0_SYSTEM_JSON_LOG"
    printf ',"dom0_system_plan":'
    json_file_value_or_null "$DOM0_SYSTEM_PLAN_LOG"
    printf ',"dom0_system_profile":'
    json_file_value_or_null "$DOM0_SYSTEM_PROFILE_JSON_LOG"
    printf ',"dom0_system_apply":'
    json_file_value_or_null "$DOM0_SYSTEM_APPLY_LOG"
    printf ',"dom0_system_profile_link":'
    json_file_string_or_null "$DOM0_SYSTEM_PROFILE_LINK_LOG"
    printf ',"archive":'
    json_file_value_or_null "$ARCHIVE_JSON"
    printf ',"archive_log":'
    json_file_string_or_null "$ARCHIVE_LOG"
    printf ',"apply":'
    json_file_value_or_null "$APPLY_JSON"
    printf ',"apply_log":'
    json_file_string_or_null "$APPLY_LOG"
    printf ',"applied_topology":'
    json_file_value_or_null "$APPLIED_TOPOLOGY_LOG"
    printf ',"qrexec_policy_file":'
    json_file_string_or_null "$QREXEC_POLICY_FILE_LOG"
    printf ',"qrexec_policy_probe":'
    json_file_string_or_null "$QREXEC_POLICY_PROBE_LOG"
    printf ',"applied_feature":'
    json_file_string_or_null "$APPLIED_FEATURE_LOG"
    printf ',"created_qube_feature":'
    json_file_string_or_null "$CREATED_QUBE_FEATURE_LOG"
    printf ',"vault_archive_tree":'
    json_file_string_or_null "$VAULT_ARCHIVE_TREE_LOG"
    printf ',"vault_deployment_tree":'
    json_file_string_or_null "$VAULT_DEPLOYMENT_TREE_LOG"
    printf ',"command_log":'
    json_string "$COMMAND_LOG"
    printf ',"qvm_ls_log":'
    json_string "$QVM_LS_LOG"
    printf ',"journal_log":'
    json_string "$JOURNAL_LOG"
    printf ',"libxl_log":'
    json_string "$LIBXL_LOG"
    printf ',"qvm_prefs_log":'
    json_string "$QVM_PREFS_LOG"
    printf ',"staging_console_log":'
    json_string "$STAGING_CONSOLE_LOG"
    printf ',"vault_console_log":'
    json_string "$VAULT_CONSOLE_LOG"
    printf ',"validator_console_log":'
    json_string "$VALIDATOR_CONSOLE_LOG"
    printf ',"created_qube_console_log":'
    json_string "$CREATED_QUBE_CONSOLE_LOG"
    printf '}\n'
  } > "$RESULT_JSON"
  result_written=1
}

fail() {
  reason="$1"
  collect_diagnostics
  write_result failed "$reason"
  printf 'config VMs smoke failed: %s\n' "$reason" >&2
  exit 1
}

write_unexpected_failure() {
  rc="$?"

  if [ "$rc" != "0" ] && [ "$result_written" != "1" ]; then
    collect_diagnostics
    write_result failed "unexpected command failure; see command_log"
  fi
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing command: $1"
  fi
}

collect_diagnostics() {
  qvm-ls > "$QVM_LS_LOG" 2>&1 || true
  {
    for vm in "$VAULT" "$STAGING" "$VALIDATOR" "$CREATED_QUBE"; do
      printf '== %s qvm-prefs ==\n' "$vm"
      if qvm-check "$vm" >/dev/null 2>&1; then
        qvm-prefs "$vm" || true
      else
        printf 'missing domain\n'
      fi
      printf '\n== %s qvm-features ==\n' "$vm"
      if qvm-check "$vm" >/dev/null 2>&1; then
        qvm-features "$vm" || true
      else
        printf 'missing domain\n'
      fi
      printf '\n'
    done
  } > "$QVM_PREFS_LOG" 2>&1 || true
  journalctl -b --no-pager -n 260 \
    -u qubesd \
    -u libvirtd \
    -u qubes-db-dom0 \
    -u qubes-qrexec-policy-daemon \
    > "$JOURNAL_LOG" 2>&1 || true
  tail -n 260 /var/log/libvirt/libxl/libxl-driver.log > "$LIBXL_LOG" 2>&1 || true
  tail -n 500 "/var/log/xen/console/guest-$VAULT.log" > "$VAULT_CONSOLE_LOG" 2>&1 || true
  tail -n 500 "/var/log/xen/console/guest-$STAGING.log" > "$STAGING_CONSOLE_LOG" 2>&1 || true
  tail -n 500 "/var/log/xen/console/guest-$VALIDATOR.log" > "$VALIDATOR_CONSOLE_LOG" 2>&1 || true
  tail -n 500 \
    "/var/log/xen/console/guest-$CREATED_QUBE.log" \
    > "$CREATED_QUBE_CONSOLE_LOG" 2>&1 || true
}

run_or_fail() {
  description="$1"
  output="$2"
  shift 2

  printf '+'
  printf ' %q' "$@"
  printf '\n'

  if ! "$@" > "$output" 2>&1; then
    cat "$output" >&2 || true
    fail "$description failed"
  fi
}

run_qrexec_or_fail() {
  vm="$1"
  description="$2"
  output="$3"
  command="$4"
  qvm_run_args=(qvm-run --pass-io)

  if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
    qvm_run_args+=(--no-gui)
  fi

  run_or_fail "$description" "$output" "${qvm_run_args[@]}" "$vm" "$command"
}

run_qrexec_script_or_fail() {
  vm="$1"
  description="$2"
  output="$3"
  script="$4"
  qvm_run_args=(qvm-run --pass-io)

  if qvm-run --help 2>&1 | grep -q -- '--no-gui'; then
    qvm_run_args+=(--no-gui)
  fi

  printf '+'
  printf ' %q' "${qvm_run_args[@]}" "$vm" "bash -s"
  printf ' < %q\n' "$script"

  if ! "${qvm_run_args[@]}" "$vm" "bash -s" < "$script" > "$output" 2>&1; then
    cat "$output" >&2 || true
    fail "$description failed"
  fi
}

run_appmenus_sync_or_fail() {
  template="$1"
  output="$2"

  if [ "$(id -u)" = 0 ]; then
    run_or_fail "sync appmenus for $template" "$output" \
      runuser -u "$DOM0_USER" -- qvm-sync-appmenus "$template"
  else
    run_or_fail "sync appmenus for $template" "$output" \
      qvm-sync-appmenus "$template"
  fi
}

start_vm() {
  vm="$1"

  if qvm-check --running "$vm" >/dev/null 2>&1; then
    printf 'VM already running: %s\n' "$vm"
    return 0
  fi

  run_or_fail "qvm-start $vm" "$LOG_DIR/config-vms-$vm-start.log" qvm-start "$vm"
}

for cmd in \
  grep \
  jq \
  journalctl \
  cat \
  date \
  nixome-config-apply \
  qrexec-client \
  readlink \
  qvm-check \
  qvm-features \
  qvm-ls \
  qvm-run \
  qvm-start \
  qvm-sync-appmenus \
  runuser \
  systemctl \
  tail \
  tr; do
  require_command "$cmd"
done

if ! [ -r /proc/xen/capabilities ] || ! grep -qw control_d /proc/xen/capabilities; then
  fail "host is not a Xen control domain"
fi

systemctl is-active --quiet qubes-db-dom0.service || fail "qubes-db-dom0.service is not active"
systemctl is-active --quiet qubesd.service || fail "qubesd.service is not active"
systemctl is-active --quiet qubes-qrexec-policy-daemon.service ||
  fail "qubes-qrexec-policy-daemon.service is not active"

case "$CREATED_QUBE" in
  ""|*[!A-Za-z0-9_.-]*)
    fail "invalid configured test qube name: $CREATED_QUBE"
    ;;
esac

case "$APPLY_MESSAGE" in
  ""|*[!A-Za-z0-9_.:-]*)
    fail "invalid configured apply message: $APPLY_MESSAGE"
    ;;
esac

qvm-check "$VAULT" >/dev/null 2>&1 || fail "missing config vault VM: $VAULT"
qvm-check "$STAGING" >/dev/null 2>&1 || fail "missing config staging VM: $STAGING"
qvm-check "$VALIDATOR" >/dev/null 2>&1 || fail "missing config validator VM: $VALIDATOR"

run_or_fail "read template for $VAULT" "$VAULT_TEMPLATE_LOG" qvm-prefs "$VAULT" template
run_or_fail "read template for $STAGING" "$STAGING_TEMPLATE_LOG" qvm-prefs "$STAGING" template
run_or_fail "read template for $VALIDATOR" "$VALIDATOR_TEMPLATE_LOG" qvm-prefs "$VALIDATOR" template
VAULT_TEMPLATE="$(tr -d '\n' < "$VAULT_TEMPLATE_LOG")"
STAGING_TEMPLATE="$(tr -d '\n' < "$STAGING_TEMPLATE_LOG")"
VALIDATOR_TEMPLATE="$(tr -d '\n' < "$VALIDATOR_TEMPLATE_LOG")"
[ -n "$VAULT_TEMPLATE" ] || fail "missing template preference for $VAULT"
[ -n "$STAGING_TEMPLATE" ] || fail "missing template preference for $STAGING"
[ -n "$VALIDATOR_TEMPLATE" ] || fail "missing template preference for $VALIDATOR"

start_vm "$VAULT"
start_vm "$STAGING"
start_vm "$VALIDATOR"
start_vm "$VAULT_TEMPLATE"
start_vm "$STAGING_TEMPLATE"
start_vm "$VALIDATOR_TEMPLATE"

run_qrexec_or_fail "$VAULT" "qrexec uname in $VAULT" "$VAULT_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$STAGING" "qrexec uname in $STAGING" "$STAGING_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$VALIDATOR" "qrexec uname in $VALIDATOR" "$VALIDATOR_UNAME_LOG" "uname -a"
run_qrexec_or_fail "$VAULT" "qrexec user in $VAULT" "$VAULT_USER_LOG" "id -un"
run_qrexec_or_fail "$STAGING" "qrexec user in $STAGING" "$STAGING_USER_LOG" "id -un"
run_qrexec_or_fail "$VALIDATOR" "qrexec user in $VALIDATOR" "$VALIDATOR_USER_LOG" "id -un"

grep -q 'Linux' "$VAULT_UNAME_LOG" || fail "vault uname output did not contain Linux"
grep -q 'Linux' "$STAGING_UNAME_LOG" || fail "staging uname output did not contain Linux"
grep -q 'Linux' "$VALIDATOR_UNAME_LOG" || fail "validator uname output did not contain Linux"
grep -qx "$QUBE_USER" "$VAULT_USER_LOG" || fail "vault qrexec user was not $QUBE_USER"
grep -qx "$QUBE_USER" "$STAGING_USER_LOG" || fail "staging qrexec user was not $QUBE_USER"
grep -qx "$QUBE_USER" "$VALIDATOR_USER_LOG" || fail "validator qrexec user was not $QUBE_USER"

run_appmenus_sync_or_fail "$VAULT_TEMPLATE" "$VAULT_APPMENUS_LOG"
run_appmenus_sync_or_fail "$STAGING_TEMPLATE" "$STAGING_APPMENUS_LOG"
run_appmenus_sync_or_fail "$VALIDATOR_TEMPLATE" "$VALIDATOR_APPMENUS_LOG"

run_qrexec_or_fail "$VAULT" "config vault repo status" "$VAULT_REPO_STATUS_LOG" \
  "cd /home/$QUBE_USER/nixome-config-vault && git status --short --branch"
run_qrexec_or_fail "$STAGING" "config staging repo status" "$STAGING_REPO_STATUS_LOG" \
  "cd /home/$QUBE_USER/nixome-config && git status --short --branch"

grep -q '^## main$' "$VAULT_REPO_STATUS_LOG" ||
  fail "config vault repository is not initialized cleanly"
grep -q '^## main$' "$STAGING_REPO_STATUS_LOG" ||
  fail "config staging repository is not initialized cleanly"

run_qrexec_or_fail "$STAGING" "config staging init" "$STAGING_INIT_LOG" "nixome-config-staging-init"
run_qrexec_or_fail \
  "$VALIDATOR" \
  "config validator init" \
  "$VALIDATOR_INIT_LOG" \
  "nixome-config-validator-init"

if [ -z "${NIXOME_CONFIG_VMS_SEED:-}" ] || [ ! -r "$NIXOME_CONFIG_VMS_SEED" ]; then
  fail "missing config VM seed helper for smoke update"
fi
if [ -z "${NIXOME_SYSTEM_SEED:-}" ] || [ ! -r "$NIXOME_SYSTEM_SEED" ]; then
  fail "missing system seed helper for smoke update"
fi
if [ -z "${NIXOME_TEMPLATES_SEED:-}" ] || [ ! -r "$NIXOME_TEMPLATES_SEED" ]; then
  fail "missing template seed helper for smoke update"
fi
if [ -z "${NIXOME_SYSTEM_SCHEMA_SEED:-}" ] || [ ! -r "$NIXOME_SYSTEM_SCHEMA_SEED" ]; then
  fail "missing system schema seed helper for smoke update"
fi
if [ -z "${NIXOME_TOPOLOGY_SCHEMA_SEED:-}" ] || [ ! -r "$NIXOME_TOPOLOGY_SCHEMA_SEED" ]; then
  fail "missing topology schema seed helper for smoke update"
fi
CONFIG_VMS_SEED_TEXT="$(cat "$NIXOME_CONFIG_VMS_SEED")"
SYSTEM_SEED_TEXT="$(cat "$NIXOME_SYSTEM_SEED")"
TEMPLATES_SEED_TEXT="$(cat "$NIXOME_TEMPLATES_SEED")"
SYSTEM_SCHEMA_SEED_TEXT="$(cat "$NIXOME_SYSTEM_SCHEMA_SEED")"
TOPOLOGY_SCHEMA_SEED_TEXT="$(cat "$NIXOME_TOPOLOGY_SCHEMA_SEED")"

cat > "$STAGING_UPDATE_SCRIPT" <<EOF
set -eu
cd /home/$QUBE_USER/nixome-config
nixpkgs_url="\$(sed -n 's/^  inputs\\.nixpkgs\\.url = "\\(.*\\)";$/\\1/p' flake.nix | head -n 1)"
nixome_support_path="\$(sed -n 's/^      nixomeSupport = "\\(.*\\)";$/\\1/p' flake.nix | head -n 1)"
nixos_qubes_path="\$(sed -n 's/^      nixosQubesSource = "\\(.*\\)";$/\\1/p' flake.nix | head -n 1)"
[ -n "\$nixpkgs_url" ]
[ -n "\$nixome_support_path" ]
[ -n "\$nixos_qubes_path" ]
mkdir -p dom0-profile/bin dom0-profile/share/nixome nixome
cat > nixome/config-vms.nix <<'CONFIGVMSNIXEOF'
$CONFIG_VMS_SEED_TEXT
CONFIGVMSNIXEOF
cat > nixome/templates.nix <<'TEMPLATESNIXEOF'
$TEMPLATES_SEED_TEXT
TEMPLATESNIXEOF
cat > nixome/topology.nix <<'TOPOLOGYNIXEOF'
$TOPOLOGY_SCHEMA_SEED_TEXT
TOPOLOGYNIXEOF
cat > nixome/system-schema.nix <<'SYSTEMSCHEMANIXEOF'
$SYSTEM_SCHEMA_SEED_TEXT
SYSTEMSCHEMANIXEOF
cat > nixome/system.nix <<'SYSTEMNIXEOF'
$SYSTEM_SEED_TEXT
SYSTEMNIXEOF
cat > dom0-profile/share/nixome/profile.json <<PROFILEJSONEOF
{"schema":"nixome.dom0-profile.v1","message":"$APPLY_MESSAGE","built_by":"config-staging"}
PROFILEJSONEOF
cat > dom0-profile/bin/nixome-dom0-profile-activate <<'PROFILEACTIVATEEOF'
#!/bin/sh
set -eu

profile_root="\$(CDPATH= cd -- "\$(dirname -- "\$0")/.." && pwd)"
state_dir="\${NIXOME_SYSTEM_STATE_DIR:-/var/lib/nixome/system}"
etc_dir="\${NIXOME_SYSTEM_ETC_DIR:-/etc/nixome}"

install -d -m 0755 "\$state_dir" "\$etc_dir"
cp "\$profile_root/share/nixome/profile.json" "\$state_dir/current-profile.json"
cp "\$profile_root/share/nixome/profile.json" "\$etc_dir/system-profile.json"

if [ -n "\${NIXOME_SYSTEM_JSON:-}" ]; then
  cp "\$NIXOME_SYSTEM_JSON" "\$state_dir/current-system.json"
fi
if [ -n "\${NIXOME_SYSTEM_PLAN_JSON:-}" ]; then
  cp "\$NIXOME_SYSTEM_PLAN_JSON" "\$state_dir/current-plan.json"
fi
if [ -n "\${NIXOME_SYSTEM_PROFILE:-}" ]; then
  printf '%s\n' "\$NIXOME_SYSTEM_PROFILE" > "\$state_dir/current-profile-link"
fi
PROFILEACTIVATEEOF
chmod 0555 dom0-profile/bin/nixome-dom0-profile-activate
cat > flake.nix <<'NIXEOF'
{
  description = "Nixome machine configuration candidate";

  inputs.nixpkgs.url = "__NIXOME_NIXPKGS_URL__";

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      topologySchema = import ./nixome/topology.nix;
      systemSchema = import ./nixome/system-schema.nix;
      configVms = import ./nixome/config-vms.nix;
      systemConfig = import ./nixome/system.nix;
      prebuiltTemplateSources = import ./nixome/prebuilt-template-sources.nix;
      nixomeSupport = "__NIXOME_SUPPORT_PATH__";
      nixosQubesSource = "__NIXOME_NIXOS_QUBES_PATH__";
      templateBuilders = import "\${nixomeSupport}/lib/nixome/qube-template.nix" {
        inherit nixpkgs system;
        inherit nixomeSupport nixosQubesSource;
      };
      templateSet = import ./nixome/templates.nix {
        inherit
          prebuiltTemplateSources
          system
          templateBuilders
          ;
      };

      appliedMessage = "$APPLY_MESSAGE";
      config = configVms {
        inherit appliedMessage;
        extraTemplates = templateSet.topology.templates;
        extraDomains = templateSet.topology.domains // {
          "$CREATED_QUBE" = {
            template = "nixome-nixos-work";
            label = "green";
            prefs = {
              memory = 1024;
              maxmem = 2048;
              vcpus = 1;
            };
            features = {
              "nixome-created-from-config" = "1";
              "nixome-created-from-config-message" = "$APPLY_MESSAGE";
              "nixome-role" = "created-from-config";
              os = "Linux";
              qrexec = "1";
            };
            services = {
              "network-manager" = false;
            };
          };
        };
      };
      topology = topologySchema.mkTopology config;
      machineSystemConfig = systemConfig { inherit appliedMessage system; };
      machineSystem = systemSchema.mkSystem machineSystemConfig;
      dom0Profile =
        let
          pkgs = templateBuilders.pkgs;
        in
        pkgs.runCommand "nixome-dom0-profile"
          {
            nativeBuildInputs = [ pkgs.jq ];
            preferLocalBuild = true;
            allowSubstitutes = false;
            appliedMessage = machineSystem.dom0.profile.message;
          }
          ''
            mkdir -p "\$out/bin" "\$out/share/nixome"
            jq -c -n \\
              --arg schema "nixome.dom0-profile.v1" \\
              --arg message "\$appliedMessage" \\
              --arg built_by "config-staging" \\
              '{
                schema: \$schema,
                message: \$message,
                built_by: \$built_by
              }' > "\$out/share/nixome/profile.json"
            cat > "\$out/bin/nixome-dom0-profile-activate" <<PROFILEACTIVATEEOF
            #!\${pkgs.runtimeShell}
            set -eu

            state_dir="''\${NIXOME_SYSTEM_STATE_DIR:-/var/lib/nixome/system}"
            etc_dir="''\${NIXOME_SYSTEM_ETC_DIR:-/etc/nixome}"

            \${pkgs.coreutils}/bin/install -d -m 0755 "\$state_dir" "\$etc_dir"
            \${pkgs.coreutils}/bin/cp "\$out/share/nixome/profile.json" "\$state_dir/current-profile.json"
            \${pkgs.coreutils}/bin/cp "\$out/share/nixome/profile.json" "\$etc_dir/system-profile.json"

            if [ -n "''\${NIXOME_SYSTEM_JSON:-}" ]; then
              \${pkgs.coreutils}/bin/cp "\$NIXOME_SYSTEM_JSON" "\$state_dir/current-system.json"
            fi
            if [ -n "''\${NIXOME_SYSTEM_PLAN_JSON:-}" ]; then
              \${pkgs.coreutils}/bin/cp "\$NIXOME_SYSTEM_PLAN_JSON" "\$state_dir/current-plan.json"
            fi
            if [ -n "''\${NIXOME_SYSTEM_PROFILE:-}" ]; then
              printf '%s\n' "\$NIXOME_SYSTEM_PROFILE" > "\$state_dir/current-profile-link"
            fi
            PROFILEACTIVATEEOF
            chmod 0555 "\$out/bin/nixome-dom0-profile-activate"
          '';
    in
    {
      packages.\${system} = templateSet.packages // {
        nixome-dom0-profile = dom0Profile;
      };

      checks.\${system} = {
        topology = templateBuilders.pkgs.runCommand "nixome-config-topology-check"
          {
            topology = builtins.toJSON topology;
          }
          ''
            printf '%s\n' "\$topology" > "\$out"
          '';
        system = templateBuilders.pkgs.runCommand "nixome-config-system-check"
          {
            system = builtins.toJSON machineSystem;
          }
          ''
            printf '%s\n' "\$system" > "\$out"
          '';
      };

      nixome = {
        metadata = {
          schema = "nixome.config-repo.v1";
          purpose = "candidate";
          topology_schema = topologySchema.schema;
          system_schema = systemSchema.schema;
        };

        inherit config;
        system = machineSystem;
        inherit topology;
      };
    };
}
NIXEOF
sed -i \
  -e "s|__NIXOME_NIXPKGS_URL__|\$nixpkgs_url|g" \
  -e "s|__NIXOME_NIXOS_QUBES_PATH__|\$nixos_qubes_path|g" \
  -e "s|__NIXOME_SUPPORT_PATH__|\$nixome_support_path|g" \
  flake.nix
grep -F 'appliedMessage = "$APPLY_MESSAGE";' flake.nix
grep -F 'configVms = import ./nixome/config-vms.nix;' flake.nix
grep -F 'systemConfig = import ./nixome/system.nix;' flake.nix
grep -F 'templateSet = import ./nixome/templates.nix' flake.nix
grep -F 'nixome-dom0-profile = dom0Profile;' flake.nix
grep -F '"$CREATED_QUBE" = {' flake.nix
git add dom0-profile/bin/nixome-dom0-profile-activate dom0-profile/share/nixome/profile.json flake.nix nixome/config-vms.nix nixome/prebuilt-template-sources.nix nixome/system.nix nixome/system-schema.nix nixome/templates.nix nixome/topology.nix
git \
  -c user.name='Nixome Config Smoke' \
  -c user.email='nixome-config-smoke@localhost' \
  commit -m 'test config creates qube'
git status --short --branch
EOF
run_qrexec_script_or_fail \
  "$STAGING" \
  "modify config candidate in $STAGING" \
  "$STAGING_UPDATE_LOG" \
  "$STAGING_UPDATE_SCRIPT"

run_or_fail "apply config candidate from $STAGING through $VALIDATOR" "$APPLY_LOG" \
  env \
    NIXOME_LOG_DIR="$LOG_DIR" \
    NIXOME_CONFIG_STAGING_VM="$STAGING" \
    NIXOME_CONFIG_VALIDATOR_VM="$VALIDATOR" \
    NIXOME_CONFIG_VAULT_VM="$VAULT" \
  nixome-config-apply --apply --staging "$STAGING" --validator "$VALIDATOR" --vault "$VAULT"

run_or_fail \
  "read applied topology" \
  "$APPLIED_TOPOLOGY_LOG" \
  cat /var/log/nixome/config-apply-topology.json
jq -e \
  --arg staging "$STAGING" \
  --arg validator "$VALIDATOR" \
  --arg vault "$VAULT" \
  --arg apply_message "$APPLY_MESSAGE" \
  --arg created "$CREATED_QUBE" \
  '
    .schema == "nixome.qubes-topology.v1" and
    .prefs.default_kernel == "" and
    .templates."nixome-nixos-work".import == true and
    (.templates."nixome-nixos-work".source | test("^/nix/store/")) and
    .templates."nixome-nixos-work".source_ref == null and
    .kernels == {} and
    .domains[$vault].class == "AppVM" and
    .domains[$vault].template == "nixome-nixos-config-system" and
    .domains[$vault].label == "black" and
    .domains[$vault].netvm == null and
    .domains[$vault].prefs.include_in_backups == true and
    .domains[$vault].features."nixome-config-authority" == "1" and
    .domains[$vault].features."nixome-role" == "config-vault" and
    .domains[$vault].services."network-manager" == false and
    .domains[$staging].class == "AppVM" and
    .domains[$staging].template == "nixome-nixos-config-system" and
    .domains[$staging].label == "gray" and
    .domains[$staging].netvm == null and
    .domains[$staging].prefs.include_in_backups == true and
    .domains[$staging].features."nixome-config-staging" == "1" and
    .domains[$staging].features."nixome-config-applied-message" == $apply_message and
    .domains[$validator].class == "AppVM" and
    .domains[$validator].template == "nixome-nixos-config-system" and
    .domains[$validator].label == "blue" and
    .domains[$validator].netvm == null and
    .domains[$validator].prefs.include_in_backups == true and
    .domains[$validator].features."nixome-config-validator" == "1" and
    .domains[$validator].features."nixome-role" == "config-validator" and
    .domains."nixome-nixos-work".class == "TemplateVM" and
    .domains."nixome-nixos-work".features."nixome-template-source" == "staged-config" and
    .domains."nixome-work".template == "nixome-nixos-work" and
    .domains[$created].template == "nixome-nixos-work"
  ' "$APPLIED_TOPOLOGY_LOG" >/dev/null ||
  fail "applied topology does not define self-hosted config VMs"

run_or_fail \
  "read generated Nixome qrexec policy" \
  "$QREXEC_POLICY_FILE_LOG" \
  cat /var/lib/nixome/qubes-policy.d/90-nixome.policy
grep -Fx "nixome.PolicyProbe * dom0 config-vault allow" "$QREXEC_POLICY_FILE_LOG" >/dev/null ||
  fail "generated qrexec policy is missing PolicyProbe allow rule"
grep -Fx "nixome.PolicyProbe * @anyvm @anyvm deny" "$QREXEC_POLICY_FILE_LOG" >/dev/null ||
  fail "generated qrexec policy is missing PolicyProbe deny rule"

run_or_fail \
  "run qrexec probe allowed by generated policy" \
  "$QREXEC_POLICY_PROBE_LOG" \
  qrexec-client -d "$VAULT" -T -w 120 "DEFAULT:QUBESRPC nixome.PolicyProbe+ dom0"
grep -Fx "nixome-policy-probe role=config-vault user=$QUBE_USER" "$QREXEC_POLICY_PROBE_LOG" >/dev/null ||
  fail "generated qrexec policy probe output did not match expected value"

run_or_fail \
  "read applied dom0 system" \
  "$DOM0_SYSTEM_JSON_LOG" \
  cat /var/lib/nixome/system/current-system.json
run_or_fail \
  "read applied dom0 system plan" \
  "$DOM0_SYSTEM_PLAN_LOG" \
  cat /var/lib/nixome/system/current-plan.json
run_or_fail \
  "read applied dom0 profile" \
  "$DOM0_SYSTEM_PROFILE_JSON_LOG" \
  cat /var/lib/nixome/system/current-profile.json
run_or_fail \
  "read system apply result" \
  "$DOM0_SYSTEM_APPLY_LOG" \
  cat /var/lib/nixome/system/last-apply.json
run_or_fail \
  "read system profile link" \
  "$DOM0_SYSTEM_PROFILE_LINK_LOG" \
  readlink /nix/var/nix/profiles/nixome-system

jq -e --arg apply_message "$APPLY_MESSAGE" '
  .schema == "nixome.system.v1" and
  .dom0.profile.message == $apply_message and
  .plan.reboot_required == false and
  any(.plan.live_actions[]; .type == "dom0-profile" and .message == $apply_message)
' "$DOM0_SYSTEM_JSON_LOG" >/dev/null ||
  fail "applied dom0 system JSON did not match expected value"

jq -e --arg apply_message "$APPLY_MESSAGE" '
  .live_actions[0].type == "dom0-profile" and
  .live_actions[0].message == $apply_message and
  .reboot_required == false
' "$DOM0_SYSTEM_PLAN_LOG" >/dev/null ||
  fail "applied dom0 system plan did not match expected value"

jq -e --arg apply_message "$APPLY_MESSAGE" '
  .schema == "nixome.dom0-profile.v1" and
  .message == $apply_message and
  .built_by == "config-staging"
' "$DOM0_SYSTEM_PROFILE_JSON_LOG" >/dev/null ||
  fail "applied dom0 profile JSON did not match expected value"

jq -e '
  .schema == "nixome.system-apply.v1" and
  .status == "passed" and
  .profile_status == "passed" and
  .reboot_required == false
' "$DOM0_SYSTEM_APPLY_LOG" >/dev/null ||
  fail "dom0 system apply JSON did not pass"

qvm-features "$STAGING" nixome-config-applied-message > "$APPLIED_FEATURE_LOG" 2>&1 ||
  fail "applied config feature is missing"
grep -qx "$APPLY_MESSAGE" "$APPLIED_FEATURE_LOG" ||
  fail "applied config feature did not match expected value"

qvm-check "$CREATED_QUBE" >/dev/null 2>&1 || fail "config-created qube is missing: $CREATED_QUBE"
run_or_fail \
  "read template for $CREATED_QUBE" \
  "$CREATED_QUBE_TEMPLATE_LOG" \
  qvm-prefs "$CREATED_QUBE" template
grep -qx "nixome-nixos-work" "$CREATED_QUBE_TEMPLATE_LOG" ||
  fail "config-created qube template did not match expected value"
qvm-features \
  "$CREATED_QUBE" \
  nixome-created-from-config-message \
  > "$CREATED_QUBE_FEATURE_LOG" 2>&1 ||
  fail "config-created qube feature is missing"
grep -qx "$APPLY_MESSAGE" "$CREATED_QUBE_FEATURE_LOG" ||
  fail "config-created qube feature did not match expected value"

start_vm "$CREATED_QUBE"
run_qrexec_or_fail \
  "$CREATED_QUBE" \
  "qrexec uname in $CREATED_QUBE" \
  "$CREATED_QUBE_UNAME_LOG" \
  "uname -a"
run_qrexec_or_fail "$CREATED_QUBE" "qrexec user in $CREATED_QUBE" "$CREATED_QUBE_USER_LOG" "id -un"
grep -q 'Linux' "$CREATED_QUBE_UNAME_LOG" || fail "created qube uname output did not contain Linux"
grep -qx "$QUBE_USER" "$CREATED_QUBE_USER_LOG" || fail "created qube qrexec user was not $QUBE_USER"

run_qrexec_or_fail \
  "$VAULT" \
  "config vault archive tree" \
  "$VAULT_ARCHIVE_TREE_LOG" \
  "cd /home/$QUBE_USER/nixome-config-vault &&
    cat latest-candidate &&
    find candidates -maxdepth 3 -type f | sort"

run_qrexec_or_fail \
  "$VAULT" \
  "config vault deployment tree" \
  "$VAULT_DEPLOYMENT_TREE_LOG" \
  "cd /home/$QUBE_USER/nixome-config-vault &&
    latest=\$(cat latest-deployment) &&
    attempt=\$(cat latest-deployment-attempt) &&
    test \"\$latest\" = \"\$attempt\" &&
    test -d \"deployments/\$latest\" &&
    jq -e '.schema == \"nixome.config-deployment.v1\" and
      .status == \"passed\"' \"deployments/\$latest/deployment.json\" &&
    jq -e '.schema == \"nixome.config-apply.v1\" and
      .status == \"passed\"' \"deployments/\$latest/apply.json\" &&
    jq -e '.schema == \"nixome.setup-config.v1\" and
      .status == \"passed\"' \"deployments/\$latest/setup.json\" &&
    jq -e '.schema == \"nixome.qubes-topology.v1\"' \"deployments/\$latest/topology.json\" &&
    find \"deployments/\$latest\" -maxdepth 2 -type f | sort"

jq -e --arg staging "$STAGING" --arg validator "$VALIDATOR" --arg vault "$VAULT" '
  .schema == "nixome.config-archive.v1" and
  .status == "passed" and
  .staging == $staging and
  .validator == $validator and
  .vault == $vault and
  .candidate_commit == .validation_commit and
  .candidate_commit == .vault_archive.candidate_commit and
  .validation.schema == "nixome.config-validation.v1" and
  .validation.status == "passed" and
  any(.validation.checks[]; .name == "nix-eval-nixome-topology" and .status == "passed") and
  any(.validation.checks[]; .name == "nixome-topology-json" and .status == "passed") and
  any(.validation.checks[]; .name == "nix-eval-nixome-system" and .status == "passed") and
  any(.validation.checks[]; .name == "nixome-system-json" and .status == "passed") and
  any(.validation.checks[]; .name == "nix-system-source-refs-dry-run" and .status == "passed")
' "$ARCHIVE_JSON" >/dev/null || fail "config archive JSON did not pass"

jq -e --arg staging "$STAGING" --arg validator "$VALIDATOR" --arg vault "$VAULT" '
  .schema == "nixome.config-apply.v1" and
  .status == "passed" and
  .staging == $staging and
  .validator == $validator and
  .vault == $vault and
  .archive.status == "passed" and
  .deployment.schema == "nixome.config-deployment.v1" and
  .deployment.status == "passed" and
  .deployment_archive.schema == "nixome.config-vault-deployment.v1" and
  .deployment_archive.status == "passed" and
  .system_apply_result.schema == "nixome.system-apply.v1" and
  .system_apply_result.status == "passed" and
  .system_apply_result.profile_status == "passed" and
  .system_apply_result.reboot_required == false and
  .deployment.system_apply.schema == "nixome.system-apply.v1" and
  .deployment.system_apply.status == "passed" and
  .setup.status == "passed" and
  .setup.domains_changed >= 1
' "$APPLY_JSON" >/dev/null || fail "config apply JSON did not pass"

collect_diagnostics
write_result passed "staged Nix created a new qrexec-capable AppVM"
cat "$RESULT_JSON"
