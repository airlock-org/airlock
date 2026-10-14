#!/usr/bin/env python3
"""Use committed local machine source for Nix; check hosted pins before sharing."""

import argparse
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[4]
COMPONENT = "airlock-machine"


def git(repository, *args):
    result = subprocess.run(
        ["git", "-C", str(repository), *args], capture_output=True, text=True
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip() or "Git command failed")
    return result.stdout.strip()


def pinned_revision(repository):
    entry = git(repository, "ls-tree", "HEAD", "--", COMPONENT).split()
    if len(entry) != 4 or entry[0:2] != ["160000", "commit"]:
        raise RuntimeError("HEAD must contain the committed airlock-machine submodule pin")
    return entry[2]


def local_component(repository):
    revision = pinned_revision(repository)
    component = repository / COMPONENT
    instruction = "Run: git submodule update --init --recursive -- airlock-machine"
    if not (component / ".git").exists():
        raise RuntimeError(f"airlock-machine is not initialized. {instruction}")
    if Path(git(component, "rev-parse", "--show-toplevel")).resolve() != component:
        raise RuntimeError(f"airlock-machine is not a separate Git checkout. {instruction}")
    if git(component, "rev-parse", "HEAD") != revision:
        raise RuntimeError(
            "airlock-machine does not match the committed parent pin. "
            "Commit the updated parent pin if intentional; otherwise " + instruction
        )
    if git(repository, "diff", "--cached", "--name-only", "--", COMPONENT):
        raise RuntimeError("Commit the staged airlock-machine pin before building")
    if git(component, "status", "--porcelain"):
        raise RuntimeError("Commit or stash changes in airlock-machine before building")
    return component, revision


def component_url(repository):
    # Read committed metadata, not an uncommitted URL or a clone's local override.
    modules = git(repository, "show", "HEAD:.gitmodules")
    result = subprocess.run(
        ["git", "config", "--file", "-", "--get", "submodule.airlock-machine.url"],
        input=modules, capture_output=True, text=True,
    )
    if result.returncode or not result.stdout.strip():
        raise RuntimeError("The committed .gitmodules lacks airlock-machine's URL")
    return result.stdout.strip()


def fetch_urls(url):
    match = re.fullmatch(r"([^/:]+@[^/:]+):(.+)", url)
    if match:
        return [url, f"ssh://{match[1]}/{match[2]}"]
    return [url]


def local_flake_requested(repository, arguments):
    for argument in arguments:
        source = argument.split("#", 1)[0]
        if source.startswith("path:"):
            source = source[5:]
        elif source.startswith("git+file://"):
            source = source[11:].split("?", 1)[0]
        if source and not source.startswith("-"):
            path = Path(source).resolve()
            if path in (repository, repository / COMPONENT):
                return True
    return False


def local_fetch_environment(environment, url, source):
    environment = environment.copy()
    count = int(environment.get("GIT_CONFIG_COUNT", "0"))
    for original in fetch_urls(url):
        environment[f"GIT_CONFIG_KEY_{count}"] = f"url.{source.as_uri()}.insteadOf"
        environment[f"GIT_CONFIG_VALUE_{count}"] = original
        count += 1
    environment[f"GIT_CONFIG_KEY_{count}"] = "protocol.file.allow"
    environment[f"GIT_CONFIG_VALUE_{count}"] = "always"
    environment["GIT_CONFIG_COUNT"] = str(count + 1)
    return environment


def run_nix(repository, arguments):
    if not local_flake_requested(repository, arguments):
        return subprocess.call(["nix", *arguments])
    component, revision = local_component(repository)
    url = component_url(repository)
    with tempfile.TemporaryDirectory(prefix="airlock-machine-source-") as temporary:
        source = Path(temporary) / "machine.git"
        git(repository, "clone", "--quiet", "--bare", "--shared", str(component), str(source))
        # Nix fetches submodules through refs/heads/main. A disposable ref also
        # supports pins developed on branches or checked out at detached HEAD.
        git(source, "update-ref", "refs/heads/main", revision)
        git(source, "symbolic-ref", "HEAD", "refs/heads/main")
        environment = local_fetch_environment(os.environ, url, source)
        return subprocess.call(["nix", *arguments], env=environment)


def check_publish(repository):
    revision = pinned_revision(repository)
    url = component_url(repository)
    with tempfile.TemporaryDirectory(prefix="airlock-published-pin-") as temporary:
        source = Path(temporary)
        git(source, "init", "--quiet", "--bare")
        # Fetch the declared upstream in an isolated repository. Do not use the
        # development checkout's origin or its cached remote-tracking refs.
        git(source, "fetch", "--quiet", "--filter=blob:none", "--no-tags", url,
            "refs/heads/main:refs/heads/main")
        result = subprocess.run(
            ["git", "-C", str(source), "merge-base", "--is-ancestor", revision,
             "refs/heads/main"], capture_output=True, text=True,
        )
        if result.returncode:
            raise RuntimeError(
                f"Machine pin {revision} is unavailable from {url} main. "
                "Publish the machine commit on main before publishing the parent pin."
            )
    print(f"Machine pin {revision} is available from upstream main")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, default=ROOT)
    parser.add_argument("command", choices=("nix", "check-publish"))
    parser.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    try:
        repository = args.repository.resolve()
        if args.command == "check-publish":
            return check_publish(repository)
        arguments = args.arguments
        if arguments[:1] == ["--"]:
            arguments = arguments[1:]
        return run_nix(repository, arguments)
    except (RuntimeError, OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
