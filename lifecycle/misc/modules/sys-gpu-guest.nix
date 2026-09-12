{ pkgs, ... }:
{
  networking.hostName = "sys-gpu";
  hardware.graphics.enable = true;
  environment.systemPackages = [
    (pkgs.callPackage ../tools/pkgs/airlock-gpu-probe { })
    pkgs.mesa-demos
    pkgs.vulkan-tools
    pkgs.pciutils
  ];
  # Only the local diagnostic user needs render-node access. This grants no
  # Qubes administration or device-assignment authority.
  users.users.user.extraGroups = [ "render" ];
}
