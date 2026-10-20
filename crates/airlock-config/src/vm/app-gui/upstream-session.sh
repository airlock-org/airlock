    cp @agent@/bin/qubes-session "$out"
    chmod u+w "$out"
    substituteInPlace "$out" \
      --replace-fail /usr/bin/qubes-session-autostart @autostart@ \
      --replace-fail '# Wait some time to pick up zombie children' \
        '@coreutils@/bin/touch "${XDG_RUNTIME_DIR:-/run/user/$UID}/airlock-app-session-ready"

# Wait some time to pick up zombie children'
