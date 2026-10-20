install -D -m 0555 @agentSource@/network/setup-ip "$out/bin/setup-ip"
substituteInPlace "$out/bin/setup-ip" \
  --replace-fail "/usr/lib/qubes/init/functions" "@agent@/lib/qubes/init/functions" \
  --replace-fail "/usr/lib/qubes/qubes-setup-dnat-to-ns" "@dnat@" \
  --replace-fail "/usr/bin/qubesdb-read" "@qubesdb@/bin/qubesdb-read" \
  --replace-fail "/usr/bin/resolvectl" "@systemd@/bin/resolvectl" \
  --replace-fail "/usr/bin/nmcli" "@networkmanager@/bin/nmcli" \
  --replace-fail "/lib/systemd/systemd-sysctl" "@systemd@/lib/systemd/systemd-sysctl"
patchShebangs "$out/bin/setup-ip"
