# Repository boundary and workflow

`airlock-machine` owns deployable whole-machine Nix configuration, declared dom0
and guest systems, runtime services and packages, bootstrap inputs, its flake
lock, and component tests. A standalone checkout builds
`.#airlock-machine-release-usb-lab` or
`.#airlock-machine-release-usb-gui-lab` without the parent repository and without
evaluating a USB image or prepared guest-store payload.

The parent `airlock` repository owns USB disk-image assembly, prepared-store seeding, flashing,
provisioning, development-host tooling, lab orchestration, and integration
tests. Its USB configuration calls `airlock-machine.lib.mkMachineConfiguration` and
only substitutes prepared descriptor sources for image assembly. Deployable
releases canonicalize those sources back to the same OS-owned guest closure
artifacts.

The current `.gitmodules` URL is the local-only path
`/home/user/dev/airlock/airlock-machine`; no remote has been selected or published.
An existing local checkout initializes with:

```bash
git -c protocol.file.allow=always submodule update --init airlock-machine
```

A fresh clone on another machine must first obtain `airlock-machine` at a chosen
local path, then override the temporary local URL before initialization:

```bash
git config submodule.airlock-machine.url /absolute/path/to/airlock-machine
git -c protocol.file.allow=always submodule update --init airlock-machine
```

Once hosting is agreed, replace the URL in `.gitmodules`, run `git submodule
sync -- airlock-machine`, and commit that metadata change. Do not invent a remote meanwhile.

Make machine changes and commits in the standalone checkout first. Then update the
parent pointer and commit its integration changes:

```bash
git -C /home/user/dev/airlock/airlock-machine switch main
# edit, test, and commit airlock-machine
git -C airlock-machine fetch /home/user/dev/airlock/airlock-machine main
git -C airlock-machine checkout <airlock-machine-commit>
git add airlock-machine
git commit
```

Both repositories retain `lifecycle/misc/tools/scripts/airlock-nix.sh`. Use it
for every Nix evaluation and build. It selects the isolated store rooted at
`/run/media/user/data/airlock/nix-root`; routine work must stop when that
external storage is unavailable. Use only the guarded `make gc-auto`,
`make gc-dry-run`, and `make gc` targets.
