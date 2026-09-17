# Repository boundary and workflow

`airlock-os` owns deployable whole-machine Nix configuration, declared dom0
and guest systems, runtime services and packages, bootstrap inputs, its flake
lock, and component tests. A standalone checkout builds
`.#airlock-machine-release-usb-lab` or
`.#airlock-machine-release-usb-gui-lab` without `airlock-dev` and without
evaluating a USB image or prepared guest-store payload.

`airlock-dev` owns USB disk-image assembly, prepared-store seeding, flashing,
provisioning, development-host tooling, lab orchestration, and integration
tests. Its USB configuration calls `airlock-os.lib.mkMachineConfiguration` and
only substitutes prepared descriptor sources for image assembly. Deployable
releases canonicalize those sources back to the same OS-owned guest closure
artifacts.

The current `.gitmodules` URL is the local-only path
`/home/user/dev/airlock/airlock-os`; no remote has been selected or published.
An existing local checkout initializes with:

```bash
git -c protocol.file.allow=always submodule update --init os
```

A fresh clone on another machine must first obtain `airlock-os` at a chosen
local path, then override the temporary local URL before initialization:

```bash
git config submodule.os.url /absolute/path/to/airlock-os
git -c protocol.file.allow=always submodule update --init os
```

Once hosting is agreed, replace the URL in `.gitmodules`, run `git submodule
sync -- os`, and commit that metadata change. Do not invent a remote meanwhile.

Make OS changes and commits in the standalone checkout first. Then update the
parent pointer and commit its integration changes:

```bash
git -C /home/user/dev/airlock/airlock-os switch main
# edit, test, and commit airlock-os
git -C os fetch /home/user/dev/airlock/airlock-os main
git -C os checkout <airlock-os-commit>
git add os
git commit
```

Both repositories retain `lifecycle/misc/tools/scripts/airlock-nix.sh`. Use it
for every Nix evaluation and build. It selects the isolated store rooted at
`/run/media/user/data/airlock/nix-root`; routine work must stop when that
external storage is unavailable. Use only the guarded `make gc-auto`,
`make gc-dry-run`, and `make gc` targets.
