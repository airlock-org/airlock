let
  schema = "nixome.qubes-topology.v1";

  domainClasses = [
    "AppVM"
    "StandaloneVM"
    "TemplateVM"
  ];

  domainBackendTypes = [
    "qubes-template"
    "qubes-template-root"
    "qubes-standalone"
    "nix-closure"
  ];

  labels = [
    "red"
    "orange"
    "yellow"
    "green"
    "gray"
    "blue"
    "purple"
    "black"
  ];

  qrexecActions = [
    "allow"
    "deny"
  ];

  reservedQrexecServices = [
    "nixome.Config.CandidateExport"
    "nixome.Config.ValidateCandidate"
    "nixome.Config.ArchiveCandidate"
    "nixome.Config.ArchiveDeployment"
  ];

  attrNames = builtins.attrNames;
  concatStringsSep = builtins.concatStringsSep;
  elem = builtins.elem;
  filter = builtins.filter;
  foldl' = builtins.foldl';
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

  fail = path: message: throw "nixome.topology${if path == "" then "" else ".${path}"}: ${message}";

  expectAttrs =
    path: value:
    if typeOf value == "set" then value else fail path "expected an attribute set, got ${typeOf value}";

  expectBool =
    path: value:
    if typeOf value == "bool" then value else fail path "expected a boolean, got ${typeOf value}";

  expectList =
    path: value:
    if typeOf value == "list" then value else fail path "expected a list, got ${typeOf value}";

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

  expectFlakeOutputRef =
    path: value:
    let
      ref = expectOptions path [
        "build"
        "attr"
        "subpath"
      ] value;
      hasBuild = hasAttr "build" ref && ref.build != null;
      hasAttrName = hasAttr "attr" ref && ref.attr != null;
      attr =
        if hasBuild && hasAttrName then
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

  expectQubesName =
    kind: path: name:
    if typeOf name != "string" then
      fail path "expected a ${kind} name string, got ${typeOf name}"
    else if match "[A-Za-z][A-Za-z0-9_.-]*" name != null then
      name
    else
      fail path "invalid ${kind} name ${toJSON name}; expected [A-Za-z][A-Za-z0-9_.-]*";

  expectLooseName =
    kind: path: name:
    if typeOf name != "string" then
      fail path "expected a ${kind} name string, got ${typeOf name}"
    else if match "[A-Za-z0-9_.:+-]+" name != null then
      name
    else
      fail path "invalid ${kind} name ${toJSON name}; expected [A-Za-z0-9_.:+-]+";

  expectQrexecService =
    path: value:
    let
      service = expectStringOrPath path value;
    in
    if match "nixome\\.[A-Za-z0-9_.-]+" service == null then
      fail path "invalid qrexec service ${toJSON service}; expected nixome.[A-Za-z0-9_.-]+"
    else if elem service reservedQrexecServices then
      fail path "reserved bootstrap qrexec service ${toJSON service} cannot be managed by topology"
    else
      service;

  expectQrexecArgument =
    path: value:
    let
      argument = expectStringOrPath path value;
    in
    if argument == "*" || match "[A-Za-z0-9_.:+-]+" argument != null then
      argument
    else
      fail path "invalid qrexec argument ${toJSON argument}; expected * or [A-Za-z0-9_.:+-]+";

  expectQrexecEndpoint =
    path: value:
    let
      endpoint = expectStringOrPath path value;
    in
    if
      endpoint == "dom0" || endpoint == "@anyvm" || match "[A-Za-z][A-Za-z0-9_.-]*" endpoint != null
    then
      endpoint
    else
      fail path "invalid qrexec endpoint ${toJSON endpoint}; expected dom0, @anyvm, or a Qubes domain name";

  expectKernelName =
    path: name:
    if typeOf name != "string" then
      fail path "expected a kernel name string, got ${typeOf name}"
    else if name != "" && match "[^/]+" name != null then
      name
    else
      fail path "invalid kernel name ${toJSON name}; expected a non-empty name without /";

  normalizeJsonScalar =
    path: value:
    if value == null || typeOf value == "bool" || typeOf value == "int" || typeOf value == "string" then
      value
    else
      fail path "expected null, boolean, integer, or string, got ${typeOf value}";

  normalizeJsonAttrs =
    kind: path: value:
    mapAttrs (
      name: scalar:
      let
        checkedName = expectLooseName kind "${path}.${name}" name;
      in
      normalizeJsonScalar "${path}.${checkedName}" scalar
    ) (expectAttrs path value);

  normalizeServiceAttrs =
    path: value:
    mapAttrs (
      name: service:
      let
        checkedName = expectLooseName "service" "${path}.${name}" name;
      in
      expectBool "${path}.${checkedName}" service
    ) (expectAttrs path value);

  imap =
    f: list:
    let
      go =
        index: values:
        if values == [ ] then [ ] else [ (f index (head values)) ] ++ go (index + 1) (tail values);
    in
    go 0 list;

  any = pred: list: foldl' (acc: value: acc || pred value) false list;

  normalizeQrexecRule =
    index: value:
    let
      path = "qrexecPolicy.rules.${toString index}";
      rule = expectOptions path [
        "service"
        "argument"
        "source"
        "target"
        "action"
      ] value;
      service = expectQrexecService "${path}.service" (get rule "service" null);
      argument = expectQrexecArgument "${path}.argument" (get rule "argument" "*");
      source = expectQrexecEndpoint "${path}.source" (get rule "source" null);
      target = expectQrexecEndpoint "${path}.target" (get rule "target" null);
      action = expectEnum "${path}.action" qrexecActions (get rule "action" null);
    in
    if target == "dom0" then
      fail "${path}.target" "topology-managed qrexec policy cannot target dom0"
    else if action == "allow" && source == "@anyvm" then
      fail "${path}.source" "allow rules must use dom0 or a concrete source domain"
    else if action == "allow" && target == "@anyvm" then
      fail "${path}.target" "allow rules must use a concrete target domain"
    else
      {
        inherit
          service
          argument
          source
          target
          action
          ;
      };

  ruleIsCatchAllDeny =
    service: rule:
    rule.service == service
    && rule.argument == "*"
    && rule.source == "@anyvm"
    && rule.target == "@anyvm"
    && rule.action == "deny";

  servicesForRules =
    rules:
    attrNames (
      listToAttrs (
        map (rule: {
          name = rule.service;
          value = true;
        }) rules
      )
    );

  normalizeQrexecPolicy =
    path: value:
    let
      policy = expectOptions path [ "rules" ] value;
      explicitRules = imap normalizeQrexecRule (expectList "${path}.rules" (get policy "rules" [ ]));
      servicesMissingCatchAllDeny = filter (service: !(any (ruleIsCatchAllDeny service) explicitRules)) (
        servicesForRules explicitRules
      );
      denyRules = map (service: {
        inherit service;
        argument = "*";
        source = "@anyvm";
        target = "@anyvm";
        action = "deny";
      }) servicesMissingCatchAllDeny;
    in
    {
      rules = explicitRules ++ denyRules;
    };

  normalizeTemplate =
    name: value:
    let
      checkedName = expectQubesName "template" "templates.${name}" name;
      template = expectOptions "templates.${checkedName}" [
        "import"
        "source"
        "sourceRef"
        "source_ref"
        "skipStart"
        "skip_start"
      ] value;
      importTemplate = expectBool "templates.${checkedName}.import" (get template "import" false);
      hasSource = hasAttr "source" template && template.source != null;
      hasCamelSourceRef = hasAttr "sourceRef" template && template.sourceRef != null;
      hasSnakeSourceRef = hasAttr "source_ref" template && template.source_ref != null;
      rawSource =
        if hasCamelSourceRef && hasSnakeSourceRef then
          fail "templates.${checkedName}" "sourceRef and source_ref cannot both be set"
        else if hasSource && (hasCamelSourceRef || hasSnakeSourceRef) then
          fail "templates.${checkedName}" "source and sourceRef cannot both be set"
        else if hasSource then
          template.source
        else if hasCamelSourceRef then
          template.sourceRef
        else if hasSnakeSourceRef then
          template.source_ref
        else
          null;
      sourceInfo = normalizeSource "templates.${checkedName}.source" rawSource;
      hasCamelSkip = hasAttr "skipStart" template;
      hasSnakeSkip = hasAttr "skip_start" template;
      camelSkip =
        if hasCamelSkip then expectBool "templates.${checkedName}.skipStart" template.skipStart else null;
      snakeSkip =
        if hasSnakeSkip then expectBool "templates.${checkedName}.skip_start" template.skip_start else null;
      skipStart =
        if hasCamelSkip && hasSnakeSkip then
          if camelSkip == snakeSkip then
            camelSkip
          else
            fail "templates.${checkedName}" "skipStart and skip_start disagree"
        else if hasCamelSkip then
          camelSkip
        else if hasSnakeSkip then
          snakeSkip
        else
          true;
    in
    if importTemplate && sourceInfo.source == null && sourceInfo.source_ref == null then
      fail "templates.${checkedName}.source" "must be set when import is true"
    else
      {
        import = importTemplate;
        inherit (sourceInfo) source source_ref;
        skip_start = skipStart;
      };

  normalizeKernel =
    name: value:
    let
      checkedName = expectKernelName "kernels.${name}" name;
      kernel = expectOptions "kernels.${checkedName}" [ "source" ] value;
      source = expectNullOrStringOrPath "kernels.${checkedName}.source" (get kernel "source" null);
    in
    if source == null || source == "" then
      fail "kernels.${checkedName}.source" "must be set"
    else
      { inherit source; };

  normalizeClosureSystem =
    domainName: value:
    if value == null then
      fail "domains.${domainName}.backend.system.source" "must be set for nix-closure domains"
    else
      let
        path = "domains.${domainName}.backend.system";
        system = expectOptions path [
          "source"
          "sourceRef"
          "source_ref"
        ] value;
        sourceInfo = normalizeSourceOption path system;
      in
      if !(sourceIsSet sourceInfo) then
        fail "${path}.source" "must be set for nix-closure domains"
      else
        sourceInfo;

  normalizeClosureBootComponent =
    domainName: component: required: value:
    let
      path = "domains.${domainName}.backend.${component}";
      componentValue =
        if value == null then
          { }
        else
          expectOptions path [
            "name"
            "source"
            "sourceRef"
            "source_ref"
          ] value;
      sourceInfo = normalizeSourceOption path componentValue;
      componentName =
        if hasAttr "name" componentValue && componentValue.name != null then
          expectLooseName "${component} binding" "${path}.name" componentValue.name
        else
          null;
    in
    if required && componentName == null && !(sourceIsSet sourceInfo) then
      fail path "must set name, source, or sourceRef for nix-closure domains"
    else
      {
        name = componentName;
        inherit (sourceInfo) source source_ref;
      };

  normalizeClosureStore =
    domainName: value:
    if value == null then
      fail "domains.${domainName}.backend.store.source" "must be set for closure-image stores"
    else
      let
        path = "domains.${domainName}.backend.store";
        store = expectOptions path [
          "mode"
          "source"
          "sourceRef"
          "source_ref"
          "mountpoint"
          "readOnly"
          "read_only"
        ] value;
        mode = expectEnum "${path}.mode" [ "closure-image" ] (get store "mode" "closure-image");
        sourceInfo = normalizeSourceOption path store;
        mountpoint = expectStringOrPath "${path}.mountpoint" (get store "mountpoint" "/nix/store");
        hasCamelReadOnly = hasAttr "readOnly" store;
        hasSnakeReadOnly = hasAttr "read_only" store;
        camelReadOnly = if hasCamelReadOnly then expectBool "${path}.readOnly" store.readOnly else null;
        snakeReadOnly = if hasSnakeReadOnly then expectBool "${path}.read_only" store.read_only else null;
        readOnly =
          if hasCamelReadOnly && hasSnakeReadOnly then
            if camelReadOnly == snakeReadOnly then
              camelReadOnly
            else
              fail path "readOnly and read_only disagree"
          else if hasCamelReadOnly then
            camelReadOnly
          else if hasSnakeReadOnly then
            snakeReadOnly
          else
            true;
      in
      if mountpoint != "/nix/store" then
        fail "${path}.mountpoint" "currently must be /nix/store"
      else if readOnly != true then
        fail path "nix-closure store views must be read-only"
      else if !(sourceIsSet sourceInfo) then
        fail "${path}.source" "must be set for closure-image stores"
      else
        {
          inherit mode mountpoint;
          read_only = readOnly;
          inherit (sourceInfo) source source_ref;
        };

  normalizeClosureRoot =
    domainName: value:
    let
      path = "domains.${domainName}.backend.root";
      root =
        if value == null then
          { }
        else
          expectOptions path [
            "mode"
            "source"
            "sourceRef"
            "source_ref"
            "readOnly"
            "read_only"
          ] value;
      mode = expectEnum "${path}.mode" [ "qubes-root-image" ] (get root "mode" "qubes-root-image");
      sourceInfo = normalizeSourceOption path root;
      hasCamelReadOnly = hasAttr "readOnly" root;
      hasSnakeReadOnly = hasAttr "read_only" root;
      camelReadOnly = if hasCamelReadOnly then expectBool "${path}.readOnly" root.readOnly else null;
      snakeReadOnly = if hasSnakeReadOnly then expectBool "${path}.read_only" root.read_only else null;
      readOnly =
        if hasCamelReadOnly && hasSnakeReadOnly then
          if camelReadOnly == snakeReadOnly then
            camelReadOnly
          else
            fail path "readOnly and read_only disagree"
        else if hasCamelReadOnly then
          camelReadOnly
        else if hasSnakeReadOnly then
          snakeReadOnly
        else
          false;
    in
    if readOnly then
      fail path "qubes-root-image materialization imports a writable Qubes root volume"
    else if !(sourceIsSet sourceInfo) then
      fail "${path}.source" "must be set for qubes-root-image materialization"
    else
      {
        inherit mode;
        read_only = readOnly;
        inherit (sourceInfo) source source_ref;
      };

  normalizeClosureState =
    domainName: value:
    let
      path = "domains.${domainName}.backend.state";
      state =
        if value == null then
          { }
        else
          expectOptions path [
            "persistent"
            "volatile"
          ] value;
      persistent = expectLooseName "persistent state volume" "${path}.persistent" (
        get state "persistent" "${domainName}-private"
      );
      volatile =
        if hasAttr "volatile" state && state.volatile != null then
          expectLooseName "volatile state volume" "${path}.volatile" state.volatile
        else
          null;
    in
    {
      inherit persistent volatile;
    };

  normalizeDomainBackend =
    domainName: class: domainTemplate: value:
    if value == null then
      if class == "AppVM" then
        {
          type = "qubes-template";
          template = domainTemplate;
        }
      else if class == "TemplateVM" then
        {
          type = "qubes-template-root";
          template = null;
        }
      else
        {
          type = "qubes-standalone";
          template = null;
        }
    else
      let
        path = "domains.${domainName}.backend";
        backend = expectOptions path [
          "type"
          "template"
          "system"
          "kernel"
          "initrd"
          "modules"
          "root"
          "store"
          "state"
        ] value;
        backendType = expectEnum "${path}.type" domainBackendTypes (get backend "type" null);
        backendTemplate =
          if hasAttr "template" backend && backend.template != null then
            expectQubesName "template" "${path}.template" backend.template
          else
            domainTemplate;
      in
      if backendType == "qubes-template" then
        if class != "AppVM" then
          fail path "qubes-template backend requires class AppVM"
        else if backendTemplate == null then
          fail "${path}.template" "must be set for qubes-template domains"
        else if
          hasAttr "template" backend
          && backend.template != null
          && domainTemplate != null
          && backendTemplate != domainTemplate
        then
          fail path "template and backend.template disagree"
        else
          {
            type = backendType;
            template = backendTemplate;
          }
      else if backendType == "qubes-template-root" then
        if class != "TemplateVM" then
          fail path "qubes-template-root backend requires class TemplateVM"
        else if domainTemplate != null then
          fail path "template must be null for qubes-template-root domains"
        else
          {
            type = backendType;
            template = null;
          }
      else if backendType == "qubes-standalone" then
        if class != "StandaloneVM" then
          fail path "qubes-standalone backend requires class StandaloneVM"
        else if domainTemplate != null then
          fail path "template must be null for qubes-standalone domains"
        else
          {
            type = backendType;
            template = null;
          }
      else if backendType == "nix-closure" then
        if class != "StandaloneVM" then
          fail path "nix-closure backend requires class StandaloneVM"
        else if domainTemplate != null then
          fail path "template must not be set for nix-closure domains"
        else
          {
            type = backendType;
            template = null;
            system = normalizeClosureSystem domainName (get backend "system" null);
            kernel = normalizeClosureBootComponent domainName "kernel" true (get backend "kernel" null);
            initrd = normalizeClosureBootComponent domainName "initrd" false (get backend "initrd" null);
            modules = normalizeClosureBootComponent domainName "modules" false (get backend "modules" null);
            root = normalizeClosureRoot domainName (get backend "root" null);
            store = normalizeClosureStore domainName (get backend "store" null);
            state = normalizeClosureState domainName (get backend "state" null);
          }
      else
        fail "${path}.type" "unsupported backend type ${toJSON backendType}";

  defaultPrefsForClass =
    class:
    if class == "AppVM" || class == "StandaloneVM" then
      {
        autostart = false;
        include_in_backups = false;
      }
    else
      { };

  normalizeDomain =
    name: value:
    let
      checkedName = expectQubesName "domain" "domains.${name}" name;
      domain = expectOptions "domains.${checkedName}" [
        "backend"
        "class"
        "template"
        "label"
        "netvm"
        "prefs"
        "features"
        "services"
      ] value;
      rawBackend = get domain "backend" null;
      rawBackendType =
        if rawBackend != null && typeOf rawBackend == "set" && hasAttr "type" rawBackend then
          rawBackend.type
        else
          null;
      defaultClass =
        if rawBackendType == "nix-closure" || rawBackendType == "qubes-standalone" then
          "StandaloneVM"
        else if rawBackendType == "qubes-template-root" then
          "TemplateVM"
        else
          "AppVM";
      rawClass = get domain "class" null;
      class =
        if rawClass == null then
          defaultClass
        else
          expectEnum "domains.${checkedName}.class" domainClasses rawClass;
      template =
        let
          rawTemplate = get domain "template" null;
        in
        if rawTemplate == null then
          null
        else
          expectQubesName "template" "domains.${checkedName}.template" rawTemplate;
      label = expectEnum "domains.${checkedName}.label" labels (get domain "label" "red");
      netvm =
        let
          rawNetvm = get domain "netvm" null;
        in
        if rawNetvm == null then null else expectQubesName "domain" "domains.${checkedName}.netvm" rawNetvm;
      prefs =
        defaultPrefsForClass class
        // normalizeJsonAttrs "pref" "domains.${checkedName}.prefs" (get domain "prefs" { });
      features = normalizeJsonAttrs "feature" "domains.${checkedName}.features" (
        get domain "features" { }
      );
      services = normalizeServiceAttrs "domains.${checkedName}.services" (get domain "services" { });
      backend = normalizeDomainBackend checkedName class template rawBackend;
      resolvedTemplate = if backend.type == "qubes-template" then backend.template else template;
    in
    if class == "AppVM" && resolvedTemplate == null then
      fail "domains.${checkedName}.template" "must be set for AppVM domains"
    else if class == "TemplateVM" && resolvedTemplate != null then
      fail "domains.${checkedName}.template" "must be null for TemplateVM domains"
    else
      {
        inherit
          class
          label
          netvm
          prefs
          features
          services
          backend
          ;
        template = resolvedTemplate;
      };

  normalizeNamedAttrs =
    path: value: normalize:
    mapAttrs normalize (expectAttrs path value);

  mkTopology =
    value:
    let
      cfg = expectOptions "" [
        "schema"
        "templates"
        "kernels"
        "domains"
        "prefs"
        "qrexecPolicy"
        "qrexec_policy"
      ] value;
      requestedSchema = get cfg "schema" schema;
      hasCamelQrexecPolicy = hasAttr "qrexecPolicy" cfg;
      hasSnakeQrexecPolicy = hasAttr "qrexec_policy" cfg;
      qrexecPolicy =
        if hasCamelQrexecPolicy && hasSnakeQrexecPolicy then
          fail "" "qrexecPolicy and qrexec_policy cannot both be set"
        else if hasCamelQrexecPolicy then
          cfg.qrexecPolicy
        else if hasSnakeQrexecPolicy then
          cfg.qrexec_policy
        else
          { };
    in
    if requestedSchema != schema then
      fail "schema" "expected ${schema}, got ${toJSON requestedSchema}"
    else
      {
        inherit schema;
        templates = normalizeNamedAttrs "templates" (get cfg "templates" { }) normalizeTemplate;
        kernels = normalizeNamedAttrs "kernels" (get cfg "kernels" { }) normalizeKernel;
        domains = normalizeNamedAttrs "domains" (get cfg "domains" { }) normalizeDomain;
        prefs = normalizeJsonAttrs "pref" "prefs" (get cfg "prefs" { });
        qrexec_policy = normalizeQrexecPolicy "qrexecPolicy" qrexecPolicy;
      };
in
{
  inherit
    domainClasses
    domainBackendTypes
    labels
    mkTopology
    qrexecActions
    reservedQrexecServices
    schema
    ;
}
