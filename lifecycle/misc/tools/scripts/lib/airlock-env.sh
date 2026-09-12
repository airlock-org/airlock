#!/usr/bin/env bash

if [ -n "${AIRLOCK_DATA_ROOT:-}" ]; then
  AIRLOCK_DEFAULT_DATA_ROOT="$AIRLOCK_DATA_ROOT"
elif [ -d /run/media/user/data ] && [ -w /run/media/user/data ]; then
  AIRLOCK_DEFAULT_DATA_ROOT="/run/media/user/data/airlock"
else
  AIRLOCK_DEFAULT_DATA_ROOT=""
fi

if [ -z "$AIRLOCK_DEFAULT_DATA_ROOT" ] && [ "${AIRLOCK_USE_HOST_NIX_STORE:-0}" != "1" ]; then
  printf 'error: /run/media/user/data is not available; refusing to use the host Nix store by default\n' >&2
  printf 'set AIRLOCK_DATA_ROOT to another data-backed path, or set AIRLOCK_USE_HOST_NIX_STORE=1 explicitly\n' >&2
  return 1 2>/dev/null || exit 1
fi

if [ -n "$AIRLOCK_DEFAULT_DATA_ROOT" ]; then
  export AIRLOCK_DATA_ROOT="${AIRLOCK_DATA_ROOT:-$AIRLOCK_DEFAULT_DATA_ROOT}"
  export AIRLOCK_VM_DIR="${AIRLOCK_VM_DIR:-$AIRLOCK_DATA_ROOT/vm}"
  export AIRLOCK_LOG_DIR="${AIRLOCK_LOG_DIR:-$AIRLOCK_DATA_ROOT/logs}"
  export AIRLOCK_QUBES_CACHE_DIR="${AIRLOCK_QUBES_CACHE_DIR:-$AIRLOCK_DATA_ROOT/cache/rpms}"
  export AIRLOCK_RESULT_LINK_DIR="${AIRLOCK_RESULT_LINK_DIR:-$AIRLOCK_DATA_ROOT/result-links}"
  export AIRLOCK_NIX_ROOT="${AIRLOCK_NIX_ROOT:-$AIRLOCK_DATA_ROOT/nix-root}"
  export AIRLOCK_NIX_STORE="${AIRLOCK_NIX_STORE:-$AIRLOCK_NIX_ROOT}"
  export TMPDIR="${TMPDIR:-$AIRLOCK_DATA_ROOT/tmp}"
  export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$AIRLOCK_DATA_ROOT/cache/xdg}"

  mkdir -p \
    "$AIRLOCK_VM_DIR" \
    "$AIRLOCK_LOG_DIR" \
    "$AIRLOCK_QUBES_CACHE_DIR" \
    "$AIRLOCK_RESULT_LINK_DIR" \
    "$AIRLOCK_NIX_ROOT" \
    "$TMPDIR" \
    "$XDG_CACHE_HOME"

  # Default Airlock builds and tests must use the data-root-backed Nix store,
  # not the host /nix/store. The shared Airlock store is intentional: it keeps
  # repeated local tests from rebuilding the same closures. Use the host store
  # only with the explicit AIRLOCK_USE_HOST_NIX_STORE=1 override.
  if [ "${AIRLOCK_USE_HOST_NIX_STORE:-0}" != "1" ] && [ "${AIRLOCK_NIX_CONFIGURED:-0}" != "1" ]; then
    if [ -n "${NIX_CONFIG:-}" ]; then
      export NIX_CONFIG="${NIX_CONFIG}"$'\n'"store = $AIRLOCK_NIX_STORE"
    else
      export NIX_CONFIG="store = $AIRLOCK_NIX_STORE"
    fi
    export AIRLOCK_NIX_CONFIGURED=1
  fi
fi

airlock_store_uses_chroot() {
  [ "${AIRLOCK_USE_HOST_NIX_STORE:-0}" != "1" ] &&
    [ -n "${AIRLOCK_NIX_ROOT:-}" ] &&
    [ "$AIRLOCK_NIX_ROOT" != "/" ]
}

airlock_physical_store_path() {
  local path="$1"

  if airlock_store_uses_chroot && [ "${path#/nix/store/}" != "$path" ]; then
    printf '%s%s\n' "$AIRLOCK_NIX_ROOT" "$path"
  else
    printf '%s\n' "$path"
  fi
}

airlock_physical_path_list() {
  local path_list="$1"
  local old_ifs="$IFS"
  local parts=()
  local translated=()
  local part

  IFS=:
  read -r -a parts <<< "$path_list"
  IFS="$old_ifs"

  for part in "${parts[@]}"; do
    translated+=("$(airlock_physical_store_path "$part")")
  done

  IFS=:
  printf '%s\n' "${translated[*]}"
  IFS="$old_ifs"
}

airlock_store_path_root() {
  local path="$1"

  if [[ "$path" =~ ^(/nix/store/[^/]+)($|/) ]]; then
    printf '%s\n' "${BASH_REMATCH[1]}"
  else
    printf '%s\n' "$path"
  fi
}

airlock_run_store_path() {
  local command_path="$1"
  shift

  if airlock_store_uses_chroot && [ "${command_path#/nix/store/}" != "$command_path" ]; then
    nix shell "$(airlock_store_path_root "$command_path")" -c "$command_path" "$@"
  else
    "$command_path" "$@"
  fi
}

airlock_exec_store_path() {
  local command_path="$1"
  shift

  if airlock_store_uses_chroot && [ "${command_path#/nix/store/}" != "$command_path" ]; then
    exec nix shell "$(airlock_store_path_root "$command_path")" -c "$command_path" "$@"
  else
    exec "$command_path" "$@"
  fi
}
