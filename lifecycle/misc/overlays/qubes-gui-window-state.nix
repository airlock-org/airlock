final: prev: {
  qubes-gui-common = prev.qubes-gui-common.overrideAttrs (old: {
    patches = (old.patches or [ ]) ++ [
      ../patches/qubes-gui-common/0001-protocol-add-maximize-window-flags.patch
      ../patches/qubes-gui-common/0002-protocol-add-interactive-window-move-message.patch
    ];
  });

  qubes-gui-agent-linux = prev.qubes-gui-agent-linux.overrideAttrs (old: {
    patches = (old.patches or [ ]) ++ [
      ../patches/qubes-gui-agent-linux/0001-gui-agent-forward-minimize-and-maximize-requests.patch
      ../patches/qubes-gui-agent-linux/0002-gui-agent-forward-interactive-window-move-requests.patch
    ];
  });

  qubes-gui-daemon = prev.qubes-gui-daemon.overrideAttrs (old: {
    patches = (old.patches or [ ]) ++ [
      ../patches/qubes-gui-daemon/0001-gui-daemon-apply-maximize-window-flags.patch
      ../patches/qubes-gui-daemon/0002-gui-daemon-apply-interactive-window-move-requests.patch
    ];
  });
}
