set -eu
@ncurses@/bin/clear
boot_id="$(@coreutils@/bin/tr -d '\n' </proc/sys/kernel/random/boot_id)"
echo
echo "  Airlock | Boot failed"
echo
echo "  A hardware check or required VM failed. Airlock has not accepted this boot."
echo "  The failed stage and diagnostics have been saved."
echo
echo "  Evidence:     /var/log/airlock/boot-attempts/$boot_id"
echo "  Attempt state: /var/lib/airlock/boot-attempts/$boot_id/state.json"
echo "  Failed units:  systemctl --failed"
echo "  Input report:  /var/log/airlock/boot-attempts/$boot_id/input-controller.json"
echo
echo "  Log in below to inspect the failure."
echo
exec @utilLinux@/bin/agetty --noclear tty1 linux
