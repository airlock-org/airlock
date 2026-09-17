{ lib, pkgs, ... }:
let
  recoveryNetwork = pkgs.writeShellScriptBin "airlock-recovery-network" ''
    set -eu

    if [ ! -e /etc/airlock/recovery ]; then
      echo "This command is available only in Airlock recovery mode." >&2
      exit 1
    fi
    if [ "$(id -u)" -ne 0 ]; then
      exec /run/wrappers/bin/sudo "$0" "$@"
    fi

    ${pkgs.systemd}/bin/systemctl start NetworkManager.service
    ${pkgs.systemd}/bin/systemctl start tailscaled.service
    if ! ${pkgs.networkmanager}/bin/nm-online --quiet --timeout 20; then
      echo "No network connection is ready." >&2
      echo "For Wi-Fi, connect with: sudo nmcli --ask device wifi connect YOUR_SSID" >&2
      echo "Then run airlock-recovery-network again." >&2
      exit 1
    fi
    exec ${pkgs.tailscale}/bin/tailscale up --ssh --hostname=airlock-recovery
  '';
in
{
  imports = [ ./usb-headless.nix ];

  environment.etc."airlock/boot-role".text = "recovery\n";
  environment.etc."airlock/recovery".text = ''
    Xen text recovery; VMs and networking do not start automatically.
    Run: sudo airlock-recovery-network
  '';
  environment.systemPackages = [ recoveryNetwork ];

  # Keep recovery offline until the operator explicitly invokes the command.
  systemd.services.NetworkManager.wantedBy = lib.mkForce [ ];
  systemd.services.NetworkManager-wait-online.enable = lib.mkForce false;
  systemd.services.tailscaled.wantedBy = lib.mkForce [ ];
}
