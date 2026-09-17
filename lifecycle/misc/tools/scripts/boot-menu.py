#!/usr/bin/env python3
"""Present Airlock roles and retained normal generations in systemd-boot."""

import argparse
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
import os
import re
import subprocess

GENERATION = re.compile(r"^Generation ([0-9]+)(?:\s|$)")
STORE_SYSTEM = re.compile(r"(?:^|\s)init=(/nix/store/[^/\s]+)/init(?:\s|$)")


@dataclass(frozen=True)
class Entry:
    path: Path
    generation: int
    role: str
    system: Path


def fields(path):
    result = {}
    for line in path.read_text().splitlines():
        key, separator, value = line.partition(" ")
        if separator:
            result[key] = value
    return result


def system_for_xen_entry(entry, store_root):
    ordinary = entry.with_name("nixos-" + entry.name.removeprefix("xen-"))
    if not ordinary.is_file():
        return None
    match = STORE_SYSTEM.search(fields(ordinary).get("options", ""))
    if not match:
        return None
    return store_root / Path(match.group(1)).name


def classified_entries(entries_dir, store_root):
    result = []
    for path in entries_dir.glob("xen-*.conf"):
        metadata = fields(path)
        generation = GENERATION.match(metadata.get("version", ""))
        system = system_for_xen_entry(path, store_root)
        marker = system / "etc/airlock/boot-role" if system else None
        if not generation or not marker or not marker.is_file():
            continue
        role = marker.read_text().strip()
        if role in ("normal", "recovery"):
            result.append(Entry(path, int(generation.group(1)), role, system))
    return result


def replace_field(path, key, value):
    lines = path.read_text().splitlines()
    replacement = f"{key} {value}"
    for index, line in enumerate(lines):
        if line.startswith(key + " "):
            lines[index] = replacement
            break
    else:
        lines.append(replacement)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text("\n".join(lines) + "\n")
    os.replace(temporary, path)


def choose_one_per_generation(entries, preferred):
    grouped = defaultdict(list)
    for entry in entries:
        grouped[entry.generation].append(entry)
    selected = []
    for generation, choices in grouped.items():
        choices.sort(
            key=lambda entry: (
                entry.path.stem != preferred,
                "usb-lab" not in fields(entry.path).get("title", ""),
                entry.path.name,
            )
        )
        selected.append(choices[0])
    return sorted(selected, key=lambda entry: entry.generation, reverse=True)


def configure(esp, store_root, recovery_profile=None, nix_env="nix-env"):
    entries_dir = esp / "loader/entries"
    loader_conf = esp / "loader/loader.conf"
    if not entries_dir.is_dir() or not loader_conf.is_file():
        raise ValueError("systemd-boot files are missing from the EFI partition")

    loader = fields(loader_conf)
    preferred = loader.get("preferred", loader.get("default", "")).removesuffix(".conf")
    classified = classified_entries(entries_dir, store_root)
    normal = choose_one_per_generation(
        [entry for entry in classified if entry.role == "normal"], preferred
    )
    recovery = choose_one_per_generation(
        [entry for entry in classified if entry.role == "recovery"], preferred
    )
    if not normal:
        raise ValueError("no Xen Airlock normal boot entry was generated")
    if not recovery:
        raise ValueError("no Xen Airlock recovery boot entry was generated")

    selected = next((entry for entry in normal if entry.path.stem == preferred), None)
    current = selected or normal[0]
    history = [entry for entry in normal if entry != current]
    current_recovery = recovery[0]
    if recovery_profile:
        recovery_profile.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [
                nix_env,
                "--profile",
                str(recovery_profile),
                "--set",
                str(current_recovery.system),
            ],
            check=True,
        )
    retained = {entry.path for entry in normal} | {current_recovery.path}
    for entry in entries_dir.glob("*.conf"):
        if entry.name.startswith(("nixos-", "xen-")) and entry not in retained:
            entry.unlink()

    replace_field(current.path, "title", "Airlock")
    replace_field(current.path, "sort-key", "00-airlock")
    replace_field(current_recovery.path, "title", "Airlock (recovery)")
    replace_field(current_recovery.path, "sort-key", "01-airlock-recovery")
    for entry in history:
        replace_field(entry.path, "title", f"Airlock (generation {entry.generation})")
        replace_field(entry.path, "sort-key", "02-airlock-generation")

    replace_field(loader_conf, "default", current.path.stem)
    if "preferred" in loader:
        replace_field(loader_conf, "preferred", current.path.stem)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--esp", type=Path, required=True)
    parser.add_argument("--store-root", type=Path, default=Path("/nix/store"))
    parser.add_argument("--recovery-profile", type=Path)
    parser.add_argument("--nix-env", default="nix-env")
    args = parser.parse_args()
    try:
        configure(
            args.esp,
            args.store_root,
            recovery_profile=args.recovery_profile,
            nix_env=args.nix_env,
        )
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
