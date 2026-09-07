{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.nixome.qubes;
  access = cfg.configAccessPolicy;
  topologyLib = import ../lib/nixome/topology.nix;
  sourcePath = lib.types.oneOf [
    lib.types.path
    lib.types.str
  ];
  sourceRefType = lib.types.submodule {
    options = {
      build = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Candidate flake output attribute that builds this template source.";
      };

      attr = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Alias for build.";
      };

      subpath = lib.mkOption {
        type = lib.types.str;
        default = "";
        description = "Optional path inside the built output that contains root.img.";
      };
    };
  };
  sourceValueType = lib.types.oneOf [
    sourcePath
    sourceRefType
  ];
  jsonScalar = lib.types.nullOr (
    lib.types.oneOf [
      lib.types.bool
      lib.types.int
      lib.types.str
    ]
  );

  templateType = lib.types.submodule {
    options = {
      import = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Import this template source if the TemplateVM is missing.";
      };

      source = lib.mkOption {
        type = lib.types.nullOr (
          lib.types.oneOf [
            sourcePath
            sourceRefType
          ]
        );
        default = null;
        description = "Concrete template source directory or candidate flake build reference.";
      };

      sourceRef = lib.mkOption {
        type = lib.types.nullOr sourceRefType;
        default = null;
        description = "Candidate flake output attribute that builds this template source.";
      };

      skipStart = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Pass --skip-start while importing the template.";
      };
    };
  };

  kernelType = lib.types.submodule {
    options = {
      source = lib.mkOption {
        type = lib.types.nullOr sourcePath;
        default = null;
        description = "Source directory containing Qubes VM kernel files.";
      };
    };
  };

  backendSystemType = lib.types.submodule {
    options = {
      source = lib.mkOption {
        type = lib.types.nullOr sourceValueType;
        default = null;
        description = "Concrete Nix system closure source or candidate flake build reference.";
      };

      sourceRef = lib.mkOption {
        type = lib.types.nullOr sourceRefType;
        default = null;
        description = "Candidate flake output attribute that builds this Nix system closure.";
      };
    };
  };

  backendBootComponentType = lib.types.submodule {
    options = {
      name = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Boot component identity, such as a Qubes VM kernel name.";
      };

      source = lib.mkOption {
        type = lib.types.nullOr sourceValueType;
        default = null;
        description = "Concrete boot component source or candidate flake build reference.";
      };

      sourceRef = lib.mkOption {
        type = lib.types.nullOr sourceRefType;
        default = null;
        description = "Candidate flake output attribute that builds this boot component.";
      };
    };
  };

  backendStoreType = lib.types.submodule {
    options = {
      mode = lib.mkOption {
        type = lib.types.enum [ "closure-image" ];
        default = "closure-image";
        description = "How the domain receives its read-only Nix store view.";
      };

      source = lib.mkOption {
        type = lib.types.nullOr sourceValueType;
        default = null;
        description = "Concrete closure image source or candidate flake build reference.";
      };

      sourceRef = lib.mkOption {
        type = lib.types.nullOr sourceRefType;
        default = null;
        description = "Candidate flake output attribute that builds this closure image.";
      };

      mountpoint = lib.mkOption {
        type = lib.types.str;
        default = "/nix/store";
        description = "Guest mountpoint for the closure view.";
      };

      device = lib.mkOption {
        type = lib.types.str;
        default = "/dev/xvde";
        description = "Guest block device expected to expose the closure view.";
      };

      readOnly = lib.mkOption {
        type = lib.types.bool;
        default = true;
        description = "Whether the closure view is read-only.";
      };
    };
  };

  backendRootType = lib.types.submodule {
    options = {
      mode = lib.mkOption {
        type = lib.types.enum [ "qubes-root-image" ];
        default = "qubes-root-image";
        description = "How the current Qubes-compatible materializer receives its root filesystem image.";
      };

      source = lib.mkOption {
        type = lib.types.nullOr sourceValueType;
        default = null;
        description = "Concrete Qubes root image source or candidate flake build reference.";
      };

      sourceRef = lib.mkOption {
        type = lib.types.nullOr sourceRefType;
        default = null;
        description = "Candidate flake output attribute that builds this root image.";
      };

      readOnly = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Whether the imported Qubes root volume is read-only. Current materialization requires false.";
      };
    };
  };

  backendStateType = lib.types.submodule {
    options = {
      persistent = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Persistent private state volume identity.";
      };

      volatile = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "Volatile state volume identity.";
      };
    };
  };

  domainBackendType = lib.types.submodule {
    options = {
      type = lib.mkOption {
        type = lib.types.nullOr (lib.types.enum topologyLib.domainBackendTypes);
        default = null;
        description = "Domain construction mechanism.";
      };

      template = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "TemplateVM used by qubes-template backends.";
      };

      system = lib.mkOption {
        type = lib.types.nullOr backendSystemType;
        default = null;
        description = "Nix system closure for nix-closure domains.";
      };

      kernel = lib.mkOption {
        type = lib.types.nullOr backendBootComponentType;
        default = null;
        description = "Kernel binding for nix-closure domains.";
      };

      initrd = lib.mkOption {
        type = lib.types.nullOr backendBootComponentType;
        default = null;
        description = "Initrd binding for nix-closure domains.";
      };

      modules = lib.mkOption {
        type = lib.types.nullOr backendBootComponentType;
        default = null;
        description = "Kernel module binding for nix-closure domains.";
      };

      root = lib.mkOption {
        type = lib.types.nullOr backendRootType;
        default = null;
        description = "Current Qubes StandaloneVM root image materialization for nix-closure domains.";
      };

      store = lib.mkOption {
        type = lib.types.nullOr backendStoreType;
        default = null;
        description = "Read-only Nix store view for nix-closure domains.";
      };

      state = lib.mkOption {
        type = lib.types.nullOr backendStateType;
        default = null;
        description = "Persistent and volatile state bindings for nix-closure domains.";
      };
    };
  };

  domainType = lib.types.submodule {
    options = {
      backend = lib.mkOption {
        type = lib.types.nullOr domainBackendType;
        default = null;
        description = "Explicit domain construction backend.";
      };

      class = lib.mkOption {
        type = lib.types.nullOr (lib.types.enum topologyLib.domainClasses);
        default = null;
        description = "Qubes domain class to create when the domain is missing.";
      };

      template = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "TemplateVM used for AppVM creation.";
      };

      label = lib.mkOption {
        type = lib.types.enum topologyLib.labels;
        default = "red";
        description = "Qubes label.";
      };

      netvm = lib.mkOption {
        type = lib.types.nullOr lib.types.str;
        default = null;
        description = "NetVM to assign. Null means no network.";
      };

      prefs = lib.mkOption {
        type = lib.types.attrsOf jsonScalar;
        default = { };
        description = "Additional qvm-prefs values to set.";
      };

      features = lib.mkOption {
        type = lib.types.attrsOf jsonScalar;
        default = { };
        description = "qvm-features values to set.";
      };

      services = lib.mkOption {
        type = lib.types.attrsOf lib.types.bool;
        default = { };
        description = "qvm-service values to set.";
      };
    };
  };

  qrexecPolicyRuleType = lib.types.submodule {
    options = {
      service = lib.mkOption {
        type = lib.types.str;
        description = "Nixome-owned qrexec service name.";
      };

      argument = lib.mkOption {
        type = lib.types.str;
        default = "*";
        description = "qrexec service argument.";
      };

      source = lib.mkOption {
        type = lib.types.str;
        description = "qrexec source endpoint.";
      };

      target = lib.mkOption {
        type = lib.types.str;
        description = "qrexec target endpoint.";
      };

      action = lib.mkOption {
        type = lib.types.enum topologyLib.qrexecActions;
        description = "qrexec policy action.";
      };
    };
  };

  qrexecPolicyType = lib.types.submodule {
    options.rules = lib.mkOption {
      type = lib.types.listOf qrexecPolicyRuleType;
      default = [ ];
      description = "Nixome-owned qrexec policy rules.";
    };
  };

  topology = {
    inherit (cfg)
      templates
      kernels
      domains
      prefs
      qrexecPolicy
      ;
  };

  setupConfig = pkgs.callPackage ../pkgs/nixome-setup-config { };
  configApply = pkgs.callPackage ../pkgs/nixome-config-apply { };
  configArchive = pkgs.callPackage ../pkgs/nixome-config-archive { };
  configBootVerify = pkgs.callPackage ../pkgs/nixome-config-boot-verify { };
  configStatus = pkgs.callPackage ../pkgs/nixome-config-status { };
  configVmsSmoke = pkgs.callPackage ../pkgs/nixome-config-vms-smoke { };
  nixClosureDomainSmoke = pkgs.callPackage ../pkgs/nixome-nix-closure-domain-smoke { };
  qrexecPolicyStaticDir = "/etc/qubes/policy.d";
  qrexecPolicyRuntimeDir = "/var/lib/nixome/qubes-policy.d";
  qrexecManagedPolicyFile = "90-nixome.policy";
  qrexecBase = pkgs.python3.pkgs.qubes-core-qrexec;

  configAccessPolicyPackage = pkgs.runCommand "nixome-config-access-policy" { } ''
        set -eu

        install -d "$out/etc/qubes/policy.d"
        cat > "$out/etc/qubes/policy.d/30-nixome-config.policy" <<'EOF'
    # Managed by Nixome. Only dom0 may fetch a candidate from config-staging,
    # ask config-validator to validate it, or archive it into config-vault.
    ${access.candidateExportService} * dom0 ${access.staging} allow
    ${access.candidateExportService} * @anyvm @anyvm deny
    ${access.candidateValidateService} * dom0 ${access.validator} allow
    ${access.candidateValidateService} * @anyvm @anyvm deny
    ${access.archiveService} * dom0 ${access.vault} allow
    ${access.archiveService} * @anyvm @anyvm deny
    ${access.archiveDeploymentService} * dom0 ${access.vault} allow
    ${access.archiveDeploymentService} * @anyvm @anyvm deny
    EOF
  '';

  qrexecPolicyPathScript = pkgs.writeShellScript "nixome-qubes-policy-path" ''
    set -euo pipefail

    static_dir="''${NIXOME_QUBES_STATIC_POLICY_DIR:-${lib.escapeShellArg qrexecPolicyStaticDir}}"
    runtime_dir="''${NIXOME_QUBES_POLICY_DIR:-${lib.escapeShellArg qrexecPolicyRuntimeDir}}"
    managed_file="''${NIXOME_QUBES_MANAGED_POLICY_FILE:-${lib.escapeShellArg qrexecManagedPolicyFile}}"
    managed_path="$runtime_dir/$managed_file"

    ${pkgs.coreutils}/bin/install -d -m 0755 "$runtime_dir"

    ${pkgs.findutils}/bin/find "$runtime_dir" -mindepth 1 -maxdepth 1 ! -name "$managed_file" -print0 |
      while IFS= read -r -d "" path; do
        ${pkgs.coreutils}/bin/rm -rf -- "$path"
      done

    if [ -d "$static_dir" ]; then
      ${pkgs.findutils}/bin/find -L "$static_dir" -mindepth 1 -maxdepth 1 -print0 |
        while IFS= read -r -d "" path; do
          name="$(${pkgs.coreutils}/bin/basename "$path")"
          [ "$name" != "$managed_file" ] || continue
          target="$(${pkgs.coreutils}/bin/readlink -f "$path")"
          ${pkgs.coreutils}/bin/ln -sfnT "$target" "$runtime_dir/$name"
        done
    fi

    if [ -e "$managed_path" ]; then
      ${pkgs.gnugrep}/bin/grep -Fx "# Managed by Nixome setup-config." "$managed_path" >/dev/null || {
        printf 'refusing unmanaged Nixome qrexec policy file: %s\n' "$managed_path" >&2
        exit 1
      }
    else
      ${pkgs.coreutils}/bin/cat > "$managed_path" <<'EOF'
    # Managed by Nixome setup-config.
    # Source: nixome.qubes.topology.qrexec_policy.
    # Bootstrap config transport policy remains in 30-nixome-config.policy.
    EOF
      ${pkgs.coreutils}/bin/chmod 0644 "$managed_path"
    fi
  '';
in
{
  options.nixome.qubes = {
    enable = lib.mkEnableOption "declarative Qubes config topology setup";

    templates = lib.mkOption {
      type = lib.types.attrsOf templateType;
      default = { };
      description = "TemplateVMs that Nixome can import before setting up domains.";
    };

    kernels = lib.mkOption {
      type = lib.types.attrsOf kernelType;
      default = { };
      description = "Qubes VM kernels Nixome should install into dom0's VM kernel pool.";
    };

    domains = lib.mkOption {
      type = lib.types.attrsOf domainType;
      default = { };
      description = "Qubes domains Nixome should create and configure.";
    };

    prefs = lib.mkOption {
      type = lib.types.attrsOf jsonScalar;
      default = { };
      description = "Global qubes-prefs values for the setup-config layer.";
    };

    qrexecPolicy = lib.mkOption {
      type = qrexecPolicyType;
      default = { };
      description = "Nixome-owned qrexec policy rendered by setup-config.";
    };

    topology = lib.mkOption {
      type = lib.types.attrs;
      readOnly = true;
      description = "Normalized Nixome Qubes topology JSON payload.";
    };

    setupConfigOnBoot = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Run setup-config once during dom0 boot to ensure the declared Qubes topology exists.";
      };

      logDir = lib.mkOption {
        type = lib.types.str;
        default = "/var/log/nixome";
        description = "Directory where the boot setup-config unit writes JSON results and command logs.";
      };
    };

    configAccessPolicy = {
      enable = lib.mkOption {
        type = lib.types.bool;
        default = false;
        description = "Install qrexec policy for dom0-mediated config candidate archival.";
      };

      candidateExportService = lib.mkOption {
        type = lib.types.str;
        default = "nixome.Config.CandidateExport";
        description = "qrexec service used by dom0 to fetch a candidate from config-staging.";
      };

      candidateValidateService = lib.mkOption {
        type = lib.types.str;
        default = "nixome.Config.ValidateCandidate";
        description = "qrexec service used by dom0 to validate a candidate in config-validator.";
      };

      archiveService = lib.mkOption {
        type = lib.types.str;
        default = "nixome.Config.ArchiveCandidate";
        description = "qrexec service used by dom0 to archive a candidate into config-vault.";
      };

      archiveDeploymentService = lib.mkOption {
        type = lib.types.str;
        default = "nixome.Config.ArchiveDeployment";
        description = "qrexec service used by dom0 to archive deployment records.";
      };

      vault = lib.mkOption {
        type = lib.types.str;
        default = "config-vault";
        description = "Qube that owns the archived configuration history.";
      };

      staging = lib.mkOption {
        type = lib.types.str;
        default = "config-staging";
        description = "Qube that owns the editable configuration candidate.";
      };

      validator = lib.mkOption {
        type = lib.types.str;
        default = "config-validator";
        description = "Qube that validates staged configuration candidates.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    assertions =
      lib.mapAttrsToList (name: domain: {
        assertion =
          let
            backendType = if domain.backend == null then null else domain.backend.type;
            backendTemplate = if domain.backend == null then null else domain.backend.template;
            resolvedClass =
              if domain.class != null then
                domain.class
              else if backendType == "nix-closure" || backendType == "qubes-standalone" then
                "StandaloneVM"
              else if backendType == "qubes-template-root" then
                "TemplateVM"
              else
                "AppVM";
          in
          resolvedClass != "AppVM" || domain.template != null || backendTemplate != null;
        message = "nixome.qubes.domains.${name}.template must be set for AppVM domains.";
      }) cfg.domains
      ++ lib.mapAttrsToList (name: template: {
        assertion = !template.import || template.source != null || template.sourceRef != null;
        message = "nixome.qubes.templates.${name}.source or sourceRef must be set when import is enabled.";
      }) cfg.templates
      ++ lib.mapAttrsToList (name: kernel: {
        assertion = kernel.source != null;
        message = "nixome.qubes.kernels.${name}.source must be set.";
      }) cfg.kernels;

    nixome.qubes.topology = topologyLib.mkTopology topology;

    environment.etc."nixome/qubes-topology.json".text =
      builtins.toJSON config.nixome.qubes.topology + "\n";
    environment.systemPackages = [
      configApply
      configArchive
      configBootVerify
      configStatus
      configVmsSmoke
      nixClosureDomainSmoke
      setupConfig
    ];

    virtualisation.qubes.packages = lib.optionals cfg.configAccessPolicy.enable [
      configAccessPolicyPackage
    ];

    systemd.services.nixome-qubes-policy-path = {
      description = "Prepare the mutable Nixome qrexec policy path";
      before = [ "qubes-qrexec-policy-daemon.service" ];
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = qrexecPolicyPathScript;
      };
    };

    systemd.services.qubes-qrexec-policy-daemon = {
      overrideStrategy = "asDropin";
      requires = [ "nixome-qubes-policy-path.service" ];
      after = [ "nixome-qubes-policy-path.service" ];
      serviceConfig.ExecStart = lib.mkForce [
        ""
        "${qrexecBase}/bin/qrexec-policy-daemon --policy-path ${qrexecPolicyRuntimeDir}"
      ];
    };

    systemd.services.nixome-setup-config = lib.mkIf cfg.setupConfigOnBoot.enable {
      description = "Set up the declared Nixome Qubes topology";
      wantedBy = [ "multi-user.target" ];
      wants = [
        "libvirtd.service"
        "qubes-db-dom0.service"
        "qubesd.service"
        "qubes-qrexec-policy-daemon.service"
      ];
      after = [
        "libvirtd.service"
        "qubes-db-dom0.service"
        "qubesd.service"
        "qubes-qrexec-policy-daemon.service"
      ];
      restartIfChanged = false;
      stopIfChanged = false;
      environment.NIXOME_LOG_DIR = cfg.setupConfigOnBoot.logDir;
      serviceConfig = {
        Type = "oneshot";
        RemainAfterExit = true;
        ExecStart = "${setupConfig}/bin/nixome-setup-config --apply";
      };
    };
  };
}
