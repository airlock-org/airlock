# Third-party source and attribution

The root MIT license covers original Airlock contributions. It does not
relicense vendored source, copied upstream code, derived patches, fetched
packages, firmware, artwork, or built OS images. Preserve each upstream's
copyright, license, and redistribution requirements.

## Sources

| Source | Relationship |
| --- | --- |
| [CertainLach/nixos-qubes](https://github.com/CertainLach/nixos-qubes) | Vendored Qubes/NixOS packaging and module base, with local compatibility fixes. |
| [NixOS/nixpkgs](https://github.com/NixOS/nixpkgs) | Locked NixOS/Xen modules and packages; individual package licenses vary. |
| [Qubes OS](https://github.com/QubesOS) | Existing control, RPC, policy, GUI, storage, and VM lifecycle stack. |
| [Xen](https://xenproject.org/) and [libvirt](https://libvirt.org/) | Upstream virtualization stack and packaging patches. |
| [GNOME Shell](https://gitlab.gnome.org/GNOME/gnome-shell), [pyxdg](https://gitlab.freedesktop.org/xdg/pyxdg), and [EROFS utilities](https://git.kernel.org/pub/scm/linux/kernel/git/xiang/erofs-utils.git/) | Targeted derived patches for integration and storage fixes. |

Exact fetched revisions are recorded in the flake locks and package definitions.
Patch directories group changes by upstream project; patch headers retain
provenance. A package's Nix license metadata is not a license grant for all
source in this repository.

## Website typeface

The website bundles the unmodified Inter variable font by Rasmus Andersson
under the SIL Open Font License 1.1. Its license is retained at
[`website/public/fonts/LICENSE.txt`](website/public/fonts/LICENSE.txt).
The font and license come from
[rsms/inter revision 353b61b](https://github.com/rsms/inter/tree/353b61b9f4430d5f420d56605a6e7993e0941470);
the source font is `docs/font-files/InterVariable.woff2`. The bundled font's
SHA-256 is `693b77d4f32ee9b8bfc995589b5fad5e99adf2832738661f5402f9978429a8e3`.
The website serves it locally, with no runtime font-service dependency.

## Outstanding vendored-source terms

The inspected `nixos-qubes` snapshot has no LICENSE/COPYING file or explicit
source-license grant in its README. Upstream's license endpoint returned HTTP 404
on 2026-10-11; no license grant was established. Do not assume its repository source is covered by either
Airlock's MIT license or the licenses of the packages it builds. Clarify the
vendored modules/packaging terms with upstream before public redistribution;
retain provenance and do not add an invented license to that directory.

This licensing question is a remaining publication prerequisite, not resolved
by adding MIT for original Airlock code. See the local readiness report in the
parent repository. No public release or artifact upload was performed.

The vendored source and upstream patch directories live in the pinned
[`airlock-machine`](airlock-machine/THIRD_PARTY.md) component.
