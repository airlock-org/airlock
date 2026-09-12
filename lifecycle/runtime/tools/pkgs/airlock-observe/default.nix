{
  coreutils,
  gnugrep,
  gnused,
  gawk,
  lib,
  systemd,
  util-linux,
  writeShellApplication,
}:

writeShellApplication {
  name = "airlock-observe-dom0";

  runtimeInputs = [
    coreutils
    gnugrep
    gnused
    gawk
    systemd
    util-linux
  ];

  text = ''
    set -eu

    json_escape() {
      printf '%s' "$1" | sed \
        -e 's/\\/\\\\/g' \
        -e 's/"/\\"/g' \
        -e ':a;N;$!ba;s/\n/\\n/g'
    }

    json_string() {
      printf '"'
      json_escape "$1"
      printf '"'
    }

    command_available() {
      command -v "$1" >/dev/null 2>&1
    }

    bool() {
      if "$@"; then
        printf true
      else
        printf false
      fi
    }

    service_active() {
      systemctl is-active --quiet "$1" >/dev/null 2>&1
    }

    service_state() {
      state="$(systemctl is-active "$1" 2>/dev/null || true)"
      if [ -n "$state" ]; then
        printf '%s' "$state"
      else
        printf unknown
      fi
    }

    service_json() {
      service="$1"
      printf '{"name":'
      json_string "$service"
      printf ',"active":'
      bool service_active "$service"
      printf ',"state":'
      json_string "$(service_state "$service")"
      printf '}'
    }

    file_contains_word() {
      file="$1"
      word="$2"

      [ -r "$file" ] && grep -qw "$word" "$file"
    }

    xen_caps=""
    if [ -r /proc/xen/capabilities ]; then
      xen_caps="$(cat /proc/xen/capabilities)"
    fi

    xen_version=""
    if command_available xl; then
      xen_version="$(xl info 2>/dev/null | awk -F: '/^xen_version/ { gsub(/^[ \t]+/, "", $2); print $2; exit }' || true)"
    fi

    domains_count=""
    if command_available qvm-ls; then
      domains_count="$(qvm-ls --raw-data --fields NAME --no-spinner 2>/dev/null | wc -l | tr -d ' ' || true)"
    fi

    printf '{'
    printf '"schema":"airlock.dom0-observation.v1"'
    printf ',"host":{'
    printf '"hostname":'
    json_string "$(hostname)"
    printf ',"kernel":'
    json_string "$(uname -r)"
    printf ',"architecture":'
    json_string "$(uname -m)"
    printf ',"virtualization":'
    json_string "$(systemd-detect-virt 2>/dev/null || true)"
    printf '}'
    printf ',"xen":{'
    printf '"proc_xen":'
    bool test -e /proc/xen
    printf ',"control_domain":'
    bool file_contains_word /proc/xen/capabilities control_d
    printf ',"capabilities":'
    json_string "$xen_caps"
    printf ',"xl_available":'
    bool command_available xl
    printf ',"xen_version":'
    json_string "$xen_version"
    printf '}'
    printf ',"services":['
    service_json qubes-db-dom0.service
    printf ','
    service_json qubesd.service
    printf ','
    service_json qubes-qrexec-policy-daemon.service
    printf ','
    service_json qubes-core.service
    printf ','
    service_json xenstored.service
    printf ','
    service_json libvirtd.service
    printf ']'
    printf ',"qubes":{'
    printf '"qvm_ls_available":'
    bool command_available qvm-ls
    printf ',"qvm_run_available":'
    bool command_available qvm-run
    printf ',"domains_count":'
    if [ -n "$domains_count" ]; then
      printf '%s' "$domains_count"
    else
      printf 'null'
    fi
    printf '}'
    printf '}\n'
  '';

  meta = {
    description = "Emit Airlock dom0 control-stack observations as JSON";
    license = lib.licenses.mit;
    platforms = [ "x86_64-linux" ];
  };
}
