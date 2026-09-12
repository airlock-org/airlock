{ pkgs, ... }:
let
  openUrl = pkgs.writeShellScript "airlock-open-browser-url" ''
    set -eu

    url="''${QREXEC_SERVICE_ARGUMENT:-}"
    case "$url" in
      http://*|https://*) ;;
      *)
        printf 'airlock.OpenUrl accepts only http and https URLs\n' >&2
        exit 2
        ;;
    esac

    exec ${pkgs.firefox}/bin/firefox --new-window "$url"
  '';
in
{
  # Firefox stores its profile below /home/user/.mozilla, which is on this
  # domain's private Qubes volume. No usr-home volume is attached here.
  environment.systemPackages = [
    pkgs.firefox
  ];

  # A fixed, validated URL-opening RPC lets the sys-gui browser desktop entry
  # preserve URI activation without granting it a general shell in this VM.
  environment.etc."qubes-rpc/airlock.OpenUrl" = {
    source = openUrl;
    mode = "0555";
  };
  environment.etc."qubes/rpc-config/airlock.OpenUrl" = {
    text = "force-user='user'\n";
    mode = "0444";
  };
}
