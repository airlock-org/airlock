{
  appliedMessage ? "initial",
  lib,
  ...
}:
{
  environment.etc."airlock/inside-config-applied-message".text = "${appliedMessage}\n";
  system.nixos.tags = [ "airlock-inside-config" ];

  # Keep this host definition tied to the POC VM target for now. A physical
  # runtime profile should replace the imported VM modules, not mutate them.
  networking.hostName = lib.mkDefault "dom0";
}
