{ pkgs }:
[
  {
    launcher = "files";
    desktopFile = "org.gnome.Nautilus";
    application = "org.gnome.Nautilus";
    desktopName = "Files";
    genericName = "File Manager";
    icon = "org.gnome.Nautilus";
    categories = [ "GNOME" "GTK" "Utility" "FileManager" ];
    mimeTypes = [ "inode/directory" ];
    package = pkgs.nautilus;
  }
  {
    launcher = "terminal";
    desktopFile = "airlock-terminal";
    application = "org.gnome.Console";
    desktopName = "Terminal";
    genericName = "Terminal Emulator";
    icon = "org.gnome.Console";
    categories = [ "GNOME" "GTK" "System" "TerminalEmulator" ];
    package = pkgs.gnome-console;
  }
  {
    launcher = "disk-usage";
    desktopFile = "airlock-disk-usage";
    application = "org.gnome.baobab";
    desktopName = "Disk Usage Analyzer";
    icon = "org.gnome.baobab";
    categories = [ "GNOME" "GTK" "System" "Filesystem" ];
    package = pkgs.baobab;
  }
  {
    launcher = "audio";
    desktopFile = "airlock-audio";
    application = "org.gnome.Decibels";
    desktopName = "Audio Player";
    icon = "org.gnome.Decibels";
    categories = [ "GNOME" "GTK" "AudioVideo" "Player" ];
    package = pkgs.decibels;
  }
  {
    launcher = "text-editor";
    desktopFile = "airlock-text-editor";
    application = "org.gnome.TextEditor";
    desktopName = "Text Editor";
    icon = "org.gnome.TextEditor";
    categories = [ "GNOME" "GTK" "Utility" "TextEditor" ];
    package = pkgs.gnome-text-editor;
  }
  {
    launcher = "calculator";
    desktopFile = "airlock-calculator";
    application = "org.gnome.Calculator";
    desktopName = "Calculator";
    icon = "org.gnome.Calculator";
    categories = [ "GNOME" "GTK" "Utility" "Calculator" ];
    package = pkgs.gnome-calculator;
  }
  {
    launcher = "calendar";
    desktopFile = "airlock-calendar";
    application = "org.gnome.Calendar";
    desktopName = "Calendar";
    icon = "org.gnome.Calendar";
    categories = [ "GNOME" "GTK" "Office" "Calendar" ];
    package = pkgs.gnome-calendar;
  }
  {
    launcher = "characters";
    desktopFile = "airlock-characters";
    application = "org.gnome.Characters";
    desktopName = "Characters";
    icon = "org.gnome.Characters";
    categories = [ "GNOME" "GTK" "Utility" ];
    package = pkgs.gnome-characters;
  }
  {
    launcher = "clocks";
    desktopFile = "airlock-clocks";
    application = "org.gnome.clocks";
    desktopName = "Clocks";
    icon = "org.gnome.clocks";
    categories = [ "GNOME" "GTK" "Utility" "Clock" ];
    package = pkgs.gnome-clocks;
  }
  {
    launcher = "contacts";
    desktopFile = "airlock-contacts";
    application = "org.gnome.Contacts";
    desktopName = "Contacts";
    icon = "org.gnome.Contacts";
    categories = [ "GNOME" "GTK" "Office" "ContactManagement" ];
    package = pkgs.gnome-contacts;
  }
  {
    launcher = "font-viewer";
    desktopFile = "airlock-font-viewer";
    application = "org.gnome.font-viewer";
    desktopName = "Fonts";
    genericName = "Font Viewer";
    icon = "org.gnome.font-viewer";
    categories = [ "GNOME" "GTK" "Utility" ];
    package = pkgs.gnome-font-viewer;
  }
  {
    launcher = "logs";
    desktopFile = "airlock-logs";
    application = "org.gnome.Logs";
    desktopName = "Logs";
    icon = "org.gnome.Logs";
    categories = [ "GNOME" "GTK" "System" "Monitor" ];
    package = pkgs.gnome-logs;
  }
  {
    launcher = "music";
    desktopFile = "airlock-music";
    application = "org.gnome.Music";
    desktopName = "Music";
    icon = "org.gnome.Music";
    categories = [ "GNOME" "GTK" "AudioVideo" "Player" ];
    package = pkgs.gnome-music;
  }
  {
    launcher = "system-monitor";
    desktopFile = "airlock-system-monitor";
    application = "org.gnome.SystemMonitor";
    desktopName = "System Monitor";
    icon = "org.gnome.SystemMonitor";
    categories = [ "GNOME" "GTK" "System" "Monitor" ];
    package = pkgs.gnome-system-monitor;
  }
  {
    launcher = "images";
    desktopFile = "airlock-images";
    application = "org.gnome.Loupe";
    desktopName = "Image Viewer";
    icon = "org.gnome.Loupe";
    categories = [ "GNOME" "GTK" "Graphics" "Viewer" ];
    package = pkgs.loupe;
  }
  {
    launcher = "documents";
    desktopFile = "airlock-documents";
    application = "org.gnome.Papers";
    desktopName = "Document Viewer";
    icon = "org.gnome.Papers";
    categories = [ "GNOME" "GTK" "Office" "Viewer" ];
    package = pkgs.papers;
  }
  {
    launcher = "videos";
    desktopFile = "airlock-videos";
    application = "org.gnome.Showtime";
    desktopName = "Video Player";
    icon = "org.gnome.Showtime";
    categories = [ "GNOME" "GTK" "AudioVideo" "Player" ];
    package = pkgs.showtime;
  }
  {
    launcher = "help";
    desktopFile = "airlock-help";
    application = "org.gnome.Yelp";
    desktopName = "Help";
    icon = "org.gnome.Yelp";
    categories = [ "GNOME" "GTK" "Utility" "Documentation" ];
    package = pkgs.yelp;
  }
]
