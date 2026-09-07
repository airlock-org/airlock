{
  extraTemplateDefinitions ? { },
  prebuiltTemplateSources ? { },
  system ? "x86_64-linux",
  templateBuilders,
}:
let
  qubesVmKernelName = "6.12.64-1.fc41";
  attrNames = builtins.attrNames;
  concatLists = builtins.concatLists;
  getAttr = builtins.getAttr;
  hasAttr = builtins.hasAttr;
  head = builtins.head;
  listToAttrs = builtins.listToAttrs;
  map = builtins.map;
  tail = builtins.tail;
  pkgs = templateBuilders.pkgs;
  lib = pkgs.lib;

  defaultTemplateConf = {
    qrexec = "1";
    virt-mode = "pvh";
  };

  mergeDomain =
    base: override:
    base
    // override
    // {
      prefs = (base.prefs or { }) // (override.prefs or { });
      features = (base.features or { }) // (override.features or { });
      services = (base.services or { }) // (override.services or { });
    };

  firstPrebuiltSource =
    keys:
    if keys == [ ] then
      null
    else
      let
        key = head keys;
      in
      if hasAttr key prebuiltTemplateSources then
        getAttr key prebuiltTemplateSources
      else
        firstPrebuiltSource (tail keys);

  baseTemplateDefinitions = {
    nixome-nixos-work = {
      packageName = "nixome-nixos-work-template";
      prebuiltSourceKeys = [
        "nixome-nixos-work"
        "work"
      ];
      role = "work-template";
      sourceLabel = "staged-config";
      hostName = "nixome-work-template";
      etc."nixome/template-role".text = "work\n";
      packages = with pkgs; [
        git
        jq
        nano
      ];
      appVms.nixome-work = {
        features = {
          "nixome-role" = "work";
        };
      };
    };
  };

  templateDefinitions = baseTemplateDefinitions // extraTemplateDefinitions;

  mkTemplateDomain =
    name: definition: builtTemplate:
    mergeDomain
      {
        class = "TemplateVM";
        label = "black";
        netvm = null;
        prefs = {
          default_user = "user";
          kernel = definition.kernel or qubesVmKernelName;
          kernelopts = builtTemplate.kernelopts;
          qrexec_timeout = 120;
          virt_mode = "pvh";
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = definition.role or "template";
          "nixome-template-source" = definition.sourceLabel or "staged-config";
          os = "Linux";
          qrexec = "1";
        };
        services = { };
      }
      (definition.domain or { });

  mkAppVmDomain =
    templateName: definition: name: appVm:
    mergeDomain
      {
        template = templateName;
        label = "green";
        netvm = null;
        prefs = {
          autostart = false;
          include_in_backups = true;
          memory = 1024;
          maxmem = 2048;
          vcpus = 1;
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "work";
          "nixome-template-source" = definition.sourceLabel or "staged-config";
          os = "Linux";
          qrexec = "1";
        };
        services = {
          "network-manager" = false;
        };
      }
      appVm;

  mkEntry =
    name:
    let
      definition = getAttr name templateDefinitions;
      packageName = definition.packageName or "${name}-template";
      hasDeclaredHostName = hasAttr "hostName" definition || hasAttr "hostname" definition;
      hostName =
        if hasAttr "hostName" definition then
          definition.hostName
        else if hasAttr "hostname" definition then
          definition.hostname
        else
          name;
      contentModule =
        { ... }:
        {
          nixome.qubeGuest.role = lib.mkDefault (definition.guestRole or "base");
          environment.systemPackages = definition.packages or [ ];
          environment.etc = definition.etc or { };
        }
        // (
          if hasDeclaredHostName then
            {
              networking.hostName = hostName;
            }
          else
            { }
        )
        // (
          if hasAttr "systemd" definition then
            {
              systemd = definition.systemd;
            }
          else
            { }
        );
      modules = [ contentModule ] ++ (definition.modules or [ ]);
      builtTemplate = templateBuilders.mkNixosQubeTemplate {
        templateName = name;
        imageName = definition.imageName or name;
        inherit modules;
        additionalSpace = definition.additionalSpace or "1024M";
        templateConf = definition.templateConf or defaultTemplateConf;
        vmAppmenus = definition.vmAppmenus or [ ];
        templateAppmenus = definition.templateAppmenus or [ ];
        netvmAppmenus = definition.netvmAppmenus or [ ];
      };
      prebuiltSource = firstPrebuiltSource (definition.prebuiltSourceKeys or [ name ]);
      packageAttr = "packages.${system}.${packageName}";
      importTemplate = definition.import or true;
    in
    {
      inherit
        builtTemplate
        definition
        importTemplate
        name
        packageAttr
        packageName
        prebuiltSource
        ;
      topologyTemplate = {
        import = importTemplate;
        source =
          if !importTemplate then
            null
          else if prebuiltSource != null then
            prebuiltSource
          else
            {
              build = packageAttr;
              subpath = definition.subpath or name;
            };
        skipStart = definition.skipStart or true;
      };
      templateDomain = mkTemplateDomain name definition builtTemplate;
      appVmDomains = map (appName: {
        name = appName;
        value = mkAppVmDomain name definition appName (getAttr appName (definition.appVms or { }));
      }) (attrNames (definition.appVms or { }));
    };

  entries = map mkEntry (attrNames templateDefinitions);
in
{
  packages = listToAttrs (
    map (entry: {
      name = entry.packageName;
      value = entry.builtTemplate.package;
    }) entries
  );

  topology = {
    templates = listToAttrs (
      map (entry: {
        name = entry.name;
        value = entry.topologyTemplate;
      }) entries
    );

    domains =
      listToAttrs (
        map (entry: {
          name = entry.name;
          value = entry.templateDomain;
        }) entries
      )
      // listToAttrs (concatLists (map (entry: entry.appVmDomains) entries));
  };
}
