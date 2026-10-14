from __future__ import annotations

import configparser
import importlib.util
import json
import os
from pathlib import Path
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "repository_source", ROOT / "lifecycle/misc/tools/scripts/repository-source.py"
)
source = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(source)


@pytest.fixture
def source_checkout(tmp_path):
    repository = tmp_path / "parent with spaces"
    upstream = tmp_path / "upstream.git"
    machine = tmp_path / "machine"
    repository.mkdir()
    machine.mkdir()
    for checkout in (repository, machine):
        source.git(checkout, "init", "--quiet", "--initial-branch=main")
        source.git(checkout, "config", "user.name", "Test")
        source.git(checkout, "config", "user.email", "test@example.invalid")
        (checkout / "README").write_text("initial\n")
        source.git(checkout, "add", "README")
        source.git(checkout, "commit", "--quiet", "-m", "Initial")
    source.git(machine, "clone", "--quiet", "--bare", str(machine), str(upstream))
    source.git(repository, "-c", "protocol.file.allow=always", "submodule", "add",
               "--quiet", upstream.as_uri(), "airlock-machine")
    source.git(repository, "commit", "--quiet", "-am", "Pin machine")
    return repository, upstream


def commit_machine(repository):
    machine = repository / "airlock-machine"
    source.git(machine, "config", "user.name", "Test")
    source.git(machine, "config", "user.email", "test@example.invalid")
    source.git(machine, "switch", "--quiet", "-c", "feature")
    (machine / "README").write_text("unpublished feature\n")
    source.git(machine, "commit", "--quiet", "-am", "Feature")
    source.git(repository, "commit", "--quiet", "-am", "Pin feature")
    return source.pinned_revision(repository)


def test_local_nix_fetches_exact_unpublished_feature_pin(source_checkout, tmp_path, monkeypatch):
    repository, upstream = source_checkout
    revision = commit_machine(repository)
    machine = repository / "airlock-machine"
    source.git(machine, "switch", "--quiet", "--detach", revision)
    original_refs = source.git(machine, "show-ref")
    snapshot = tmp_path / "fetched.git"
    report = tmp_path / "nix.json"
    binary = tmp_path / "bin"
    binary.mkdir()
    nix = binary / "nix"
    nix.write_text(
        "#!/usr/bin/env python3\n"
        "import json, os, subprocess, sys\n"
        "from pathlib import Path\n"
        f"subprocess.run(['git', 'init', '--quiet', '--bare', {str(snapshot)!r}], check=True)\n"
        f"subprocess.run(['git', '-C', {str(snapshot)!r}, 'fetch', '--quiet', "
        f"{upstream.as_uri()!r}, 'refs/heads/main'], check=True)\n"
        f"revision = subprocess.check_output(['git', '-C', {str(snapshot)!r}, "
        "'rev-parse', 'FETCH_HEAD'], text=True).strip()\n"
        "count = int(os.environ['GIT_CONFIG_COUNT'])\n"
        "keys = {os.environ['GIT_CONFIG_KEY_' + str(i)]: "
        "os.environ['GIT_CONFIG_VALUE_' + str(i)] for i in range(count)}\n"
        f"Path({str(report)!r}).write_text(json.dumps(dict(revision=revision, "
        "args=sys.argv[1:], keys=keys)))\n"
        "sys.exit(7)\n"
    )
    nix.chmod(0o755)
    monkeypatch.chdir(repository)
    monkeypatch.setenv("PATH", f"{binary}:{os.environ['PATH']}")
    monkeypatch.setenv("GIT_CONFIG_COUNT", "1")
    monkeypatch.setenv("GIT_CONFIG_KEY_0", "example.setting")
    monkeypatch.setenv("GIT_CONFIG_VALUE_0", "preserved")
    assert source.run_nix(repository, ["build", ".#image"]) == 7
    result = json.loads(report.read_text())
    assert result["revision"] == revision
    assert result["args"] == ["build", ".#image"]
    assert result["keys"]["example.setting"] == "preserved"
    urls = [key[4:-10] for key in result["keys"] if key.endswith(".insteadOf")]
    from urllib.parse import unquote, urlsplit
    assert all(not Path(unquote(urlsplit(url).path)).exists() for url in urls)
    assert source.git(machine, "show-ref") == original_refs
    assert source.git(repository, "status", "--porcelain") == ""
    assert source.git(upstream, "rev-parse", "main") != revision


@pytest.mark.parametrize("problem", ["missing", "mismatch", "dirty", "staged"])
def test_local_build_errors_before_nix_for_invalid_machine(source_checkout, problem):
    repository, _ = source_checkout
    machine = repository / "airlock-machine"
    if problem == "missing":
        source.git(repository, "submodule", "deinit", "--force", "airlock-machine")
        message = "not initialized"
    elif problem == "mismatch":
        commit_machine(repository)
        source.git(repository, "reset", "--quiet", "HEAD~")
        message = "does not match"
    elif problem == "staged":
        commit_machine(repository)
        source.git(repository, "reset", "--soft", "HEAD~")
        source.git(machine, "switch", "--quiet", "--detach", source.pinned_revision(repository))
        message = "Commit the staged"
    else:
        (machine / "README").write_text("dirty\n")
        message = "Commit or stash"
    with pytest.raises(RuntimeError, match=message):
        source.run_nix(repository, ["eval", str(repository) + "#image"])


def test_publishing_checks_actual_main_not_cached_or_other_branches(source_checkout):
    repository, upstream = source_checkout
    assert source.check_publish(repository) == 0
    revision = commit_machine(repository)
    machine = repository / "airlock-machine"
    # A cached origin/main is insufficient proof, and a feature branch is not
    # reachable by Nix's default submodule main fetch.
    source.git(machine, "update-ref", "refs/remotes/origin/main", revision)
    source.git(machine, "push", "--quiet", str(upstream), "HEAD:refs/heads/feature")
    with pytest.raises(RuntimeError, match="Publish the machine commit on main"):
        source.check_publish(repository)
    source.git(machine, "push", "--quiet", str(upstream), "HEAD:refs/heads/main")
    assert source.check_publish(repository) == 0


def test_unrelated_nix_commands_do_not_require_a_machine(tmp_path, monkeypatch):
    binary = tmp_path / "bin"
    binary.mkdir()
    nix = binary / "nix"
    nix.write_text("#!/bin/sh\nexit 9\n")
    nix.chmod(0o755)
    monkeypatch.setenv("PATH", f"{binary}:{os.environ['PATH']}")
    monkeypatch.chdir(binary)
    for arguments in (["--version"], ["store", "gc"], ["shell", "nixpkgs#git"]):
        assert source.run_nix(tmp_path, arguments) == 9


def test_ssh_submodule_urls_cover_nix_and_git_transports():
    assert source.fetch_urls("git@example.invalid:org/machine.git") == [
        "git@example.invalid:org/machine.git",
        "ssh://git@example.invalid/org/machine.git",
    ]


@pytest.mark.parametrize(
    "name",
    [
        "setup-config.sh",
        "start-network-providers.sh",
        "import-config-inputs.sh",
        "nixos-config-vms-smoke.sh",
        "nix-closure-domain-smoke.sh",
        "boot-framebuffer.py",
        "boot-menu.py",
    ],
)
def test_runtime_scripts_have_one_machine_owned_source(name):
    relative = Path("lifecycle/misc/tools/scripts") / name
    assert (ROOT / "airlock-machine" / relative).is_file()
    assert not (ROOT / relative).exists()


def test_component_test_modules_are_not_copied_into_parent():
    parent = {path.name for path in (ROOT / "tests").glob("test_*.py")}
    machine = {
        path.name for path in (ROOT / "airlock-machine/tests").glob("test_*.py")
    }
    assert not parent & machine, "run canonical machine tests instead of copying them"


def test_machine_is_a_real_pinned_submodule():
    entry = subprocess.check_output(
        ["git", "ls-files", "--stage", "airlock-machine"], cwd=ROOT, text=True
    ).split()
    assert entry[0] == "160000"
    assert (
        entry[1]
        == subprocess.check_output(
            ["git", "-C", "airlock-machine", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip()
    )

    modules = configparser.ConfigParser()
    modules.read(ROOT / ".gitmodules")
    assert modules['submodule "airlock-machine"']["path"] == "airlock-machine"
    assert (
        modules['submodule "airlock-machine"']["url"]
        == "git@github.com:airlock-org/machine.git"
    )


def test_release_and_usb_assembly_have_separate_dependency_edges():
    machine_release = (
        ROOT / "airlock-machine/lifecycle/misc/modules/machine-release.nix"
    ).read_text()
    usb_adapter = (ROOT / "lifecycle/image/profiles/usb-prepared-store.nix").read_text()
    dev_flake = (ROOT / "flake.nix").read_text()

    assert "prepared" not in machine_release.lower()
    assert "airlockMachineRelease" not in usb_adapter
    assert "airlock-machine.lib.mkMachineConfiguration" in dev_flake
    assert "preparedStore.descriptors" in dev_flake


def test_both_nix_wrappers_select_the_external_airlock_store(tmp_path):
    data_root = tmp_path / "external data"
    env = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith("AIRLOCK_")
        and key not in {"NIX_CONFIG", "TMPDIR", "XDG_CACHE_HOME"}
    }
    env["AIRLOCK_DATA_ROOT"] = str(data_root)
    for repository in (ROOT, ROOT / "airlock-machine"):
        output = subprocess.check_output(
            [
                "bash",
                "-c",
                'source "$1" && printf "%s\n%s\n" "$AIRLOCK_NIX_ROOT" "$NIX_CONFIG"',
                "bash",
                str(repository / "lifecycle/misc/tools/scripts/lib/airlock-env.sh"),
            ],
            env=env,
            text=True,
        ).splitlines()
        assert output == [
            str(data_root / "nix-root"),
            f"store = {data_root / 'nix-root'}",
        ]
