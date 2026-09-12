final: prev:
prev.lib.optionalAttrs (prev ? qubes-manager) {
  qubes-manager = prev.qubes-manager.overrideAttrs (oldAttrs: {
    passthru = (oldAttrs.passthru or { }) // {
      airlockTemplateToolsRemoved = true;
    };
    postInstall = (oldAttrs.postInstall or "") + ''
      # Airlock templates are built from Nix configuration. Do not expose the
      # upstream template installer until that repository/RPC path is supported.
      qubes_manager_py="$out/${prev.python3.sitePackages}/qubesmanager"

      rm -f \
        "$out/bin/qvm-template-gui" \
        "$out/bin/qubes-template-manager" \
        "$out/share/applications/qubes-template-manager.desktop" \
        "$out/share/applications/qubes-template-switcher.desktop"

      rm -f \
        "$qubes_manager_py/qvm_template_gui.py" \
        "$qubes_manager_py/template_manager.py" \
        "$qubes_manager_py/ui_qvmtemplate.py" \
        "$qubes_manager_py/ui_templateinstallconfirmdlg.py" \
        "$qubes_manager_py/ui_templateinstallprogressdlg.py" \
        "$qubes_manager_py/ui_templatemanager.py" \
        "$qubes_manager_py/__pycache__/"*template*.pyc \
        "$qubes_manager_py/__pycache__/qvm_template_gui."*.pyc

      for dist_info in "$out/${prev.python3.sitePackages}"/qubesmanager-*.dist-info; do
        if [ -d "$dist_info" ]; then
          if [ -f "$dist_info/entry_points.txt" ]; then
            sed -i \
              -e '/^qubes-template-manager = /d' \
              -e '/^qvm-template-gui = /d' \
              "$dist_info/entry_points.txt"
          fi
          if [ -f "$dist_info/RECORD" ]; then
            sed -i \
              -e '/bin\/qubes-template-manager/d' \
              -e '/bin\/qvm-template-gui/d' \
              -e '/qubesmanager\/qvm_template_gui[.]/d' \
              -e '/qubesmanager\/template_manager[.]/d' \
              -e '/qubesmanager\/ui_qvmtemplate[.]/d' \
              -e '/qubesmanager\/ui_templateinstall/d' \
              -e '/qubesmanager\/ui_templatemanager[.]/d' \
              "$dist_info/RECORD"
          fi
        fi
      done

      if [ -f "$out/etc/xdg/menus/applications-merged/qubes-tools.menu" ]; then
        sed -i '/qubes-template-manager[.]desktop/d' \
          "$out/etc/xdg/menus/applications-merged/qubes-tools.menu"
      fi

      if [ -f "$qubes_manager_py/ui_qubemanager.py" ]; then
        substituteInPlace "$qubes_manager_py/ui_qubemanager.py" \
          --replace-fail "self.menu_system.addAction(self.action_manage_templates)" \
            "self.action_manage_templates.setVisible(False)"
      fi

      if [ -f "$qubes_manager_py/qube_manager.py" ]; then
        substituteInPlace "$qubes_manager_py/qube_manager.py" \
          --replace-fail "        spawn_in_background('qubes-template-manager')" \
            "        return" \
          --replace-fail "supported\\nInstall new template with Template Manager" \
            "supported\\nInstall supported templates through Airlock config"
      fi

      if [ -f "$qubes_manager_py/tests/test_qube_manager.py" ]; then
        substituteInPlace "$qubes_manager_py/tests/test_qube_manager.py" \
          --replace-fail "assert qubes_manager.action_manage_templates.isEnabled()" \
            "assert not qubes_manager.action_manage_templates.isVisible()" \
          --replace-fail "mock_subprocess.assert_called_once_with(['qubes-template-manager'])" \
            "mock_subprocess.assert_not_called()"
      fi
    '';
  });
}
