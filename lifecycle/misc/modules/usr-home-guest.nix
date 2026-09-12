{ pkgs, ... }:
{
  # `/home` is bind-mounted from the Qubes private volume by qube-guest.nix.
  # Keep the personal desktop deliberately small: this is the owner of the
  # user's files, not an all-purpose networked workstation.
  environment.systemPackages = [
    pkgs.nautilus
  ];
}
