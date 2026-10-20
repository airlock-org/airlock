set -eu

check() {
  name="$1"
  expected="$2"
  actual="$3"

  if [ "$actual" != "$expected" ]; then
    printf 'contract check failed: %s expected %s, got %s\n' "$name" "$expected" "$actual" >&2
    exit 1
  fi
}

check airlock-dom0-enabled true "$airlockDom0Enabled"
check qubes-dom0-enabled true "$qubesDom0Enabled"
check qubes-secure false "$qubesSecure"
check xen-enabled true "$xenEnabled"
check initrd-systemd-enabled true "$initrdSystemdEnabled"
check qubesd-enabled true "$qubesdEnabled"
check qubes-db-dom0-enabled true "$qubesDbDom0Enabled"
check qrexec-policy-enabled true "$qrexecPolicyEnabled"
check dedicated-sys-net-opt-out true "$dedicatedSysNetOptOut"
check dedicated-sys-usb-opt-out true "$dedicatedSysUsbOptOut"
check dom0-gui-opt-out true "$dom0GuiOptOut"
check xen-package qubes-vmm-xen "$xenPackage"
check qemu-package qemu-xen "$qemuPackage"
check libvirt-package qubes-core-libvirt "$libvirtPackage"
check libvirt-firewall-backend iptables "$libvirtFirewallBackend"

printf 'airlock module contract ok\n'
touch "$out"
