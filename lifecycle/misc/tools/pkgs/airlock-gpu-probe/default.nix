{
  writeShellApplication,
  python3,
  mesa-demos,
  vulkan-tools,
}:
writeShellApplication {
  name = "airlock-gpu-probe";
  runtimeInputs = [
    mesa-demos
    vulkan-tools
  ];
  text = ''
    exec ${python3}/bin/python3 ${./src/gpu-probe.py} "$@"
  '';
}
