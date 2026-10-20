install -D -m 0555 @agent@/lib/qubes/init/network-proxy-setup.sh "$out/bin/network-proxy-setup"
substituteInPlace "$out/bin/network-proxy-setup" \
  --replace-fail "@agent@/lib/qubes/qubes-setup-dnat-to-ns" "@dnat@"
