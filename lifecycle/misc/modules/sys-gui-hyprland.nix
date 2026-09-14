{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.airlock.sysGuiHyprland;
  shellStart = pkgs.writeShellScript "airlock-hyprland-shell-start" ''
    set -eu
    ${pkgs.dbus}/bin/dbus-update-activation-environment --systemd \
      WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_TYPE HYPRLAND_INSTANCE_SIGNATURE
    ${pkgs.systemd}/bin/systemctl --user start airlock-desktop-shell.service
  '';
  logout = "${config.programs.hyprland.package}/bin/hyprctl dispatch 'hl.dsp.exit()'";
  settings = pkgs.writeText "airlock-dms-settings.json" (
    builtins.toJSON {
      # These are first-login defaults, not a security boundary. Host controls
      # must continue to use narrowly scoped Qubes RPCs, not local VM power APIs.
      powerMenuActions = [
        "logout"
        "poweroff"
      ];
      powerMenuDefaultAction = "logout";
      customPowerActionLogout = logout;
      customPowerActionPowerOff = "${pkgs.python3.pkgs.qubes-core-admin-client}/bin/qvm-run --pass-io --no-gui --service dom0 airlock.Dom0Poweroff";
      # A comfortable default for the lab's HiDPI display. Both scales are
      # exposed by DMS and remain editable in the user's settings file.
      fontScale = 1.5;
      dankBarFontScale = 1.5;
      # DMS keeps its bar-icon controls separate from its text scale.
      dankBarIconScale = 1.25;
      dankBarMaximizeWidgetIcons = true;
      controlCenterWidgets = [
        {
          id = "darkMode";
          enabled = true;
          width = 100;
        }
      ];
    }
  );
  hyprConfig = pkgs.writeText "airlock-hyprland.lua" ''
    -- Hyprland 0.56 uses Lua. No laptop-specific outputs or PCI addresses.
    -- Let each display's density determine scaling, including HiDPI panels.
    hl.monitor({ output = "", mode = "preferred", position = "auto", scale = "auto" })
    hl.config({
      input = { kb_layout = "${config.services.xserver.xkb.layout}" },
      general = { gaps_in = 4, gaps_out = 8, border_size = 2 },
      misc = { disable_hyprland_logo = true },
    })
    hl.on("hyprland.start", function ()
      hl.exec_cmd("${shellStart}")
      hl.exec_cmd("${pkgs.foot}/bin/foot -f 'monospace:size=15'")
    end)
    hl.bind("SUPER + Return", hl.dsp.exec_cmd("${pkgs.foot}/bin/foot -f 'monospace:size=15'"))
    hl.bind("SUPER + D", hl.dsp.exec_cmd("${cfg.shellPackage}/bin/dms ipc call spotlight toggle"))
    hl.bind("SUPER + SHIFT + D", hl.dsp.exec_cmd("airlock-dom0-terminal"))
    hl.bind("SUPER + SHIFT + E", hl.dsp.exit())
    hl.bind("SUPER + Q", hl.dsp.window.close())
    hl.bind("SUPER + V", hl.dsp.window.float({ action = "toggle" }))
    hl.bind("SUPER + mouse:272", hl.dsp.window.drag(), { mouse = true })
    hl.bind("SUPER + mouse:273", hl.dsp.window.resize(), { mouse = true })
    for i = 1, 9 do
      hl.bind("SUPER + " .. i, hl.dsp.focus({ workspace = i }))
      hl.bind("SUPER + SHIFT + " .. i, hl.dsp.window.move({ workspace = i }))
    end
  '';
  session = pkgs.writeShellScript "airlock-hyprland-session" ''
    set -eu
    config_dir="''${XDG_CONFIG_HOME:-$HOME/.config}"
    state_dir="''${XDG_STATE_HOME:-$HOME/.local/state}/airlock"
    mkdir -p "$config_dir/DankMaterialShell" "$state_dir"
    # Preserve interactive customization on subsequent logins.
    if [ ! -e "$config_dir/DankMaterialShell/settings.json" ]; then
      install -m 0600 ${settings} "$config_dir/DankMaterialShell/settings.json"
    fi
    export XDG_CURRENT_DESKTOP=Hyprland XDG_SESSION_DESKTOP=Hyprland XDG_SESSION_TYPE=wayland
    unset LD_PRELOAD
    compositor_pid=""
    cleanup() {
      ${pkgs.systemd}/bin/systemctl --user stop airlock-desktop-shell.service || true
      if [ -n "$compositor_pid" ]; then kill "$compositor_pid" 2>/dev/null || true; fi
    }
    trap cleanup EXIT
    trap 'exit 143' TERM
    trap 'exit 130' INT
    # Fixed lab config avoids overwriting the user's own Hyprland configuration.
    /run/wrappers/bin/Hyprland --config ${hyprConfig} >> "$state_dir/hyprland.log" 2>&1 &
    compositor_pid=$!
    wait "$compositor_pid"
  '';
  sessionPackage =
    pkgs.runCommand "airlock-hyprland-session"
      {
        passthru.providedSessions = [ "airlock-hyprland" ];
      }
      ''
        mkdir -p $out/share/wayland-sessions
        cat > $out/share/wayland-sessions/airlock-hyprland.desktop <<EOF
        [Desktop Entry]
        Name=Airlock Hyprland (experimental)
        Comment=Hyprland and DankMaterialShell in sys-gui
        Exec=${session}
        Type=Application
        DesktopNames=Hyprland
        EOF
      '';
in
{
  options.airlock.sysGuiHyprland = {
    enable = lib.mkEnableOption "the optional physical sys-gui Hyprland session";
    shellPackage = lib.mkOption {
      type = lib.types.package;
      description = "Pinned DankMaterialShell package; Quickshell remains a separate runtime.";
    };
    shellCommand = lib.mkOption {
      type = lib.types.str;
      default = "${cfg.shellPackage}/bin/dms run --session";
      description = "Desktop-shell command, replaceable with a different Quickshell layout.";
    };
  };
  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = config.airlock.sysGuiGuest.enable && config.airlock.sysGuiGuest.displayMode != "hybrid";
        message = "Airlock Hyprland requires a physical-GPU sys-gui; the hybrid relay remains X11-only.";
      }
    ];
    programs.hyprland.enable = true;
    programs.hyprland.xwayland.enable = true;
    programs.foot = {
      enable = true;
      settings.main.font = "monospace:size=15";
    };
    environment.systemPackages = [
      cfg.shellPackage
      pkgs.quickshell
      pkgs.foot
      pkgs.wayland-utils
    ];
    services.displayManager.sessionPackages = [ sessionPackage ];
    # No WantedBy: only the explicit Airlock Hyprland session starts this unit.
    # Its wrapper stops it on logout, including compositor failure.
    systemd.user.services.airlock-desktop-shell = {
      description = "Airlock sys-gui desktop shell (Quickshell)";
      path = [
        "/run/current-system/sw"
        "/run/wrappers"
        pkgs.quickshell
        pkgs.coreutils
        pkgs.bash
        pkgs.systemd
        config.programs.hyprland.package
      ];
      serviceConfig = {
        ExecStart = cfg.shellCommand;
        Restart = "on-failure";
        RestartSec = 2;
        TimeoutStopSec = 5;
      };
    };
  };
}
