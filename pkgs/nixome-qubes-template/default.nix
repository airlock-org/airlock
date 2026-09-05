{
  lib,
  runCommand,
  templateName,
  rootImageFile,
  templateConf ? {
    qrexec = "1";
    virt-mode = "pvh";
  },
  vmAppmenus ? [ ],
  templateAppmenus ? [ ],
  netvmAppmenus ? [ ],
}:
let
  renderValue = value: if lib.isBool value then (if value then "1" else "0") else toString value;

  templateConfText = lib.concatStringsSep "\n" (
    lib.mapAttrsToList (name: value: "${name}=${renderValue value}") templateConf
  );

  renderList = values: lib.concatStringsSep "\n" values;
in
runCommand "qubes-template-${templateName}"
  {
    inherit templateName rootImageFile;
    passthru = {
      inherit templateName rootImageFile;
    };
    meta = {
      description = "Qubes template directory for ${templateName}";
      license = lib.licenses.mit;
      platforms = [ "x86_64-linux" ];
    };
  }
  ''
        set -eu

        template="$out/${templateName}"
        install -d "$template/apps" "$template/apps.tempicons" "$template/apps.templates"

        ln -s "${rootImageFile}" "$template/root.img"

        cat > "$template/template.conf" <<'EOF'
    ${templateConfText}
    EOF

        cat > "$template/vm-whitelisted-appmenus.list" <<'EOF'
    ${renderList vmAppmenus}
    EOF

        cat > "$template/whitelisted-appmenus.list" <<'EOF'
    ${renderList templateAppmenus}
    EOF

        cat > "$template/netvm-whitelisted-appmenus.list" <<'EOF'
    ${renderList netvmAppmenus}
    EOF

        find "$out" -type d -exec chmod 0555 {} +
        find "$out" -type f -exec chmod 0444 {} +
  ''
