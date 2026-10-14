"""Exercise the explicitly built app; never build/provision from test-quick."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[1]


def test_preview_uses_standalone_app_flake(tmp_path):
    runner = tmp_path / "fake-nix"
    runner.write_text(
        f"#!{sys.executable}\nimport json, sys\nprint(json.dumps(sys.argv[1:]))\n"
    )
    runner.chmod(0o755)
    result = subprocess.run(
        ["make", "--no-print-directory", "-s", "ctrl-preview", f"AIRLOCK_NIX={runner}"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )
    assert json.loads(result.stdout) == [
        "run",
        "./apps#airlock-ctrl",
        "--no-write-lock-file",
        "--",
        "--demo",
    ]


@pytest.mark.nix
def test_clean_app_flake_does_not_fetch_an_unavailable_machine_submodule(tmp_path):
    repo = tmp_path / "checkout"
    repo.mkdir()
    shutil.copytree(
        ROOT / "apps",
        repo / "apps",
        ignore=shutil.ignore_patterns("target", "decisions"),
    )
    git(repo, "init", "-b", "main")
    git(repo, "config", "user.name", "Airlock test")
    git(repo, "config", "user.email", "test@example.invalid")
    git(repo, "config", "commit.gpgsign", "false")
    (repo / "flake.nix").write_text(
        '{ inputs.self.submodules = true; outputs = _: throw "root flake must not be loaded"; }\n'
    )
    (repo / ".gitmodules").write_text(
        '[submodule "airlock-machine"]\n'
        "\tpath = airlock-machine\n"
        f"\turl = file://{tmp_path}/unavailable-machine.git\n"
    )
    git(repo, "add", "--all")
    git(
        repo,
        "update-index",
        "--add",
        "--cacheinfo",
        f"160000,{'1' * 40},airlock-machine",
    )
    git(repo, "commit", "-m", "Clean apps with an unavailable machine pin")
    (repo / "airlock-machine").mkdir()
    assert git(repo, "status", "--porcelain") == ""
    result = subprocess.run(
        [
            str(ROOT / "lifecycle/misc/tools/scripts/airlock-nix.sh"),
            "eval",
            "--raw",
            "--no-write-lock-file",
            f"{repo}/apps#packages.x86_64-linux.airlock-ctrl.drvPath",
        ],
        text=True,
        capture_output=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.endswith("-airlock-ctrl-0.1.0.drv")


@pytest.fixture(scope="module")
def ctrl():
    configured = os.environ.get("AIRLOCK_CTRL_BIN")
    binary = Path(configured) if configured else ROOT / "apps/target/debug/airlock-ctrl"
    if not binary.is_file():
        if configured:
            pytest.fail(f"AIRLOCK_CTRL_BIN does not exist: {binary}")
        pytest.skip("build airlock-ctrl explicitly or set AIRLOCK_CTRL_BIN")
    return binary.resolve()


def run(ctrl, *args):
    return subprocess.run(
        [str(ctrl), *map(str, args)], capture_output=True, text=True, check=False
    )


def git(repo, *args):
    return subprocess.check_output(
        ["git", "-C", str(repo), *args], text=True, stderr=subprocess.PIPE
    ).strip()


def commit(repo, message):
    git(repo, "add", "--all")
    git(repo, "commit", "-m", message)
    return git(repo, "rev-parse", "HEAD")


@pytest.fixture
def source(tmp_path):
    repo = tmp_path / "source"
    repo.mkdir()
    git(repo, "init", "-b", "main")
    git(repo, "config", "user.name", "Airlock test")
    git(repo, "config", "user.email", "test@example.invalid")
    git(repo, "config", "commit.gpgsign", "false")
    (repo / "domain.nix").write_text('{ packages = [ "firefox" ]; }\n')
    (repo / "flake.lock").write_text('{"nodes": {}}\n')
    (repo / "remove.nix").write_text("old declaration\n")
    (repo / "helper.py").write_text("print('before')\n")
    (repo / "odd\tname\n.nix").write_text("before\n")
    base = commit(repo, "Baseline")
    (repo / "domain.nix").write_text('{ packages = [ "firefox" "keepassxc" ]; }\n')
    (repo / "remove.nix").unlink()
    (repo / "added.nix").write_text("new declaration\n")
    (repo / "helper.py").write_text("print('after')\n")
    (repo / "odd\tname\n.nix").write_text("after\n")
    candidate = commit(repo, "Candidate")
    plan = tmp_path / "plan.json"
    plan.write_text(
        json.dumps(
            {
                "revision": candidate,
                "plan": {
                    "revision": candidate,
                    "changed": ["usr-home"],
                    "stop": ["usr-home"],
                    "start": ["usr-home"],
                    "retire": [],
                    "restore": [],
                    "host_reboot_required": False,
                },
                "prepared": "a" * 64,
            }
        )
    )
    return repo, base, candidate, plan


def inspect(ctrl, source, *extra):
    repo, base, candidate, _plan = source
    result = run(
        ctrl,
        "--repo",
        repo,
        "--base",
        base,
        "--candidate",
        candidate,
        "--inspect",
        *extra,
    )
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)


def test_demo_is_explicitly_local_and_has_no_repository(ctrl):
    result = run(ctrl, "--demo", "--inspect")
    assert result.returncode == 0, result.stderr
    review = json.loads(result.stdout)
    assert review["mode"] == "demo"
    assert review["repository"] is None
    assert review["deployment"]["example"] is True


def test_review_uses_committed_blobs_and_includes_all_changed_files(ctrl, source):
    repo, base, candidate, plan = source
    (repo / "domain.nix").write_text("uncommitted and must not appear\n")
    (repo / "untracked.nix").write_text("untracked\n")
    review = inspect(ctrl, source, "--plan", plan)
    assert review["baseline"]["commit"] == base
    assert review["candidate"]["commit"] == candidate
    files = {file["path"]: file for file in review["files"]}
    assert set(files) == {
        "domain.nix",
        "added.nix",
        "remove.nix",
        "helper.py",
        "odd\tname\n.nix",
    }
    assert "keepassxc" in files["domain.nix"]["after"]
    assert files["added.nix"]["status"] == "Added"
    assert files["remove.nix"]["status"] == "Deleted"
    assert (
        review["lock_hash"]
        == hashlib.sha256((repo / "flake.lock").read_bytes()).hexdigest()
    )
    assert (repo / "domain.nix").read_text() == "uncommitted and must not appear\n"


def test_git_replace_cannot_change_the_reviewed_commit(ctrl, source):
    repo, base, candidate, _plan = source
    git(repo, "replace", candidate, base)
    review = inspect(ctrl, source)
    assert len(review["files"]) == 5
    assert review["candidate"]["subject"] == "Candidate"


def test_plan_changes_change_the_fingerprint(ctrl, source):
    plan = source[3]
    first = inspect(ctrl, source, "--plan", plan)
    value = json.loads(plan.read_text())
    value["plan"]["host_reboot_required"] = True
    plan.write_text(json.dumps(value))
    second = inspect(ctrl, source, "--plan", plan)
    assert first["fingerprint"] != second["fingerprint"]


@pytest.mark.parametrize("change", ["revision", "lock", "shape", "names"])
def test_invalid_or_mismatched_plan_is_rejected(ctrl, source, change):
    repo, base, candidate, plan = source
    value = json.loads(plan.read_text())
    if change == "revision":
        value["plan"]["revision"] = base
    elif change == "lock":
        value["lock_hash"] = "0" * 64
    elif change == "shape":
        value["plan"] = []
    else:
        value["plan"]["stop"] = [123]
    plan.write_text(json.dumps(value))
    result = run(
        ctrl,
        "--repo",
        repo,
        "--base",
        base,
        "--candidate",
        candidate,
        "--plan",
        plan,
        "--inspect",
    )
    assert result.returncode != 0


def test_local_receipt_binds_review_and_cannot_be_overwritten(ctrl, source, tmp_path):
    repo, base, candidate, plan = source
    directory = tmp_path / "decisions"
    args = [
        "--repo",
        repo,
        "--base",
        base,
        "--candidate",
        candidate,
        "--plan",
        plan,
        "--decisions-dir",
        directory,
    ]
    result = run(ctrl, *args, "--record-decision", "approve")
    assert result.returncode == 0, result.stderr
    path = Path(result.stdout.strip())
    receipt = json.loads(path.read_text())
    assert receipt["authority"] == "local-review-only"
    assert receipt["schema"] == "airlock.ctrl-local-decision.v1"
    assert receipt["candidate_revision"] == candidate
    assert (
        receipt["review_fingerprint"]
        == inspect(ctrl, source, "--plan", plan)["fingerprint"]
    )
    assert path.stat().st_mode & 0o777 == 0o600
    original = path.read_bytes()
    result = run(ctrl, *args, "--record-decision", "reject")
    assert result.returncode != 0
    assert path.read_bytes() == original
    assert list(directory.iterdir()) == [path]


def test_source_only_review_cannot_approve_but_can_reject(ctrl, source, tmp_path):
    repo, base, candidate, _plan = source
    args = [
        "--repo",
        repo,
        "--base",
        base,
        "--candidate",
        candidate,
        "--decisions-dir",
        tmp_path / "decisions",
    ]
    result = run(ctrl, *args, "--record-decision", "approve")
    assert result.returncode != 0
    assert "Load a saved deployment plan" in result.stderr
    result = run(ctrl, *args, "--record-decision", "reject")
    assert result.returncode == 0, result.stderr


@pytest.mark.parametrize(
    "content",
    [b"binary\x00content", b"bad\xffutf8", b"x" * (1024 * 1024 + 1)],
    ids=["binary", "non-utf8", "oversized"],
)
def test_undisplayable_source_is_listed_and_blocks_approval(
    ctrl, source, tmp_path, content
):
    repo, base, _candidate, plan = source
    (repo / "opaque.nix").write_bytes(content)
    candidate = commit(repo, "Opaque change")
    value = json.loads(plan.read_text())
    value["revision"] = value["plan"]["revision"] = candidate
    plan.write_text(json.dumps(value))
    updated = repo, base, candidate, plan
    review = inspect(ctrl, updated, "--plan", plan)
    file = next(file for file in review["files"] if file["path"] == "opaque.nix")
    assert file["opaque"]
    assert file["new_oid"]
    result = run(
        ctrl,
        "--repo",
        repo,
        "--base",
        base,
        "--candidate",
        candidate,
        "--plan",
        plan,
        "--decisions-dir",
        tmp_path / "decisions",
        "--record-decision",
        "approve",
    )
    assert result.returncode != 0
    assert "cannot be displayed" in result.stderr


def test_symlink_and_mode_changes_are_reviewed_without_following_links(
    ctrl, source, tmp_path
):
    repo, base, _candidate, _plan = source
    secret = tmp_path / "outside-repo"
    secret.write_text("must never be read\n")
    (repo / "link.nix").symlink_to(secret)
    (repo / "flake.lock").chmod(0o755)
    candidate = commit(repo, "Symlink and executable mode")
    review = inspect(ctrl, (repo, base, candidate, None))
    files = {file["path"]: file for file in review["files"]}
    assert files["link.nix"]["new_mode"] == "120000"
    assert files["link.nix"]["after"] == str(secret)
    assert files["flake.lock"]["old_mode"] == "100644"
    assert files["flake.lock"]["new_mode"] == "100755"
    assert files["flake.lock"]["additions"] == 0


def test_demo_cannot_write_receipts(ctrl, tmp_path):
    result = run(
        ctrl,
        "--demo",
        "--decisions-dir",
        tmp_path / "decisions",
        "--record-decision",
        "approve",
    )
    assert result.returncode != 0
    assert not (tmp_path / "decisions").exists()
