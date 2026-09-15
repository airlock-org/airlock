{ pkgs, ... }:
let
  applications = import ../lib/usr-home-applications.nix { inherit pkgs; };
in
{
  imports = [ ./app-gui-guest.nix ];
  # `/home` is bind-mounted from the Qubes private volume by qube-guest.nix.
  # Keep the desktop-work VM deliberately small: it owns the user's files and
  # terminal, while sys-gui owns the display and starts these applications by
  # fixed qrexec services. It remains offline through its null NetVM.
  environment.systemPackages = map (application: application.package) applications;
}
