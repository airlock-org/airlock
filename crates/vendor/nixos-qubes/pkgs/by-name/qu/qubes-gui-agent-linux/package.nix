{
  lib,
  stdenv,
  fetchFromGitHub,
  autoconf,
  automake,
  pkg-config,
  libtool,
  coreutils,
  libX11,
  libXcomposite,
  libXcursor,
  libXdamage,
  libXfixes,
  libXt,
  libunistring,
  libgbm,
  dbus,
  pam,
  qubes-core-qubesdb,
  qubes-core-agent-linux,
  qubes-core-vchan-xen,
  qubes-gui-common,
  qubes-gui-daemon,
  qubes-vmm-xen,
  xorgserver,
  xorgproto,
  util-macros,
  xinit,
  systemd,
  xdpyinfo,
}:
let
  version = "4.3.21";
  src = fetchFromGitHub {
    owner = "QubesOS";
    repo = "qubes-gui-agent-linux";
    rev = "refs/tags/v${version}";
    hash = "sha256-XQcoN/xod32bd0SlasiC5+SQt/WgbDQvrCmPmUKswJc=";
  };
in
stdenv.mkDerivation {
  inherit version src;
  pname = "qubes-gui-agent-linux";

  nativeBuildInputs = [
    autoconf
    automake
    pkg-config
    libtool
  ];

  buildInputs = [
    libX11
    libXcomposite
    libXcursor
    libXdamage
    libXfixes
    libXt
    libunistring
    libgbm
    dbus
    pam
    qubes-core-qubesdb
    qubes-core-vchan-xen
    qubes-gui-common
    qubes-vmm-xen.dev
    xorgserver.dev
    xorgproto
    util-macros
  ];

  preBuild = ''
    substituteInPlace gui-agent/vmside.c \
      --replace-fail /usr/bin/qubes-run-xorg "$out/bin/qubes-run-xorg"
    make -C xf86-qubes-common libxf86-qubes-common.so
    (cd xf86-input-mfndev && ./autogen.sh && ./configure --with-xorg-module-dir="$out/lib/xorg/modules")
    (cd xf86-video-dummy && ./autogen.sh && ./configure --with-xorg-module-dir="$out/lib/xorg/modules")
  '';

  buildFlags = [
    "gui-agent/qubes-gui"
    "gui-agent/qubes-gui-runuser"
    "xf86-input-mfndev/src/.libs/qubes_drv.so"
    "xf86-video-dummy/src/.libs/dummyqbs_drv.so"
  ];

  installTargets = [
    "install-common"
    "install-systemd"
  ];

  makeFlags = [
    "DESTDIR=$(out)"
    "LIBDIR=/lib"
    "USRLIBDIR=/lib"
    "SYSLIBDIR=/lib"
    "UNITDIR=/lib/systemd/system"
    "USERUNITDIR=/lib/systemd/user"
    "USERPRESETDIR=/lib/systemd/user-preset"
    "UDEVRULESDIR=/lib/udev/rules.d"
    "XINITRCDIR=/etc/X11/xinit/xinitrc.d"
  ];

  postInstall = ''
    mkdir -p "$out/bin" "$out/lib"
    mv "$out/usr/bin/"* "$out/bin/"
    mv "$out/usr/lib/qubes" "$out/lib/"
    mv "$out/usr/lib/sysctl.d" "$out/lib/"
    rm -rf "$out/usr"

    ln -sfn ../../bin/qubes-set-monitor-layout "$out/etc/qubes-rpc/qubes.SetMonitorLayout"
    ln -sfn ../../bin/qubes-start-xephyr "$out/etc/qubes-rpc/qubes.GuiVMSession"

    substituteInPlace "$out/etc/X11/xorg-qubes.conf.template" \
      --replace-fail 'Section "Module"' "Section \"Files\"
        ModulePath \"$out/lib/xorg/modules\"
        ModulePath \"${xorgserver}/lib/xorg/modules\"
    EndSection

    Section \"Module\""
    substituteInPlace "$out/bin/qubes-run-xorg" \
      --replace-fail '. /usr/lib/qubes/init/functions' '. ${qubes-core-agent-linux}/lib/qubes/init/functions' \
      --replace-fail /usr/lib/qubes "$out/lib/qubes" \
      --replace-fail /usr/bin/qubes-gui-runuser "$out/bin/qubes-gui-runuser" \
      --replace-fail /usr/bin/xinit "${xinit}/bin/xinit"
    substituteInPlace "$out/lib/qubes/qubes-xorg-wrapper" \
      --replace-fail 'XORG="/usr/bin/X"' 'XORG="${xorgserver}/bin/Xorg"'
    substituteInPlace "$out/lib/qubes/qubes-gui-agent-pre.sh" \
      --replace-fail '. /usr/lib/qubes/init/functions' '. ${qubes-core-agent-linux}/lib/qubes/init/functions'
    substituteInPlace "$out/bin/qubes-run-xephyr" \
      --replace-fail /usr/lib64/qubes-gui-daemon/shmoverride.so "${qubes-gui-daemon}/lib/qubes-gui-daemon/shmoverride.so" \
      --replace-fail /usr/bin/Xephyr "${xorgserver}/bin/Xephyr"
    substituteInPlace "$out/bin/qubes-start-xephyr" \
      --replace-fail 'exec systemd-run' 'exec ${systemd}/bin/systemd-run' \
      --replace-fail 'timeout 30s sh -c' '${coreutils}/bin/timeout 30s ${stdenv.shell} -c' \
      --replace-fail 'xdpyinfo -display' '${xdpyinfo}/bin/xdpyinfo -display' \
      --replace-fail 'sleep 1' '${coreutils}/bin/sleep 1' \
      --replace-fail 'qubesdb-read --default' '${qubes-core-qubesdb}/bin/qubesdb-read --default' \
      --replace-fail /usr/bin/qubes-gui-runuser "$out/bin/qubes-gui-runuser" \
      --replace-fail /usr/bin/env "${coreutils}/bin/env" \
      --replace-fail /usr/bin/xinit "${xinit}/bin/xinit" \
      --replace-fail /usr/bin/qubes-run-xephyr "$out/bin/qubes-run-xephyr"
    substituteInPlace "$out/bin/qubes-session" \
      --replace-fail '. /usr/lib/qubes/init/functions' '. ${qubes-core-agent-linux}/lib/qubes/init/functions' \
      --replace-fail 'XSESSION=/usr/bin/xfce4-session' 'echo "qubes-session: GUI VM requires an explicit XSESSION" >&2; exit 1' \
      --replace-fail /usr/bin/qubesdb-read "${qubes-core-qubesdb}/bin/qubesdb-read"
    substituteInPlace "$out/lib/systemd/system/qubes-gui-agent.service" \
      --replace-fail /usr/bin/qubes-gui "$out/bin/qubes-gui" \
      --replace-fail /usr/lib/qubes "$out/lib/qubes"
  '';

  meta = {
    description = "Qubes GUI agent and virtual Xorg drivers for guest domains";
    homepage = "https://github.com/QubesOS/qubes-gui-agent-linux";
    license = lib.licenses.gpl2Plus;
    platforms = lib.platforms.linux;
    maintainers = with lib.maintainers; [
      lach
      sigmasquadron
    ];
  };
}
