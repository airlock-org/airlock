# Omission preserves option defaults; explicit null is a distinct definition.
{
  schema = { lib, ... }: {
    options.services.example = {
      port = lib.mkOption { type = lib.types.int; default = 5432; };
      label = lib.mkOption { type = lib.types.nullOr lib.types.str; default = "fallback"; };
    };
  };
}
