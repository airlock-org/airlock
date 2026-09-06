{
  prebuiltTemplateSources ? { },
  system ? "x86_64-linux",
  templateBuilders,
}:
let
  qubesVmKernelName = "6.12.64-1.fc41";
  workTemplateName = "nixome-nixos-work";
  workTemplatePackageName = "nixome-nixos-work-template";
  workTemplatePackageAttr = "packages.${system}.${workTemplatePackageName}";
  workTemplatePrebuiltSource =
    prebuiltTemplateSources.${workTemplateName} or prebuiltTemplateSources.work or null;

  workTemplate = templateBuilders.mkNixosQubeTemplate {
    templateName = workTemplateName;
    imageName = workTemplateName;
    modules = [
      (
        { pkgs, ... }:
        {
          nixome.qubeGuest.role = "base";
          networking.hostName = "nixome-work-template";
          environment.etc."nixome/template-role".text = "work\n";
          environment.systemPackages = with pkgs; [
            git
            jq
            nano
          ];
        }
      )
    ];
  };
in
{
  packages.${workTemplatePackageName} = workTemplate.package;

  topology = {
    templates.${workTemplateName} = {
      import = true;
      source =
        if workTemplatePrebuiltSource != null then
          workTemplatePrebuiltSource
        else
          {
            build = workTemplatePackageAttr;
            subpath = workTemplateName;
          };
      skipStart = true;
    };

    domains = {
      ${workTemplateName} = {
        class = "TemplateVM";
        label = "black";
        netvm = null;
        prefs = {
          default_user = "user";
          kernel = qubesVmKernelName;
          kernelopts = workTemplate.kernelopts;
          qrexec_timeout = 120;
          virt_mode = "pvh";
        };
        features = {
          "nixome-managed" = "1";
          "nixome-role" = "work-template";
          "nixome-template-source" = "staged-config";
          os = "Linux";
          qrexec = "1";
        };
      };

      nixome-work = {
        template = workTemplateName;
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
          "nixome-template-source" = "staged-config";
          os = "Linux";
          qrexec = "1";
        };
        services = {
          "network-manager" = false;
        };
      };
    };
  };
}
