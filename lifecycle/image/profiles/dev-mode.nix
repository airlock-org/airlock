{ ... }:
{
  # Development-only image flavor: installs temporary dom0 tools and routes
  # dom0 HTTP(S) traffic through sys-firewall's Qubes updates proxy.
  nixome.image.devMode.enable = true;
}
