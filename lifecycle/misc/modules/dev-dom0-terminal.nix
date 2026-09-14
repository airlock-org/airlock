{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.airlock.devDom0Terminal;
  shell = pkgs.writeShellScript "airlock-dev-dom0-login" ''
    set -eu
    echo "dom0-terminal: PTY opened (uid=$EUID); preparing login shell."
    ${pkgs.util-linux}/bin/logger -t airlock-dev-dom0-terminal -- "PTY opened uid=$EUID" || true
    cd "$HOME"
    export TERM=xterm-256color
    export TERMINFO_DIRS=${pkgs.ncurses}/share/terminfo
    ${pkgs.coreutils}/bin/stty rows 30 cols 100
    echo 'DEVELOPMENT: dom0 shell. Use sudo for administration; exit to disconnect.'
    exec ${pkgs.bashInteractive}/bin/bash -l
  '';
  handler = pkgs.writeShellScript "airlock-dev-dom0-shell" ''
    set -eu
    # Qubes' dom0 service path forwards stdout, but leaves the handler's
    # stderr in dom0. Expose startup/PAM errors in this administrative terminal.
    # Never tee the interactive stream: it can contain passwords and secrets.
    exec 2>&1
    stage() {
      echo "dom0-terminal: $*"
      ${pkgs.util-linux}/bin/logger -t airlock-dev-dom0-terminal -- "$*" || true
    }
    stage "handler reached (uid=$EUID)"
    [ "''${QREXEC_REMOTE_DOMAIN:-}" = sys-gui ] || {
      stage 'refused: caller is not sys-gui'
      exit 1
    }
    if [ -n "''${QREXEC_SERVICE_ARGUMENT:-}" ] || [ -n "''${1:-}" ] || [ "$#" -gt 1 ]; then
      stage 'refused: this service accepts no arguments'
      exit 1
    fi
    # Policy executes this fixed handler as root, but the interactive shell
    # and its PTY run as the ordinary dom0 user. No passwordless sudo is added.
    stage 'starting the unprivileged dom0 PTY session'
    exec ${pkgs.util-linux}/bin/runuser -u ${lib.escapeShellArg cfg.user} -- \
      ${pkgs.util-linux}/bin/script --quiet --return --flush \
      --command ${shell} /dev/null
  '';
  policy = pkgs.runCommand "airlock-dev-dom0-terminal-policy" { } ''
    install -d "$out/etc/qubes/policy.d"
    cat > "$out/etc/qubes/policy.d/40-airlock-dev-dom0-terminal.policy" <<'EOF'
    # Intentional development-only dom0 user shell, not a production capability.
    airlock.DevDom0Shell + sys-gui dom0 allow user=root
    airlock.DevDom0Shell * @anyvm dom0 deny
    airlock.DevDom0Shell * @anyvm @anyvm deny
    EOF
  '';
  client = pkgs.writeShellScript "airlock-dev-dom0-terminal-client" ''
    set -eu
    saved="$(${pkgs.coreutils}/bin/stty -g)"
    trap '${pkgs.coreutils}/bin/stty "$saved"' EXIT
    trap 'exit 1' HUP INT TERM
    echo 'Connecting to dom0 through Qubes...'
    # The remote script supplies the PTY: pass keys (including Ctrl-C) through
    # unchanged and avoid a second local echo. Only dom0 output is trusted.
    ${pkgs.coreutils}/bin/stty raw -echo
    result=0
    ${pkgs.python3.pkgs.qubes-core-qrexec.domU}/bin/qrexec-client-vm \
      --no-filter-escape-chars-stdout --no-filter-escape-chars-stderr \
      dom0 airlock.DevDom0Shell || result=$?
    ${pkgs.coreutils}/bin/stty "$saved"
    printf '\nDom0 connection ended (exit code %s).\n' "$result"
    if [ "$result" -ne 0 ]; then
      echo 'Please report the messages above. If no handler message appeared, inspect dom0 qrexec logs.'
    fi
    echo 'You can close this window.'
    exit "$result"
  '';
  launcher = pkgs.writeShellScriptBin "airlock-dom0-terminal" ''
    exec ${pkgs.xterm}/bin/xterm -hold -T 'dom0 terminal — DEVELOPMENT' \
      -geometry 100x30 -e ${client}
  '';
  desktop = pkgs.makeDesktopItem {
    name = "airlock-dom0-terminal";
    desktopName = "dom0 Terminal (Development)";
    comment = "Administrative dom0 access; sudo requires the dom0 password";
    exec = "airlock-dom0-terminal";
    icon = "utilities-terminal";
    categories = [
      "System"
      "TerminalEmulator"
    ];
  };
in
{
  options.airlock.devDom0Terminal = {
    host.enable = lib.mkEnableOption "the development-only sys-gui to dom0 shell RPC";
    guest.enable = lib.mkEnableOption "the development-only dom0 terminal launcher";
    user = lib.mkOption {
      type = lib.types.strMatching "[a-z_][a-z0-9_-]*";
      default = "airlock";
      description = "Unprivileged dom0 account; sudo policy remains unchanged.";
    };
  };
  config = lib.mkMerge [
    (lib.mkIf cfg.host.enable {
      assertions = [
        {
          assertion =
            config.airlock.image.devMode.enable
            && config.airlock.dom0.enable
            && config.users.users.${cfg.user}.isNormalUser;
          message = "The dom0 terminal requires a development image and an ordinary dom0 user.";
        }
      ];
      virtualisation.qubes.packages = [ policy ];
      environment.etc."qubes-rpc/airlock.DevDom0Shell" = {
        source = handler;
        mode = "0555";
      };
    })
    (lib.mkIf cfg.guest.enable {
      environment.systemPackages = [
        launcher
        desktop
      ];
    })
  ];
}
