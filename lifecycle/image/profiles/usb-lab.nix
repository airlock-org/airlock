{ lib, pkgs, ... }:
{
  imports = [ ./usb-gui-lab.nix ];

  # First general physical-machine lab stage: sys-dev owns the NIC and is the
  # intentionally privileged Tailnet entry point. A later profile will replace
  # its direct PCI networking with sys-dev -> sys-firewall -> sys-net.
  airlock.devDom0Terminal = {
    host.enable = true;
    callers = lib.mkForce [ "sys-dev" ];
  };
  networking.networkmanager.enable = lib.mkForce false;
  services.tailscale.enable = lib.mkForce false;
  services.openssh.enable = lib.mkForce false;

  airlock.qubes = {
    startNetworkProvidersOnBoot.enable = lib.mkForce true;
    prefs.default_netvm = lib.mkForce "sys-dev";
  };

  environment.etc."airlock/usb-lab".text = ''
    experimental general USB lab
    sys-dev owns physical networking and provides Tailnet development access
    run: tailscale ssh user@sys-dev; then airlock-dom0-shell
  '';
  systemd.services.airlock-start-gui-gpu.serviceConfig.ExecStartPost =
    "${pkgs.systemd}/bin/systemctl start airlock-gui-input.target";
}
