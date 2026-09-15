{ pkgs, ... }:
{
  # `/home` is bind-mounted from the Qubes private volume by qube-guest.nix.
  # Keep the desktop-work VM deliberately small: it owns the user's files and
  # terminal, while sys-gui owns the display and starts these applications by
  # fixed qrexec services. It remains offline through its null NetVM.
  environment.systemPackages = [
    pkgs.nautilus
    pkgs.gnome-console
  ];
}
