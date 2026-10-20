# Public-interface tests: the candidate supplies its own declarations and implementation.
{ nixpkgs, generated, rewritten, caseName ? "disabled", mode ? "metadata" }:
let
  lib = import (nixpkgs + "/lib");
  normalize = value:
    if lib.isDerivation value then {
      name = normalize value.name;
      drvPath = normalize value.drvPath;
      outPath = normalize value.outPath;
    }
    else if builtins.isFunction value then { function = true; result = normalize (value {}); }
    else if builtins.isString value then { text = value; context = builtins.getContext value; }
    else if builtins.isPath value then normalize (toString value)
    else if builtins.isList value then map normalize value
    else if builtins.isAttrs value then lib.mapAttrs (_: normalize) value
    else value;
  cases = {
    disabled = { module = {}; select = pg: pg.enable; };
    enabled = { module.services.postgresql.enable = true; select = pg: pg.dataDir; };
    state_21 = { module = { services.postgresql.enable = true; system.stateVersion = lib.mkForce "21.11"; }; select = pg: pg.package.version; };
    state_22 = { module = { services.postgresql.enable = true; system.stateVersion = lib.mkForce "22.05"; }; select = pg: pg.package.version; };
    state_23 = { module = { services.postgresql.enable = true; system.stateVersion = lib.mkForce "23.11"; }; select = pg: pg.package.version; };
    jit_enabled = { module.services.postgresql = { enable = true; enableJIT = true; }; select = pg: pg.enableJIT; };
    jit_disabled = { module.services.postgresql = { enable = true; enableJIT = false; }; select = pg: pg.enableJIT; };
    bool_false = { module.services.postgresql.checkConfig = false; select = pg: pg.checkConfig; };
    bool_invalid = { module.services.postgresql.checkConfig = "yes"; select = pg: pg.checkConfig; };
    package_valid = { module = { pkgs, ... }: { services.postgresql.package = pkgs.postgresql_15; }; select = pg: pg.package; };
    package_invalid = { module.services.postgresql.package = { outPath = "/not-a-derivation"; }; select = pg: pg.package; };
    port_valid = { module.services.postgresql.settings.port = 6432; select = pg: pg.settings.port; };
    port_invalid = { module.services.postgresql.settings.port = 70000; select = pg: pg.settings.port; };
    port_null = { module.services.postgresql.settings.port = null; select = pg: pg.settings.port; };
    path_valid = { module.services.postgresql.dataDir = "/srv/postgres"; select = pg: pg.dataDir; };
    path_invalid = { module.services.postgresql.dataDir = 42; select = pg: pg.dataDir; };
    nullable_null = { module.services.postgresql.initialScript = null; select = pg: pg.initialScript; };
    nullable_file = { module = { pkgs, ... }: { services.postgresql.initialScript = pkgs.writeText "initial.sql" "SELECT 1;"; }; select = pg: pg.initialScript; };
    nullable_invalid = { module.services.postgresql.initialScript = false; select = pg: pg.initialScript; };
    recovery_lines = { module.imports = [ { services.postgresql.recoveryConfig = "first"; } { services.postgresql.recoveryConfig = "second"; } ]; select = pg: pg.recoveryConfig; };
    database_list_merge = { module.imports = [ { services.postgresql.ensureDatabases = [ "alpha" ]; } { services.postgresql.ensureDatabases = [ "beta" ]; } ]; select = pg: pg.ensureDatabases; };
    initdb_list_merge = { module.imports = [ { services.postgresql.initdbArgs = [ "--data-checksums" ]; } { services.postgresql.initdbArgs = [ "--locale=C" ]; } ]; select = pg: pg.initdbArgs; };
    database_list_invalid = { module.services.postgresql.ensureDatabases = [ 42 ]; select = pg: pg.ensureDatabases; };
    settings_merge = { module.imports = [ { services.postgresql.settings.custom_int = 42; } { services.postgresql.settings.custom_flag = true; } ]; select = pg: pg.settings; };
    settings_mixed = { module.services.postgresql.settings = { "literal.setting-name" = "value"; custom_int = 42; custom_float = 1.25; custom_flag = false; }; select = pg: pg.settings; };
    settings_null_invalid = { module.services.postgresql.settings.custom = null; select = pg: pg.settings; };
    settings_nested_invalid = { module.services.postgresql.settings.custom = { x = 1; }; select = pg: pg.settings; };
    preload_list = { module.services.postgresql.settings.shared_preload_libraries = [ "auto_explain" "anon" ]; select = pg: pg.settings.shared_preload_libraries; };
    preload_null = { module.services.postgresql.settings.shared_preload_libraries = null; select = pg: pg.settings.shared_preload_libraries; };
    preload_invalid = { module.services.postgresql.settings.shared_preload_libraries = [ 1 ]; select = pg: pg.settings.shared_preload_libraries; };
    users_valid = { module.services.postgresql.ensureUsers = [ { name = "app"; ensureClauses = { login = true; superuser = null; }; } ]; select = pg: pg.ensureUsers; };
    users_list_merge = { module.imports = [ { services.postgresql.ensureUsers = [ { name = "alice"; } ]; } { services.postgresql.ensureUsers = [ { name = "bob"; } ]; } ]; select = pg: pg.ensureUsers; };
    users_missing_name = { module.services.postgresql.ensureUsers = [ { ensureDBOwnership = false; } ]; select = pg: pg.ensureUsers; };
    users_unknown_field = { module.services.postgresql.ensureUsers = [ { name = "app"; unknown = true; } ]; select = pg: pg.ensureUsers; };
    users_invalid_clause = { module.services.postgresql.ensureUsers = [ { name = "app"; ensureClauses.login = "yes"; } ]; select = pg: pg.ensureUsers; };
    users_invalid_ownership = { module.services.postgresql.ensureUsers = [ { name = "app"; ensureDBOwnership = "yes"; } ]; select = pg: pg.ensureUsers; };
    users_invalid_shape = { module.services.postgresql.ensureUsers = "app"; select = pg: pg.ensureUsers; };
    plugins_function = { module.services.postgresql.extraPlugins = _: []; select = pg: pg.extraPlugins {}; };
    plugins_list = { module = { pkgs, ... }: { services.postgresql.extraPlugins = [ pkgs.postgresql_16.pkgs.pg_repack ]; }; select = pg: pg.extraPlugins {}; };
    plugins_invalid_list = { module.services.postgresql.extraPlugins = [ 1 ]; select = pg: pg.extraPlugins {}; };
    plugins_invalid_result = { module.services.postgresql.extraPlugins = _: "bad"; select = pg: pg.extraPlugins {}; };
    superuser_readonly = { module.services.postgresql.superUser = "changed"; select = pg: pg.superUser; };
    priorities = { module.imports = [ { services.postgresql.settings.port = lib.mkDefault 1111; } { services.postgresql.settings.port = 6432; } { services.postgresql.settings.port = lib.mkForce 7432; } ]; select = pg: pg.settings.port; };
    alias_port = { module.services.postgresql.port = 6432; select = pg: pg.settings.port; };
    alias_prefix = { module.services.postgresql.logLinePrefix = "%m "; select = pg: pg.settings.log_line_prefix; };
    removed = { module.services.postgresql.extraConfig = "invalid"; select = pg: pg.extraConfig; };
    ordinary_consumer = { module.services.postgresql = { enable = true; settings.port = 6432; ensureDatabases = [ "app" ]; ensureUsers = [ { name = "app"; ensureDBOwnership = true; ensureClauses.login = true; } ]; }; select = pg: { inherit (pg) dataDir ensureDatabases ensureUsers; port = pg.settings.port; }; };
  };
  evaluation = import (nixpkgs + "/nixos/lib/eval-config.nix") {
    system = "x86_64-linux";
    baseModules = map (module:
      if rewritten && module == nixpkgs + "/nixos/modules/services/databases/postgresql.nix"
      then generated else module
    ) (import (nixpkgs + "/nixos/modules/module-list.nix"));
    modules = [ {
      boot.isContainer = true;
      fileSystems."/" = { device = "none"; fsType = "tmpfs"; };
      system.stateVersion = "24.11";
    } cases.${caseName}.module ];
  };
  project = options: lib.mapAttrs (_: option:
    if lib.isOption option then {
      type = { inherit (option.type) name description; };
      metadata = normalize (lib.getAttrs (builtins.filter (name: builtins.hasAttr name option)
        [ "description" "defaultText" "example" "internal" "readOnly" "visible" ]) option);
      hasDefault = option ? default;
      default = if option ? default then normalize option.default else null;
      nested = project (lib.filterAttrs (name: _: name != "_module") (option.type.getSubOptions option.loc));
    } else project option
  ) options;
in if mode == "metadata" then project evaluation.options.services.postgresql
else if mode == "defaults" then let pg = evaluation.config.services.postgresql; in normalize {
  inherit (pg) enable enableJIT enableTCPIP checkConfig package authentication identMap initdbArgs initialScript ensureDatabases ensureUsers recoveryConfig superUser settings;
  dataDir = if pg.enable then pg.dataDir else null;
}
else normalize (cases.${caseName}.select evaluation.config.services.postgresql)
