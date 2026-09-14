{ pkgs, ... }:
{
  imports = [ ./usb-headless.nix ];
  # Device selection belongs to the imported machine hardware profile.
  # Forward only after qvm-start has brought up the guest and its qrexec agent.
  airlock.inputProxy.sender.enable = true;
  systemd.services.airlock-start-gui-gpu.serviceConfig.ExecStartPost =
    "${pkgs.systemd}/bin/systemctl start airlock-gui-input.target";
  environment.etc."airlock/usb-amd-lab".text =
    "experimental; declarative GPU assignment and one boot start\n";
  airlock.guiGpu = {
    enable = true;
    startOnBoot = true;
  };
  airlock.amdVbios = {
    enable = true;
    domains.sys-gui = { };
  };
  # Nothing here relaxes reset checks or enables permissive PCI access.
  # The prepared physical guest boots to its XFCE/LightDM login screen.
}
