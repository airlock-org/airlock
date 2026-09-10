{ ... }:
{
  # Development-only image flavor: installs temporary dom0 tools and leaves
  # direct dom0 DHCP available in the throwaway nested VM. sys-net remains
  # configured the same way as non-dev images.
  nixome.image.devMode.enable = true;
}
