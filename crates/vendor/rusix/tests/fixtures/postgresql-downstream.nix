# An ordinary NixOS contributor, independent of Rusix and Rust lowering.
{ lib, pkgs, ... }: {
  services.postgresql = {
    package = lib.mkForce pkgs.postgresql_15;
    dataDir = "/srv/downstream-postgresql";
    settings = { port = 6432; max_connections = 80; log_connections = true; };
    authentication = lib.mkForce "local downstream downstream peer";
    ensureDatabases = [ "downstream" ];
    ensureUsers = [ {
      name = "downstream";
      ensureDBOwnership = true;
      ensureClauses = { login = true; superuser = false; };
    } ];
  };
}
