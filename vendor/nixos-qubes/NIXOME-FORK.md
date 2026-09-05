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
- `qubes-vmm-xen`: make the nixpkgs Xen `xsd_glue` OCaml split-output copy
  detect the actual installed directory instead of deriving one path from
  `ls $out/lib/ocaml/`, and skip the newer plugin-interface source copy when
  the older Qubes Xen source layout does not include those files.
