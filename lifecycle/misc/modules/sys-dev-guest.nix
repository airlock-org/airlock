{ pkgs, ... }:
{
  # sys-dev is an intentionally privileged USB-lab development domain. It
  # owns the NIC during the first networking stage and is the Tailnet entry
  # point for the development-only dom0 qrexec shell.
  services.tailscale = {
    enable = true;
    extraUpFlags = [ "--ssh" ];
  };
  environment.etc."qubes-rpc/airlock.DevTailscaleUp" = {
    source = pkgs.writeShellScript "airlock-dev-tailscale-up" ''
      set -eu
      [ "''${QREXEC_REMOTE_DOMAIN:-}" = sys-gui ] || exit 1
      [ -z "''${QREXEC_SERVICE_ARGUMENT:-}" ] || exit 1
      exec ${pkgs.tailscale}/bin/tailscale up --ssh
    '';
    mode = "0555";
  };
  environment.etc."qubes/rpc-config/airlock.DevTailscaleUp" = {
    text = "force-user='root'\n";
    mode = "0444";
  };
  # USB-lab's local dom0 operator needs to perform the one-time Tailnet
  # enrollment before remote access exists. This is intentionally confined to
  # the privileged development domain.
  airlock.qubeGuest.allowQrexecUserSelection = true;
  networking.firewall.interfaces.tailscale0.allowedTCPPorts = [ 22 ];
  airlock.devDom0Terminal.guest.enable = true;
}
