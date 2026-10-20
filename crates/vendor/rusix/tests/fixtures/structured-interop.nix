# Evaluation-only schema for the generated-file smoke case, not a PostgreSQL rewrite.
{
  getContext = builtins.getContext;
  schema = { lib, ... }: {
    options.services.postgresql.dataDir = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/postgresql";
    };
  };
}
