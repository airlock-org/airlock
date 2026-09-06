let
  schema = "nixome.qubes-topology.v1";

  domainClasses = [
    "AppVM"
    "StandaloneVM"
    "TemplateVM"
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
        "skipStart"
        "skip_start"
      ] value;
      importTemplate = expectBool "templates.${checkedName}.import" (get template "import" false);
      source = expectNullOrStringOrPath "templates.${checkedName}.source" (get template "source" null);
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
    if importTemplate && source == null then
      fail "templates.${checkedName}.source" "must be set when import is true"
    else
      {
        import = importTemplate;
        inherit source;
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
        "class"
        "template"
        "label"
        "netvm"
        "prefs"
        "features"
        "services"
      ] value;
      class = expectEnum "domains.${checkedName}.class" domainClasses (get domain "class" "AppVM");
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
    in
    if class == "AppVM" && template == null then
      fail "domains.${checkedName}.template" "must be set for AppVM domains"
    else if class == "TemplateVM" && template != null then
      fail "domains.${checkedName}.template" "must be null for TemplateVM domains"
    else
      {
        inherit
          class
          template
          label
          netvm
          prefs
          features
          services
          ;
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
    labels
    mkTopology
    qrexecActions
    reservedQrexecServices
    schema
    ;
}
