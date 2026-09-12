let
  schema = "airlock.system.v1";

  activationActions = [
    "none"
    "run"
  ];

  systemdActions = [
    "none"
    "restart"
    "reload"
  ];

  switchActions = [
    "switch"
    "boot"
    "test"
    "dry-activate"
  ];

  attrNames = builtins.attrNames;
  concatLists = builtins.concatLists;
  concatStringsSep = builtins.concatStringsSep;
  elem = builtins.elem;
  filter = builtins.filter;
  getAttr = builtins.getAttr;
  hasAttr = builtins.hasAttr;
  head = builtins.head;
  listToAttrs = builtins.listToAttrs;
  map = builtins.map;
  match = builtins.match;
  tail = builtins.tail;
  toJSON = builtins.toJSON;
  typeOf = builtins.typeOf;

  mapAttrs =
    f: attrs:
    listToAttrs (
      map (name: {
        inherit name;
        value = f name (getAttr name attrs);
      }) (attrNames attrs)
    );

  get =
    attrs: name: default:
    if hasAttr name attrs then getAttr name attrs else default;

  optional = condition: value: if condition then [ value ] else [ ];

  concatMap = f: values: concatLists (map f values);

  fail = path: message: throw "airlock.system${if path == "" then "" else ".${path}"}: ${message}";

  expectAttrs =
    path: value:
    if typeOf value == "set" then value else fail path "expected an attribute set, got ${typeOf value}";

  expectList =
    path: value:
    if typeOf value == "list" then value else fail path "expected a list, got ${typeOf value}";

  expectBool =
    path: value:
    if typeOf value == "bool" then value else fail path "expected a boolean, got ${typeOf value}";

  expectStringOrPath =
    path: value:
    if typeOf value == "string" then
      value
    else if typeOf value == "path" then
      builtins.toString value
    else
      fail path "expected a string or path, got ${typeOf value}";

  expectNullOrStringOrPath =
    path: value: if value == null then null else expectStringOrPath path value;

  checkKeys =
    path: allowed: attrs:
    let
      unknown = filter (name: !(elem name allowed)) (attrNames attrs);
    in
    if unknown == [ ] then attrs else fail path "unknown option(s): ${concatStringsSep ", " unknown}";

  expectOptions =
    path: allowed: value:
    checkKeys path allowed (expectAttrs path value);

  expectEnum =
    path: allowed: value:
    if typeOf value != "string" then
      fail path "expected one of ${concatStringsSep ", " allowed}, got ${typeOf value}"
    else if elem value allowed then
      value
    else
      fail path "expected one of ${concatStringsSep ", " allowed}, got ${toJSON value}";

  expectName =
    kind: path: name:
    if typeOf name != "string" then
      fail path "expected a ${kind} name string, got ${typeOf name}"
    else if match "[A-Za-z0-9_.:+@-]+" name != null then
      name
    else
      fail path "invalid ${kind} name ${toJSON name}; expected [A-Za-z0-9_.:+@-]+";

  expectRelativePath =
    path: value:
    let
      relative = expectStringOrPath path value;
    in
    if relative == "" then
      fail path "must not be empty"
    else if match "[A-Za-z0-9_.+-]+(/[A-Za-z0-9_.+-]+)*" relative == null then
      fail path "invalid relative path ${toJSON relative}; expected a relative path without .."
    else
      relative;

  expectFlakeOutputRef =
    path: value:
    let
      ref = expectOptions path [
        "type"
        "build"
        "attr"
        "subpath"
      ] value;
      declaredType = get ref "type" "flake-output";
      hasBuild = hasAttr "build" ref;
      hasAttrName = hasAttr "attr" ref;
      attr =
        if declaredType != "flake-output" then
          fail "${path}.type" "expected flake-output, got ${toJSON declaredType}"
        else if hasBuild && hasAttrName then
          fail path "build and attr cannot both be set"
        else if hasBuild then
          expectStringOrPath "${path}.build" ref.build
        else if hasAttrName then
          expectStringOrPath "${path}.attr" ref.attr
        else
          fail path "must set build or attr";
      subpath = expectStringOrPath "${path}.subpath" (get ref "subpath" "");
    in
    if match "[A-Za-z0-9_+-]+(\\.[A-Za-z0-9_+-]+)*" attr == null then
      fail path "invalid flake output reference ${toJSON attr}; expected dot-separated flake attribute path"
    else if subpath != "" && match "[A-Za-z0-9_.+-]+(/[A-Za-z0-9_.+-]+)*" subpath == null then
      fail "${path}.subpath" "invalid source subpath ${toJSON subpath}; expected a relative path without .."
    else
      {
        type = "flake-output";
        inherit attr subpath;
      };

  normalizeSource =
    path: value:
    if value == null then
      {
        source = null;
        source_ref = null;
      }
    else if typeOf value == "set" then
      {
        source = null;
        source_ref = expectFlakeOutputRef path value;
      }
    else
      {
        source = expectStringOrPath path value;
        source_ref = null;
      };

  sourceIsSet = sourceInfo: sourceInfo.source != null || sourceInfo.source_ref != null;

  normalizeSourceOption =
    path: attrs:
    let
      hasSource = hasAttr "source" attrs && attrs.source != null;
      hasCamelSourceRef = hasAttr "sourceRef" attrs && attrs.sourceRef != null;
      hasSnakeSourceRef = hasAttr "source_ref" attrs && attrs.source_ref != null;
      rawSource =
        if hasCamelSourceRef && hasSnakeSourceRef then
          fail path "sourceRef and source_ref cannot both be set"
        else if hasSource && (hasCamelSourceRef || hasSnakeSourceRef) then
          fail path "source and sourceRef cannot both be set"
        else if hasSource then
          attrs.source
        else if hasCamelSourceRef then
          attrs.sourceRef
        else if hasSnakeSourceRef then
          attrs.source_ref
        else
          null;
    in
    normalizeSource "${path}.source" rawSource;

  normalizeProfile =
    value:
    let
      profile = expectOptions "dom0.profile" [
        "source"
        "sourceRef"
        "source_ref"
        "activationCommand"
        "activation_command"
        "activationAction"
        "activation_action"
        "message"
      ] value;
      sourceInfo = normalizeSourceOption "dom0.profile" profile;
      hasCamelCommand = hasAttr "activationCommand" profile;
      hasSnakeCommand = hasAttr "activation_command" profile;
      activationCommand =
        if hasCamelCommand && hasSnakeCommand then
          let
            camel = expectRelativePath "dom0.profile.activationCommand" profile.activationCommand;
            snake = expectRelativePath "dom0.profile.activation_command" profile.activation_command;
          in
          if camel == snake then
            camel
          else
            fail "dom0.profile" "activationCommand and activation_command disagree"
        else if hasCamelCommand then
          expectRelativePath "dom0.profile.activationCommand" profile.activationCommand
        else if hasSnakeCommand then
          expectRelativePath "dom0.profile.activation_command" profile.activation_command
        else
          "bin/airlock-dom0-profile-activate";
      hasCamelAction = hasAttr "activationAction" profile;
      hasSnakeAction = hasAttr "activation_action" profile;
      camelAction =
        if hasCamelAction then
          expectEnum "dom0.profile.activationAction" activationActions profile.activationAction
        else
          null;
      snakeAction =
        if hasSnakeAction then
          expectEnum "dom0.profile.activation_action" activationActions profile.activation_action
        else
          null;
      activationAction =
        if hasCamelAction && hasSnakeAction then
          if camelAction == snakeAction then
            camelAction
          else
            fail "dom0.profile" "activationAction and activation_action disagree"
        else if hasCamelAction then
          camelAction
        else if hasSnakeAction then
          snakeAction
        else
          "run";
      message = expectStringOrPath "dom0.profile.message" (get profile "message" "");
    in
    {
      inherit message;
      inherit (sourceInfo) source source_ref;
      activation_command = activationCommand;
      activation_action = activationAction;
    };

  normalizeHostSystem =
    value:
    let
      path = "dom0.nixos.system";
      componentValue =
        if value == null then
          { }
        else
          expectOptions path [
            "enable"
            "reason"
            "source"
            "sourceRef"
            "source_ref"
            "switchAction"
            "switch_action"
          ] value;
      sourceInfo = normalizeSourceOption path componentValue;
      enable = expectBool "${path}.enable" (get componentValue "enable" (sourceIsSet sourceInfo));
      reason = expectStringOrPath "${path}.reason" (
        get componentValue "reason" "new dom0 NixOS system generation"
      );
      hasCamelAction = hasAttr "switchAction" componentValue;
      hasSnakeAction = hasAttr "switch_action" componentValue;
      camelAction =
        if hasCamelAction then
          expectEnum "${path}.switchAction" switchActions componentValue.switchAction
        else
          null;
      snakeAction =
        if hasSnakeAction then
          expectEnum "${path}.switch_action" switchActions componentValue.switch_action
        else
          null;
      switchAction =
        if hasCamelAction && hasSnakeAction then
          if camelAction == snakeAction then
            camelAction
          else
            fail path "switchAction and switch_action disagree"
        else if hasCamelAction then
          camelAction
        else if hasSnakeAction then
          snakeAction
        else
          "switch";
    in
    if enable && !(sourceIsSet sourceInfo) then
      fail path "enable is true but no source or sourceRef was provided"
    else
      {
        inherit enable reason;
        switch_action = switchAction;
        inherit (sourceInfo) source source_ref;
      };

  normalizeRebootComponent =
    component: value:
    let
      path = "dom0.nixos.${component}";
      componentValue =
        if value == null then
          { }
        else
          expectOptions path [
            "enable"
            "reason"
            "source"
            "sourceRef"
            "source_ref"
          ] value;
      sourceInfo = normalizeSourceOption path componentValue;
      enable = expectBool "${path}.enable" (get componentValue "enable" (sourceIsSet sourceInfo));
      reason = expectStringOrPath "${path}.reason" (
        get componentValue "reason" "dom0 ${component} change requires reboot"
      );
    in
    {
      inherit enable reason;
      inherit (sourceInfo) source source_ref;
    };

  normalizeSystemdService =
    name: value:
    let
      checkedName = expectName "systemd service" "dom0.systemd.services.${name}" name;
      service = expectOptions "dom0.systemd.services.${checkedName}" [
        "action"
        "reason"
      ] value;
    in
    {
      action = expectEnum "dom0.systemd.services.${checkedName}.action" systemdActions (
        get service "action" "none"
      );
      reason = expectStringOrPath "dom0.systemd.services.${checkedName}.reason" (
        get service "reason" "dom0 systemd service ${checkedName} changed"
      );
    };

  normalizeSystemd =
    value:
    let
      systemd = expectOptions "dom0.systemd" [ "services" ] value;
    in
    {
      services = mapAttrs normalizeSystemdService (
        expectAttrs "dom0.systemd.services" (get systemd "services" { })
      );
    };

  normalizePackage =
    index: value:
    let
      package = expectOptions "dom0.packages.${toString index}" [ "name" ] value;
    in
    {
      name = expectName "package" "dom0.packages.${toString index}.name" (get package "name" null);
    };

  imap =
    f: list:
    let
      go =
        index: values:
        if values == [ ] then [ ] else [ (f index (head values)) ] ++ go (index + 1) (tail values);
    in
    go 0 list;

  normalizeDom0 =
    value:
    let
      dom0 = expectOptions "dom0" [
        "profile"
        "packages"
        "systemd"
        "nixos"
      ] value;
      nixos = expectOptions "dom0.nixos" [
        "system"
        "kernel"
        "xen"
        "bootloader"
        "qubes"
      ] (get dom0 "nixos" { });
    in
    {
      profile = normalizeProfile (get dom0 "profile" { });
      packages = imap normalizePackage (expectList "dom0.packages" (get dom0 "packages" [ ]));
      systemd = normalizeSystemd (get dom0 "systemd" { });
      nixos = {
        system = normalizeHostSystem (get nixos "system" null);
        kernel = normalizeRebootComponent "kernel" (get nixos "kernel" null);
        xen = normalizeRebootComponent "xen" (get nixos "xen" null);
        bootloader = normalizeRebootComponent "bootloader" (get nixos "bootloader" null);
        qubes = normalizeRebootComponent "qubes" (get nixos "qubes" null);
      };
    };

  serviceLiveActions =
    services:
    concatMap (
      name:
      let
        service = getAttr name services;
      in
      optional (service.action != "none") {
        type = "systemd-service";
        inherit name;
        action = service.action;
        inherit (service) reason;
      }
    ) (attrNames services);

  hostSystemLiveAction =
    hostSystem:
    optional hostSystem.enable {
      type = "dom0-nixos-system";
      action = hostSystem.switch_action;
      source = hostSystem.source;
      source_ref = hostSystem.source_ref;
      reason = hostSystem.reason;
    };

  hostSystemRebootAction =
    hostSystem:
    optional (hostSystem.enable && hostSystem.switch_action == "boot") {
      type = "dom0-nixos-system";
      switch_action = hostSystem.switch_action;
      source = hostSystem.source;
      source_ref = hostSystem.source_ref;
      reason = hostSystem.reason;
    };

  rebootComponentActions =
    nixos:
    concatMap (
      name:
      let
        component = getAttr name nixos;
      in
      optional component.enable {
        type = "dom0-nixos-${name}";
        source = component.source;
        source_ref = component.source_ref;
        reason = component.reason;
      }
    ) (attrNames nixos);

  mkSystem =
    value:
    let
      cfg = expectOptions "" [
        "schema"
        "dom0"
      ] value;
      requestedSchema = get cfg "schema" schema;
      dom0 = normalizeDom0 (get cfg "dom0" { });
      profileEnabled = sourceIsSet dom0.profile;
      liveActions =
        optional profileEnabled {
          type = "dom0-profile";
          action = dom0.profile.activation_action;
          source = dom0.profile.source;
          source_ref = dom0.profile.source_ref;
          activation_command = dom0.profile.activation_command;
          message = dom0.profile.message;
        }
        ++ hostSystemLiveAction dom0.nixos.system
        ++ serviceLiveActions dom0.systemd.services;
      rebootActions =
        hostSystemRebootAction dom0.nixos.system
        ++ rebootComponentActions {
          inherit (dom0.nixos)
            bootloader
            kernel
            qubes
            xen
            ;
        };
      reasons = map (action: action.reason) rebootActions;
    in
    if requestedSchema != schema then
      fail "schema" "expected ${schema}, got ${toJSON requestedSchema}"
    else
      {
        inherit schema dom0;
        plan = {
          live_actions = liveActions;
          reboot_actions = rebootActions;
          reboot_required = rebootActions != [ ];
          reboot_reasons = reasons;
        };
      };
in
{
  inherit
    activationActions
    mkSystem
    schema
    switchActions
    systemdActions
    ;
}
