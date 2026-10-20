# Complete-module compatibility harness: upstream reference versus Rust declarations and implementation.
{ nixpkgs, generated, caseName, rewritten ? true, downstream ? {}, checkAssertions ? false, normalizeRecovery ? true }:
let
  lib = import (nixpkgs + "/lib");
  # The Rust example puts the recovery symlink command on one line. Normalize
  # only this shell-equivalent formatting difference before unit derivation;
  # all resulting script bytes, store references and contexts still compare.
  upstream = args@{ config, lib, pkgs, ... }: let
    path = nixpkgs + "/nixos/modules/services/databases/postgresql.nix";
    original = import path args;
    script = original.config.content.systemd.services.postgresql.preStart;
    continuation = "\" \\\n  \"${config.services.postgresql.dataDir}/recovery.conf\"";
    singleLine = "\" \"${config.services.postgresql.dataDir}/recovery.conf\"";
  in original // {
    _file = toString path;
    config = lib.recursiveUpdate original.config {
      content.systemd.services.postgresql.preStart =
        builtins.replaceStrings [ continuation ] [ singleLine ] script;
    };
  };
  cases = {
    disabled = {};
    minimal = {};
    custom_package = { pkgs, ... }: { services.postgresql.package = pkgs.postgresql_15; };
    jit_enabled = { services.postgresql.enableJIT = true; };
    jit_disabled = { services.postgresql.enableJIT = false; };
    empty_extensions = { services.postgresql.extraPlugins = []; };
    extensions = { services.postgresql.extraPlugins = ps: [ ps.pg_repack ]; };
    extension_list = { pkgs, ... }: { services.postgresql.extraPlugins = [ pkgs.postgresql_16.pkgs.pg_repack ]; };
    custom_data_dir = { services.postgresql.dataDir = "/srv/postgres-data"; };
    legacy_data_dir = { services.postgresql.dataDir = "/var/lib/postgresql"; };
    basic_settings = { services.postgresql.settings = { max_connections = 120; log_connections = true; }; };
    mixed_settings = { services.postgresql.settings = {
      log_connections = false; max_connections = 42; random_page_cost = 1.25;
      log_line_prefix = "quote' backslash\\ and \"double quote\"";
      shared_preload_libraries = null;
      "arbitrary.setting-name" = "opaque";
    }; };
    preload_libraries = { services.postgresql.settings.shared_preload_libraries = [ "auto_explain" "pg_stat_statements" ]; };
    authentication_addition = { services.postgresql.authentication = "local app app peer"; };
    authentication_replacement = { lib, ... }: { services.postgresql.authentication = lib.mkForce "local all all trust"; };
    ident_map = { services.postgresql.identMap = "appmap unix_app db_app\nsecond other db_other"; };
    custom_port = { services.postgresql.settings.port = 6432; };
    initdb_arguments = { services.postgresql.initdbArgs = [ "--data-checksums" "--locale=C" "argument with spaces and 'quote'" ]; };
    initial_script = { pkgs, ... }: { services.postgresql.initialScript = pkgs.writeText "initial.sql" "CREATE TABLE initial_table (id integer);\n"; };
    recovery = { services.postgresql.recoveryConfig = "standby_mode = on\n"; };
    databases = { services.postgresql.ensureDatabases = [ "alpha" "beta" ]; };
    users = { services.postgresql.ensureUsers = [ { name = "alice"; } ]; };
    ownership = { services.postgresql = { ensureDatabases = [ "alice" ]; ensureUsers = [ { name = "alice"; ensureDBOwnership = true; } ]; }; };
    clauses_preserve = { services.postgresql.ensureUsers = [ { name = "alice"; ensureClauses = { superuser = null; createrole = null; createdb = null; "inherit" = null; login = null; replication = null; bypassrls = null; }; } ]; };
    clauses_enable = { services.postgresql.ensureUsers = [ { name = "alice"; ensureClauses = { superuser = true; createrole = true; createdb = true; "inherit" = true; login = true; replication = true; bypassrls = true; }; } ]; };
    clauses_disable = { services.postgresql.ensureUsers = [ { name = "alice"; ensureClauses = { superuser = false; createrole = false; createdb = false; "inherit" = false; login = false; replication = false; bypassrls = false; }; } ]; };
    multiple_roles = { services.postgresql = {
      ensureDatabases = [ "zebra" "app" "audit" ];
      ensureUsers = [
        { name = "app"; ensureDBOwnership = true; ensureClauses = { login = true; superuser = false; createrole = null; }; }
        { name = "audit"; ensureDBOwnership = true; ensureClauses = { "inherit" = false; replication = true; bypassrls = false; createdb = true; }; }
        { name = "readonly"; }
      ];
    }; };
    invalid_ownership = { services.postgresql.ensureUsers = [ { name = "orphan"; ensureDBOwnership = true; } ]; };
    tcpip = { services.postgresql.enableTCPIP = true; };
    port_priorities = { lib, ... }: { imports = [
      { services.postgresql.settings.port = lib.mkDefault 1111; }
      { services.postgresql.settings.port = 6432; }
      { services.postgresql.settings.port = lib.mkForce 7432; }
    ]; };
    jit_setting_override = { lib, ... }: { services.postgresql = { enableJIT = true; settings.jit = lib.mkForce "off"; }; };
    hardening_override = { lib, ... }: { systemd.services.postgresql.serviceConfig.MemoryDenyWriteExecute = lib.mkForce false; };
    check_disabled = { services.postgresql.checkConfig = false; };
    state_21 = { lib, ... }: { system.stateVersion = lib.mkForce "21.11"; };
    state_22 = { lib, ... }: { system.stateVersion = lib.mkForce "22.05"; };
    state_23 = { lib, ... }: { system.stateVersion = lib.mkForce "23.11"; };
    removed_11 = { lib, ... }: { system.stateVersion = lib.mkForce "20.03"; };
    removed_96 = { lib, ... }: { system.stateVersion = lib.mkForce "17.09"; };
    removed_95 = { lib, ... }: { system.stateVersion = lib.mkForce "16.09"; };
    invalid_port = { services.postgresql.settings.port = "not-an-integer"; };
    invalid_setting = { services.postgresql.settings.max_connections = [ 42 ]; };
    invalid_clause = { services.postgresql.ensureUsers = [ { name = "alice"; ensureClauses.login = "yes"; } ]; };
    removed_option = { services.postgresql.extraConfig = "invalid"; };
    renamed_options = { services.postgresql = { port = 6432; logLinePrefix = "%m [%p] "; }; };
    disabled_lazy = { lib, ... }: { services.postgresql = { enable = lib.mkForce false; settings.port = throw "disabled option was forced"; }; };
    cross_compiled = { nixpkgs.crossSystem = { config = "aarch64-unknown-linux-gnu"; }; };
    legacy_package_metadata = { pkgs, ... }: let
      # Exercise old-version decisions using a real derivation with fixture metadata.
      legacy = pkgs.postgresql_13 // {
        version = "9.5"; psqlSchema = "9.5";
        withJIT = legacy; withoutJIT = legacy; withPackages = _: legacy;
      };
    in { services.postgresql.package = legacy; };
    invalid_null = { services.postgresql.settings.non_nullable = null; };
    downstream_base = {};
    rust_model = { services.postgresql = {
      settings.max_connections = 100;
      ensureDatabases = [ "app" ];
      ensureUsers = [ { name = "app"; ensureDBOwnership = true; ensureClauses = { login = true; superuser = false; }; } ];
    }; };
  };
  evaluated = import (nixpkgs + "/nixos/lib/eval-config.nix") {
    system = "x86_64-linux";
    # Replace exactly the original import slot, retaining list-definition order.
    baseModules = map (module:
      if module == nixpkgs + "/nixos/modules/services/databases/postgresql.nix"
      then if rewritten then generated
        else if normalizeRecovery then upstream else module
      else module
    ) (import (nixpkgs + "/nixos/modules/module-list.nix"));
    modules = [
      ({ ... }: {
        boot.isContainer = true;
        fileSystems."/" = { device = "none"; fsType = "tmpfs"; };
        system.stateVersion = "24.11";
      })
      { services.postgresql.enable = caseName != "disabled"; }
      # rust_model inputs are authored in Rust on the rewritten side.
      (if rewritten && caseName == "rust_model" then {} else cases.${caseName})
      downstream
    ];
  };
  cfg = evaluated.config;
  pg = cfg.services.postgresql;
  # Preserve dependency context, not just JSON's context-erased string bytes.
  normalize = value:
    if lib.isDerivation value then {
      derivation = normalize value.drvPath;
      output = normalize value.outPath;
      recipe = builtins.readFile (builtins.unsafeDiscardStringContext value.drvPath);
    }
    else if builtins.isString value then { text = value; context = builtins.getContext value; }
    else if builtins.isList value then map normalize value
    else if builtins.isAttrs value then lib.mapAttrs (_: normalize) value
    else value;
  # A generated file's string carries its .drv dependencies. Read their recipes
  # rather than building the output or comparing incidental generated Nix text.
  file = value: {
    reference = normalize value;
    recipes = map
      (drv: builtins.readFile (builtins.unsafeDiscardStringContext drv))
      (builtins.attrNames (builtins.getContext value));
  };
  service = cfg.systemd.services.postgresql;
  configurationPath = builtins.head (builtins.elemAt
    (builtins.split ''"(/nix/store/[a-z0-9]+-postgresql[.]conf)/postgresql[.]conf"'' service.preStart) 1);
  ownAssertions = builtins.filter
    (a: lib.hasPrefix "For each database user defined with `services.postgresql.ensureUsers`" a.message)
    (lib.concatMap (definition: definition.value) (builtins.filter
    (definition: lib.hasSuffix "/services/databases/postgresql.nix" definition.file
      || builtins.match "rn-[0-9a-f]{16}" definition.file != null)
    evaluated.options.assertions.definitionsWithLocations));
  failedAssertions = builtins.filter (a: !a.assertion) cfg.assertions;
  checks = builtins.filter (drv: drv.name == "postgresql-configfile-check") cfg.system.checks;
in
if checkAssertions && failedAssertions != [] then
  builtins.addErrorContext "rusix-stage:nixos-assertions"
    (throw (lib.concatStringsSep "\n" (map (a: a.message) failedAssertions)))
else if !pg.enable then {
  enabled = false;
  servicePresent = builtins.hasAttr "postgresql" cfg.systemd.services;
  userPresent = builtins.hasAttr "postgres" cfg.users.users;
  checks = map normalize checks;
} else {
  enabled = true;
  package = normalize pg.package;
  packageMetadata = { inherit (pg.package) version psqlSchema; };
  settings = normalize pg.settings;
  authentication = normalize pg.authentication;
  identMap = normalize pg.identMap;
  generatedFiles = {
    configuration = file (builtins.appendContext configurationPath (builtins.getContext service.preStart));
    authentication = file pg.settings.hba_file;
    ident = file pg.settings.ident_file;
  };
  checks = map normalize checks;
  assertions = normalize ownAssertions;
  user = normalize cfg.users.users.postgres;
  group = normalize cfg.users.groups.postgres;
  # Inclusion paths are a set; module import depth changes their list order.
  # Preserve ordering for packages, SQL, arguments and all executable strings.
  pathsToLink = normalize (lib.sort builtins.lessThan cfg.environment.pathsToLink);
  installedPackages = map normalize (builtins.filter (package: lib.hasPrefix "postgresql" package.name) cfg.environment.systemPackages);
  service = normalize (lib.getAttrs [
    "description" "wantedBy" "after" "environment" "path"
    "preStart" "postStart" "serviceConfig" "unitConfig"
  ] service);
  unit = normalize cfg.systemd.units."postgresql.service".text;
}
