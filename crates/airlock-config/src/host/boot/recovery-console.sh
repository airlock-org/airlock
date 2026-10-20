set -eu
@service@/bin/airlock-dom0-service boot-supervisor recovery-ack
@ncurses@/bin/clear
echo
echo "  Airlock | Safe boot recovery"
echo
echo "  The previous boot did not finish hardware and desktop startup checks."
echo "  PCI-backed VMs and GPU handoff are disabled for this boot."
echo
echo "  Persistent evidence: /var/log/airlock/boot-attempts"
echo "  Attempt state:      /var/lib/airlock/boot-attempts"
echo "  Failed units:       systemctl --failed"
echo
echo "  Log in below, inspect the evidence, then reboot to make one new attempt."
echo
exec @utilLinux@/bin/agetty --noclear tty1 linux
