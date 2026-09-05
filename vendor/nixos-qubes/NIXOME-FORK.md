# Nixome Local Fork

This directory is a local source fork of `CertainLach/nixos-qubes` for Nixome.

Seed upstream:

```text
repo: https://github.com/CertainLach/nixos-qubes
branch: trunk
commit: d50ecf2185ed5232c63845d7fe8e16395e5096d7
lastModifiedDate: 20260419042231
```

Local changes should stay narrow and upstreamable. Nixome should continue to
use Qubes and `nixos-qubes` architecture directly; this fork exists only for
compatibility fixes needed to evaluate, build, boot, and test the POC.

Current local changes:

- `qubes-linux-utils`: disable Python metadata checking for the imgconverter
  subpackage because the derivation name is `qubes-linux-utils-imgconverter`
  while the installed Python distribution is `qubesimgconverter`.
- `qubes-core-qubesdb`: disable Python metadata checking for the split daemon
  Python module because the importable extension is `qubesdb`, but no matching
  distribution metadata is installed for the derivation name.
- `qubes-core-libvirt`: disable inherited libvirt package checks because the
  Qubes patchset invalidates driver/capability fixture tests in the generic Nix
  build environment.
- `qubes-core-libvirt`: build libvirt 10.5 from the old nixpkgs package
  expression using Nixome's current nixpkgs dependency set. Importing the old
  package output directly mixed old glibc 2.39 libvirt objects with current
  Qubes Xen/glibc 2.42 dependencies at runtime.
- `qubes-core-admin`: patch `qubesd`, `qubes-create`, and `qubesd-query` to
  create and register asyncio event loops explicitly. Python 3.14 no longer
  gives the main thread an implicit current event loop for
  `asyncio.get_event_loop()`.
- `qubes-vmm-xen`: make the nixpkgs Xen `xsd_glue` OCaml split-output copy
  detect the actual installed directory instead of deriving one path from
  `ls $out/lib/ocaml/`, and skip the newer plugin-interface source copy when
  the older Qubes Xen source layout does not include those files.
