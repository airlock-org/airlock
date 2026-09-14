{ package }:
package.overrideAttrs (old: {
  # DMS v1.6.1 ships a stale Go vendor hash. Keep the release pinned; this is
  # the observed hash, also corrected on upstream master.
  vendorHash = "sha256-Ls6Dquwt0fzDCEjZ6FfTsZTXDI8408mFdByv/OWHVgI=";
  # DMS leaves its Spotlight launcher at fixed desktop-sized dimensions. This
  # version-pinned Quickshell patch is a HiDPI appearance profile, not a
  # compositor or hardware configuration.
  postInstall = (old.postInstall or "") + ''
    spotlight="$out/share/quickshell/dms/Modals/DankLauncherV2"
    substituteInPlace "$spotlight/SpotlightLauncherContent.qml" \
      --replace-fail 'readonly property real _searchBarH: 56' 'readonly property real _searchBarH: 72' \
      --replace-fail 'readonly property real _rowH: 64' 'readonly property real _rowH: 80' \
      --replace-fail 'Math.min(430,' 'Math.min(520,' \
      --replace-fail 'width: 36' 'width: 48' \
      --replace-fail 'height: 36' 'height: 48' \
      --replace-fail 'size: 20' 'size: 26' \
      --replace-fail 'font.pixelSize: 18' 'font.pixelSize: 24'
    substituteInPlace "$spotlight/SpotlightResultsList.qml" \
      --replace-fail 'readonly property int rowHeight: 64' 'readonly property int rowHeight: 80'
    substituteInPlace "$spotlight/SpotlightResultRow.qml" \
      --replace-fail 'height: 64' 'height: 80' \
      --replace-fail 'width: 40' 'width: 50' \
      --replace-fail 'height: 40' 'height: 50' \
      --replace-fail 'width: 30' 'width: 38' \
      --replace-fail 'height: 30' 'height: 38' \
      --replace-fail 'iconSize: 30' 'iconSize: 38'
    substituteInPlace "$spotlight/DankLauncherV2ModalSpotlight.qml" \
      --replace-fail 'Math.min(680, screenWidth - 80)' 'Math.min(800, screenWidth - 96)' \
      --replace-fail 'const searchBarH = 56' 'const searchBarH = 72' \
      --replace-fail 'screenHeight - insetB - 56' 'screenHeight - insetB - 72'
  '';
})
