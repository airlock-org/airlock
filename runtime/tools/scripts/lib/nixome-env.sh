#!/usr/bin/env bash

if [ -n "${NIXOME_DATA_ROOT:-}" ]; then
  NIXOME_DEFAULT_DATA_ROOT="$NIXOME_DATA_ROOT"
elif [ -d /run/media/user/data ] && [ -w /run/media/user/data ]; then
  NIXOME_DEFAULT_DATA_ROOT="/run/media/user/data/nixome"
else
  NIXOME_DEFAULT_DATA_ROOT=""
fi

if [ -z "$NIXOME_DEFAULT_DATA_ROOT" ] && [ "${NIXOME_USE_HOST_NIX_STORE:-0}" != "1" ]; then
  printf 'error: /run/media/user/data is not available; refusing to use the host Nix store by default\n' >&2
  printf 'set NIXOME_DATA_ROOT to another data-backed path, or set NIXOME_USE_HOST_NIX_STORE=1 explicitly\n' >&2
  return 1 2>/dev/null || exit 1
fi

if [ -n "$NIXOME_DEFAULT_DATA_ROOT" ]; then
  export NIXOME_DATA_ROOT="${NIXOME_DATA_ROOT:-$NIXOME_DEFAULT_DATA_ROOT}"
  export NIXOME_VM_DIR="${NIXOME_VM_DIR:-$NIXOME_DATA_ROOT/vm}"
  export NIXOME_LOG_DIR="${NIXOME_LOG_DIR:-$NIXOME_DATA_ROOT/logs}"
  export NIXOME_QUBES_CACHE_DIR="${NIXOME_QUBES_CACHE_DIR:-$NIXOME_DATA_ROOT/cache/rpms}"
  export NIXOME_RESULT_LINK_DIR="${NIXOME_RESULT_LINK_DIR:-$NIXOME_DATA_ROOT/result-links}"
  export NIXOME_NIX_ROOT="${NIXOME_NIX_ROOT:-$NIXOME_DATA_ROOT/nix-root}"
  export NIXOME_NIX_STORE="${NIXOME_NIX_STORE:-$NIXOME_NIX_ROOT}"
  export TMPDIR="${TMPDIR:-$NIXOME_DATA_ROOT/tmp}"
  export XDG_CACHE_HOME="${XDG_CACHE_HOME:-$NIXOME_DATA_ROOT/cache/xdg}"

  mkdir -p \
    "$NIXOME_VM_DIR" \
    "$NIXOME_LOG_DIR" \
    "$NIXOME_QUBES_CACHE_DIR" \
    "$NIXOME_RESULT_LINK_DIR" \
    "$NIXOME_NIX_ROOT" \
    "$TMPDIR" \
    "$XDG_CACHE_HOME"

  # Default Nixome builds and tests must use the data-root-backed Nix store,
  # not the host /nix/store. The shared Nixome store is intentional: it keeps
  # repeated local tests from rebuilding the same closures. Use the host store
  # only with the explicit NIXOME_USE_HOST_NIX_STORE=1 override.
  if [ "${NIXOME_USE_HOST_NIX_STORE:-0}" != "1" ] && [ "${NIXOME_NIX_CONFIGURED:-0}" != "1" ]; then
    if [ -n "${NIX_CONFIG:-}" ]; then
      export NIX_CONFIG="${NIX_CONFIG}"$'\n'"store = $NIXOME_NIX_STORE"
    else
      export NIX_CONFIG="store = $NIXOME_NIX_STORE"
    fi
    export NIXOME_NIX_CONFIGURED=1
  fi
fi

nixome_store_uses_chroot() {
  [ "${NIXOME_USE_HOST_NIX_STORE:-0}" != "1" ] &&
    [ -n "${NIXOME_NIX_ROOT:-}" ] &&
    [ "$NIXOME_NIX_ROOT" != "/" ]
}

nixome_physical_store_path() {
  local path="$1"

  if nixome_store_uses_chroot && [ "${path#/nix/store/}" != "$path" ]; then
    printf '%s%s\n' "$NIXOME_NIX_ROOT" "$path"
  else
    printf '%s\n' "$path"
  fi
}

nixome_physical_path_list() {
  local path_list="$1"
  local old_ifs="$IFS"
  local parts=()
  local translated=()
  local part

  IFS=:
  read -r -a parts <<< "$path_list"
  IFS="$old_ifs"

  for part in "${parts[@]}"; do
    translated+=("$(nixome_physical_store_path "$part")")
  done

  IFS=:
  printf '%s\n' "${translated[*]}"
  IFS="$old_ifs"
}

nixome_store_path_root() {
  local path="$1"

  if [[ "$path" =~ ^(/nix/store/[^/]+)($|/) ]]; then
    printf '%s\n' "${BASH_REMATCH[1]}"
  else
    printf '%s\n' "$path"
  fi
}

nixome_run_store_path() {
  local command_path="$1"
  shift

  if nixome_store_uses_chroot && [ "${command_path#/nix/store/}" != "$command_path" ]; then
    nix shell "$(nixome_store_path_root "$command_path")" -c "$command_path" "$@"
  else
    "$command_path" "$@"
  fi
}

nixome_exec_store_path() {
  local command_path="$1"
  shift

  if nixome_store_uses_chroot && [ "${command_path#/nix/store/}" != "$command_path" ]; then
    exec nix shell "$(nixome_store_path_root "$command_path")" -c "$command_path" "$@"
  else
    exec "$command_path" "$@"
  fi
}
