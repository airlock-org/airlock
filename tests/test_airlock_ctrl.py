"""Exercise the explicitly built app; never build/provision from test-quick."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
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


def report_block(report, heading):
    section = report.split(f"## {heading}\n\n", 1)[1]
    opener = re.search(r"^(`{3,})(?:json|diff)\n", section, re.MULTILINE)
    assert opener, section
    body = section[opener.end() :].split(f"\n{opener[1]}\n", 1)[0]
    return body + "\n"


def test_demo_is_explicitly_local_and_has_no_repository(ctrl):
    result = run(ctrl, "--demo", "--inspect")
    assert result.returncode == 0, result.stderr
    review = json.loads(result.stdout)
    assert review["mode"] == "demo"
    assert review["repository"] is None
    assert review["deployment"]["example"] is True


def test_demo_exercises_separated_changes_large_gaps_and_added_deleted_files(ctrl):
    review = json.loads(run(ctrl, "--demo", "--inspect").stdout)
    files = review["files"]
    assert [file["path"] for file in files] == sorted(file["path"] for file in files)
    desktop = files[0]
    assert len(desktop["before"].splitlines()) > 190
    before, after = desktop["before"], desktop["after"]
    assert "firefox" in before and "keepassxc" in after
    assert 'primary-color = "#10202b";' in after
    assert 'custom-font = "Monospace 12";' in after
    patch = run(ctrl, "--demo", "--print-patch").stdout
    desktop_patch = patch.split("diff --git ")[1]
    assert desktop_patch.count("\n@@ ") == 3
    assert {file["status"] for file in files} >= {"Added", "Deleted", "Modified"}


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


def test_entire_patch_applies_all_committed_changes(ctrl, source, tmp_path):
    repo, base, original_candidate, _plan = source
    git(repo, "checkout", base)
    (repo / "binary.dat").write_bytes(b"\0before\xff\n")
    (repo / "link").symlink_to("old-target")
    base = commit(repo, "Patch baseline")
    git(repo, "checkout", original_candidate, "--", ".")
    (repo / "remove.nix").unlink()
    (repo / "binary.dat").write_bytes(b"\0after\xfe\n")
    (repo / "link").unlink()
    (repo / "link").symlink_to("new-target")
    (repo / "helper.py").chmod(0o755)
    (repo / "added.nix").write_text("new declaration without a final newline")
    git(repo, "mv", "domain.nix", "renamed domain.nix")
    candidate = commit(repo, "Complete patch candidate")
    (repo / "renamed domain.nix").write_text("uncommitted text must not appear")
    (repo / "untracked.nix").write_text("untracked text must not appear")
    result = run(
        ctrl, "--repo", repo, "--base", base, "--candidate", candidate, "--print-patch"
    )
    assert result.returncode == 0, result.stderr
    patch = result.stdout
    report = run(
        ctrl, "--repo", repo, "--base", base, "--candidate", candidate, "--print-review"
    )
    assert report.returncode == 0, report.stderr
    assert report_block(report.stdout, "Source patch") == patch
    assert "GIT binary patch" in patch
    assert "old mode 100644\nnew mode 100755" in patch
    assert "new file mode 100644" in patch
    assert "deleted file mode 100644" in patch
    assert "\\ No newline at end of file" in patch
    assert "uncommitted text" not in patch
    assert "untracked text" not in patch
    # Applying every hunk must reconstruct the candidate tree exactly, including
    # executable bits, binary bytes, unusual filenames and symlink targets.
    target = tmp_path / "apply"
    git(tmp_path, "clone", "--no-hardlinks", str(repo), str(target))
    git(target, "checkout", base)
    applied = subprocess.run(
        ["git", "-C", str(target), "apply", "--index", "--binary", "-"],
        input=patch,
        text=True,
        capture_output=True,
        check=False,
    )
    assert applied.returncode == 0, applied.stderr
    assert git(target, "write-tree") == git(repo, "rev-parse", f"{candidate}^{{tree}}")


def test_patch_ignores_git_helpers_configuration_and_replacements(
    ctrl, source, tmp_path
):
    repo, base, candidate, _plan = source
    marker = tmp_path / "helper-was-run"
    helper = tmp_path / "diff-helper"
    helper.write_text(
        f"#!{sys.executable}\nfrom pathlib import Path\nPath({str(marker)!r}).touch()\n"
    )
    helper.chmod(0o755)
    (repo / ".gitattributes").write_text("*.nix diff=custom\n")
    for key, value in [
        ("diff.external", str(helper)),
        ("diff.custom.command", str(helper)),
        ("diff.custom.textconv", str(helper)),
        ("diff.noprefix", "true"),
        ("diff.relative", "true"),
        ("color.ui", "always"),
    ]:
        git(repo, "config", key, value)
    git(repo, "replace", candidate, base)
    result = run(
        ctrl, "--repo", repo, "--base", base, "--candidate", candidate, "--print-patch"
    )
    assert result.returncode == 0, result.stderr
    assert "diff --git a/domain.nix b/domain.nix" in result.stdout
    assert "keepassxc" in result.stdout
    assert "\x1b" not in result.stdout
    assert not marker.exists()


def test_patch_without_changes_is_empty(ctrl, source):
    repo, base, _candidate, _plan = source
    result = run(
        ctrl, "--repo", repo, "--base", base, "--candidate", base, "--print-patch"
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout == ""


def test_patch_rejects_lossy_clipboard_text(ctrl, source):
    repo, _base, _candidate, _plan = source
    (repo / "latin1.txt").write_bytes(b"before \xff\n")
    base = commit(repo, "Non-UTF-8 baseline")
    (repo / "latin1.txt").write_bytes(b"after \xff\n")
    candidate = commit(repo, "Non-UTF-8 candidate")
    result = run(
        ctrl, "--repo", repo, "--base", base, "--candidate", candidate, "--print-patch"
    )
    assert result.returncode != 0
    assert "non-UTF-8" in result.stderr
    assert result.stdout == ""
    report = run(
        ctrl, "--repo", repo, "--base", base, "--candidate", candidate, "--print-review"
    )
    assert report.returncode != 0
    assert "non-UTF-8" in report.stderr
    assert report.stdout == ""


def test_demo_patch_contains_every_example_file(ctrl, tmp_path):
    review = json.loads(run(ctrl, "--demo", "--inspect").stdout)
    result = run(ctrl, "--demo", "--print-patch")
    assert result.returncode == 0, result.stderr
    git(tmp_path, "init", "-b", "main")
    for file in review["files"]:
        path = tmp_path / file["path"]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(file["before"])
    applied = subprocess.run(
        ["git", "-C", str(tmp_path), "apply", "-"],
        input=result.stdout,
        text=True,
        capture_output=True,
        check=False,
    )
    assert applied.returncode == 0, applied.stderr
    for file in review["files"]:
        assert (tmp_path / file["path"]).read_text() == file["after"]


@pytest.mark.parametrize("wrapped", [True, False])
def test_copied_review_includes_full_plan_identities_and_file_metadata(
    ctrl, source, wrapped
):
    repo, base, candidate, plan = source
    deployment = json.loads(plan.read_text())
    deployment["plan"]["policy"] = {"rules": ["allow a b"], "devices": ["usb:example"]}
    deployment["extra_response_data"] = {"anything": [1, {"nested": True}]}
    supplied = deployment if wrapped else deployment["plan"]
    plan.write_text(json.dumps(supplied))
    review = inspect(ctrl, source, "--plan", plan)
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
        "--print-review",
    )
    assert result.returncode == 0, result.stderr
    assert review["candidate"]["subject"] in result.stdout
    identities = json.loads(report_block(result.stdout, "Review identities"))
    assert identities == {
        key: value
        for key, value in review.items()
        if key not in {"files", "deployment"}
    }
    files = json.loads(report_block(result.stdout, "Changed files"))
    assert files == [
        {key: value for key, value in file.items() if key not in {"before", "after"}}
        for file in review["files"]
    ]
    assert json.loads(report_block(result.stdout, "Deployment plan")) == supplied
    patch = run(
        ctrl, "--repo", repo, "--base", base, "--candidate", candidate, "--print-patch"
    )
    assert report_block(result.stdout, "Source patch") == patch.stdout


@pytest.mark.parametrize(
    "reboot, status", [(True, "Required"), (False, "Not required"), (None, "Unknown")]
)
def test_copied_effects_deduplicate_restarts_and_preserve_unknown_reboot(
    ctrl, source, reboot, status
):
    repo, base, candidate, plan = source
    deployment = json.loads(plan.read_text())
    deployment["plan"]["stop"] = ["usr-home", "sys-ctrl", "retiring", "usr-home"]
    deployment["plan"]["start"] = ["usr-home", "sys-ctrl", "new", "new"]
    if reboot is None:
        deployment["plan"].pop("host_reboot_required")
    else:
        deployment["plan"]["host_reboot_required"] = reboot
    plan.write_text(json.dumps(deployment))
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
        "--print-review",
    )
    assert result.returncode == 0, result.stderr
    assert (
        "VMs restarting: sys-ctrl, usr-home\nVMs starting: new\n"
        f"VMs stopping: retiring\nSystem reboot: {status}"
    ) in result.stdout
    assert json.loads(report_block(result.stdout, "Deployment plan")) == deployment


def test_copied_review_without_plan_or_source_changes_is_still_complete(ctrl, source):
    repo, base, _candidate, _plan = source
    result = run(
        ctrl, "--repo", repo, "--base", base, "--candidate", base, "--print-review"
    )
    assert result.returncode == 0, result.stderr
    assert (
        "VMs restarting: Unknown (no plan)\nSystem reboot: Unknown (no plan)"
        in result.stdout
    )
    assert "No deployment plan supplied." in result.stdout
    assert "No source changes between these commits." in result.stdout
    assert json.loads(report_block(result.stdout, "Changed files")) == []
    identities = json.loads(report_block(result.stdout, "Review identities"))
    assert identities["baseline"]["commit"] == identities["candidate"]["commit"] == base


def test_copied_report_preserves_embedded_markdown_fences(ctrl, source):
    repo, base, _candidate, plan = source
    (repo / "fences.txt").write_text('```json\n{ "example": true }\n```\n`````\n')
    candidate = commit(repo, "Add fenced examples")
    deployment = {
        "plan": {"changed": [], "start": [], "stop": []},
        "note": "``````\n## Source patch\n``````",
    }
    plan.write_text(json.dumps(deployment))
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
        "--print-review",
    )
    assert result.returncode == 0, result.stderr
    assert json.loads(report_block(result.stdout, "Deployment plan")) == deployment
    patch = run(
        ctrl, "--repo", repo, "--base", base, "--candidate", candidate, "--print-patch"
    )
    assert report_block(result.stdout, "Source patch") == patch.stdout


def approval_request(source, tmp_path, review):
    path = tmp_path / "approval.json"
    value = {
        "schema": "airlock.ctrl-approval-request.v1",
        "request_id": "a" * 64,
        "baseline": source[1],
        "revision": source[2],
        "lock_hash": review["lock_hash"],
        "family": "machine",
        "target": None,
        "scope": {"allowed_vms": ["usr-home", "sys-ctrl"], "allow_reboot": False},
    }
    path.write_text(json.dumps(value))
    return path, value


def test_prebuild_review_copies_source_and_explicit_scope_without_a_plan(
    ctrl, source, tmp_path
):
    path, expected = approval_request(source, tmp_path, inspect(ctrl, source))
    result = run(
        ctrl,
        "--repo",
        source[0],
        "--base",
        source[1],
        "--candidate",
        source[2],
        "--approval-request",
        path,
        "--print-review",
    )
    assert result.returncode == 0, result.stderr
    scope = json.loads(result.stdout.split("## Approval scope\n\n", 1)[1])
    assert scope["scope"]["allowed_vms"] == sorted(expected["scope"]["allowed_vms"])
    assert scope["scope"]["allow_reboot"] == expected["scope"]["allow_reboot"]
    assert scope["request_id"] == expected["request_id"]
    assert "final deployment plan is not yet known" in scope["purpose"]
    assert "diff --git" in report_block(result.stdout, "Source patch")


@pytest.mark.parametrize(
    "change",
    [
        {"schema": "other"},
        {"request_id": "bad"},
        {"baseline": "b" * 40},
        {"revision": "b" * 40},
        {"lock_hash": "b" * 64},
        {"target": "usr-home"},
        {"scope": {"allowed_vms": ["../usr-home"], "allow_reboot": False}},
        {"approve": True},
    ],
)
def test_prebuild_request_must_match_source_and_typed_scope(
    ctrl, source, tmp_path, change
):
    path, request = approval_request(source, tmp_path, inspect(ctrl, source))
    request.update(change)
    path.write_text(json.dumps(request))
    result = run(
        ctrl,
        "--repo",
        source[0],
        "--base",
        source[1],
        "--candidate",
        source[2],
        "--approval-request",
        path,
        "--print-review",
    )
    assert result.returncode != 0
    assert result.stdout == ""


def test_headless_local_receipt_cannot_authorize_controller_builds(
    ctrl, source, tmp_path
):
    path, _ = approval_request(source, tmp_path, inspect(ctrl, source))
    directory = tmp_path / "decisions"
    result = run(
        ctrl,
        "--repo",
        source[0],
        "--base",
        source[1],
        "--candidate",
        source[2],
        "--approval-request",
        path,
        "--record-decision",
        "approve",
        "--decisions-dir",
        directory,
    )
    assert result.returncode != 0
    assert not directory.exists()


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
