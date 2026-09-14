{ ... }:
{
  # Laptop-lab appearance defaults. They are not facts about GPU ownership and
  # do not belong in the reusable Hyprland/DMS role.
  airlock.sysGuiHyprland.appearance = {
    terminalFont = "monospace:size=18";
    dmsSettings = {
      fontScale = 1.5;
      dankBarFontScale = 1.5;
      dankBarIconScale = 1.25;
      dankBarMaximizeWidgetIcons = true;
    };
  };
}
