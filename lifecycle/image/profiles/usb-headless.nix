{ lib, ... }:
{
  # Keep dom0's existing Qubes policy, but no local display transport or
  # automatic guest desktop session. Tailscale, SSH and local gettys remain.
  services.xserver.enable = lib.mkForce false;
  services.xserver.displayManager.lightdm.enable = lib.mkForce false;
  services.displayManager.autoLogin.enable = lib.mkForce false;
  systemd.defaultUnit = "multi-user.target";
  systemd.services.display-manager.enable = lib.mkForce false;
}
