"""Development-host deployment transport and pinned release integration."""

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess

import pytest

ROOT = Path(__file__).resolve().parents[1]

client_spec = importlib.util.spec_from_file_location(
    "deploy_client", ROOT / "lifecycle/misc/tools/scripts/machine-deploy.py"
)
client = importlib.util.module_from_spec(client_spec)
client_spec.loader.exec_module(client)


@pytest.mark.parametrize('action', ['resume', 'rollback'])
def test_recovery_can_use_committed_coordinator_without_rebuilding_release(monkeypatch, action):
    calls = []
    class Session:
        def __init__(self, target):
            calls.append(('target', target))
        def run(self, script, **kwargs):
            calls.append(('run', script))
            return 'started'
        def close(self):
            pass
    def command(*args, **kwargs):
        if args[0] == 'git':
            return ''
        assert args[1:3] == ('build', './airlock-machine#airlock-machine-deploy')
        return json.dumps([{'outputs': {'out': '/nix/store/updated-coordinator'}}])
    monkeypatch.setattr(client, 'Dom0', Session)
    monkeypatch.setattr(client, 'command', command)
    def transfer(*args):
        assert args[1] == 'sys-agent'
        calls.append(('transfer', args[2]))
    monkeypatch.setattr(client, 'transfer', transfer)
    monkeypatch.setattr(client, 'build', lambda *a, **kw: pytest.fail('unexpected whole-machine build'))
    assert client.main([action, '--target', 'user@old-assistant', '--legacy-dom0',
                        '--bridge-vm', 'sys-agent', '--update-coordinator']) == 0
    assert ('transfer', '/nix/store/updated-coordinator') in calls
    script = next(value for kind, value in calls if kind == 'run')
    assert f'/nix/store/updated-coordinator/bin/airlock-machine-deploy {action}' in script


def test_coordinator_recovery_requires_explicit_development_transport():
    with pytest.raises(SystemExit) as error:
        client.main(['resume', '--target', 'user@assistant', '--update-coordinator'])
    assert error.value.code == 2


def test_legacy_apply_selects_explicit_administrative_repair(monkeypatch):
    scripts = []
    manifest = {"revision": "machine-revision", "profile": "usb-lab", "coordinator": "/nix/store/engine/bin/deploy"}
    class Session:
        def __init__(self, target):
            assert target == "user@assistant"
        def run(self, script, **kwargs):
            scripts.append(script)
            return json.dumps(manifest) if script.startswith("cat ") else '{}'
        def close(self):
            pass
    monkeypatch.setattr(client, "Dom0", Session)
    monkeypatch.setattr(client, "build", lambda *_: ("/nix/store/release", "machine-revision", "parent-revision"))
    monkeypatch.setattr(client, "transfer", lambda *_: None)
    monkeypatch.setattr("builtins.input", lambda _: "y")
    assert client.main(["apply", "--target", "user@assistant", "--legacy-dom0"]) == 0
    assert any("apply /nix/store/release --administrative" in script for script in scripts)


@pytest.mark.parametrize("legacy", [False, True])
def test_guest_repair_requires_explicit_administrative_transport(monkeypatch, legacy):
    scripts = []
    candidate = "/var/lib/airlock/machine/candidates/validated-guest"

    class Session:
        def __init__(self, target):
            assert target == "user@assistant"

        def run(self, script, **kwargs):
            scripts.append(script)
            return json.dumps({"candidate": candidate, "plan": {}})

        def close(self):
            pass

    monkeypatch.setattr(client, "Dom0", Session)
    monkeypatch.setattr(client, "deployed_engine", lambda _: "/nix/store/engine/bin/deploy")
    monkeypatch.setattr(client, "remote_receipt", lambda *_: {})
    monkeypatch.setattr(client, "install_receipt", lambda *_: "/receipt.json")
    monkeypatch.setattr("builtins.input", lambda _: "y")
    arguments = ["guest-apply", "--target", "user@assistant", "--receipt", "/remote.json"]
    if legacy:
        arguments.append("--legacy-dom0")
    assert client.main(arguments) == 0
    assert "guest-plan /receipt.json" in scripts[0]
    if legacy:
        assert f"apply {candidate} --administrative" in scripts[-1]
    else:
        assert "guest-apply /receipt.json" in scripts[-1]
        assert "--administrative" not in scripts[-1]


@pytest.mark.parametrize("arguments", [["apply", "--host-repair"],
                                       ["status", "--host-repair", "--legacy-dom0"]])
def test_host_repair_requires_explicit_development_plan_or_apply(arguments):
    with pytest.raises(SystemExit):
        client.main([*arguments, "--target", "user@assistant"])


def test_host_repair_uses_original_image_gitlink_instead_of_candidate_head(tmp_path, monkeypatch):
    root = tmp_path / "parent with spaces"
    root.mkdir()
    machine = root / "airlock-machine"
    machine.mkdir()
    def git(repo, *args):
        return subprocess.check_output(["git", "-C", str(repo),
            "-c", "user.name=Test", "-c", "user.email=test@example.invalid", *args], text=True).strip()
    git(root, "init", "-q")
    git(machine, "init", "-q")
    (machine / "flake.lock").write_text('{"original": true}\n')
    git(machine, "add", "flake.lock")
    git(machine, "commit", "-qm", "Original machine")
    baseline = git(machine, "rev-parse", "HEAD")
    git(root, "update-index", "--add", "--cacheinfo", "160000", baseline, "airlock-machine")
    git(root, "commit", "-qm", "Original image")
    image = git(root, "rev-parse", "HEAD")
    (machine / "flake.lock").write_text('{"candidate": true}\n')
    git(machine, "commit", "-qam", "Candidate")
    candidate = git(machine, "rev-parse", "HEAD")
    class Session:
        def run(self, _script):
            return json.dumps({"host": "/nix/store/original-host", "profile": "usb-lab", "devMode": True,
                               "boot": {"revision": image}, "manifest": None, "topology": {"domains": {}}})
    monkeypatch.setattr(client, "ROOT", root)
    snapshot = client.host_repair_snapshot(Session(), "usb-lab")
    assert snapshot["revision"] == baseline != candidate
    assert snapshot["lockHash"] == hashlib.sha256(b'{"original": true}\n').hexdigest()
    assert snapshot["repairRevision"] == candidate
    assert "%20" in snapshot["baselineFlake"]


def test_client_refuses_uncommitted_sources(monkeypatch):
    monkeypatch.setattr(
        client, "command", lambda *args, **kwargs: " M configuration.nix"
    )
    with pytest.raises(RuntimeError, match="commit repository changes"):
        client.build("usb-lab")


@pytest.mark.parametrize("assistant_rename", [False, True])
def test_client_builds_the_pinned_airlock_machine_checkout(monkeypatch, tmp_path, assistant_rename):
    monkeypatch.setattr(client, "ROOT", tmp_path)
    calls = []

    def command(*args, **kwargs):
        calls.append((args, kwargs))
        if args == ("git", "status", "--porcelain"):
            return ""
        if args == ("git", "rev-parse", "HEAD"):
            return "dev-revision"
        if args == ("git", "-C", "airlock-machine", "rev-parse", "HEAD"):
            return "machine-revision"
        installable = "./airlock-machine#airlock-machine-release-usb-lab"
        if assistant_rename:
            installable += "-assistant-rename"
        if args[1:3] == ("build", installable):
            return json.dumps([{"outputs": {"out": "/nix/store/release"}}])
        if args[1:] == (
            "build",
            "/nix/store/release",
            "--out-link",
            str(
                tmp_path
                / ".airlock-logs"
                / "machine-deploy"
                / "dev-revision"
                / "release"
            ),
        ):
            return ""
        raise AssertionError(f"unexpected command: {args}")

    monkeypatch.setattr(client, "command", command)

    assert client.build("usb-lab", assistant_rename=assistant_rename) == (
        "/nix/store/release",
        "machine-revision",
        "dev-revision",
    )
    assert all("os" not in args for args, _ in calls)


@pytest.mark.parametrize("arguments", [
    ["status", "--target", "old-ssh-target", "--assistant-rename", "--legacy-dom0"],
    ["build", "--profile", "usb-gui-lab", "--assistant-rename"],
])
def test_assistant_rename_rejects_incompatible_action_before_build(monkeypatch, arguments):
    monkeypatch.setattr(client, "build", lambda *_args, **_kwargs: pytest.fail("unexpected build"))
    with pytest.raises(SystemExit) as error:
        client.main(arguments)
    assert error.value.code == 2


@pytest.mark.parametrize("action", ["plan", "apply", "status", "verify", "resume", "rollback"])
def test_normal_commands_use_the_airlock_machine_namespace(monkeypatch, action):
    calls = []
    monkeypatch.delenv("AIRLOCK_DEPLOY_REMOTE_CLIENT", raising=False)
    monkeypatch.setattr(client, "Dom0", lambda *_: pytest.fail("unexpected development shell"))
    monkeypatch.setattr(client.subprocess, "run", lambda argv, **_kwargs: calls.append(argv))
    monkeypatch.setattr(client.subprocess, "call", lambda argv: calls.append(argv) or 0)
    assert client.main([action, "--target", "user@usr-assistant"]) == 0
    assert calls[-1] == ["ssh", "user@usr-assistant", "airlock", "machine", action]
    assert len(calls) == (2 if action in {"plan", "apply"} else 1)


def test_assistant_rename_build_is_explicit_and_does_not_open_a_remote_session(monkeypatch):
    calls = []
    def build(profile, **options):
        calls.append((profile, options))
        return "/nix/store/rename-release", "machine-revision", "parent-revision"
    monkeypatch.setattr(client, "build", build)
    monkeypatch.setattr(client, "Dom0", lambda *_: pytest.fail("unexpected remote session"))
    assert client.main(["build", "--assistant-rename"]) == 0
    assert calls == [("usb-lab", {"assistant_rename": True})]


@pytest.mark.parametrize("action", ["plan", "apply"])
def test_assistant_rename_defaults_to_incremental_controller_submission(monkeypatch, action):
    calls = []
    monkeypatch.setattr(client, "build", lambda *_a, **_k: pytest.fail("legacy build attempted"))
    monkeypatch.setattr(client, "Dom0", lambda *_a: pytest.fail("development shell attempted"))
    monkeypatch.delenv("AIRLOCK_DEPLOY_SOURCE_COMMAND", raising=False)
    def run(args, **options):
        calls.append((args, options))
    monkeypatch.setattr(client.subprocess, "run", run)
    monkeypatch.setattr(client.subprocess, "call", lambda args: calls.append(args) or 0)
    assert client.main([action, "--target", "user@sys-agent", "--assistant-rename"]) == 0
    assert calls[0][1]["env"]["AIRLOCK_DEPLOY_SOURCE_COMMAND"] == "airlock-sys-agent-machine-source"
    assert calls[1] == ["ssh", "user@sys-agent", "python3",
                       "/home/user/airlock-machine/lifecycle/runtime/tools/airlock.py",
                       "machine", action, "--assistant-rename"]


@pytest.mark.parametrize("action", ["plan", "apply"])
def test_controller_bootstrap_uses_existing_guest_client_and_old_importer(monkeypatch, action):
    calls = []
    monkeypatch.delenv("AIRLOCK_DEPLOY_SOURCE_COMMAND", raising=False)
    monkeypatch.setattr(client, "build", lambda *_a, **_k: pytest.fail("local build attempted"))
    monkeypatch.setattr(client.subprocess, "run", lambda args, **options: calls.append((args, options)))
    monkeypatch.setattr(client.subprocess, "call", lambda args: calls.append(args) or 0)
    assert client.main([action, "--target", "user@sys-agent", "--controller-upgrade"]) == 0
    assert calls[0][1]["env"]["AIRLOCK_DEPLOY_SOURCE_COMMAND"] == "airlock-sys-agent-machine-source"
    assert calls[1] == ["ssh", "user@sys-agent", "airlock-sys-agent-machine-deploy",
                       "guest-" + action, "--target", "sys-ctrl"]


@pytest.mark.parametrize("arguments", [
    ["status", "--controller-upgrade"],
    ["plan", "--controller-upgrade", "--assistant-rename"],
    ["plan", "--controller-upgrade", "--legacy-dom0"],
])
def test_controller_bootstrap_rejects_other_actions_before_submission(monkeypatch, arguments):
    monkeypatch.setattr(client.subprocess, "run", lambda *_a, **_k: pytest.fail("source import attempted"))
    with pytest.raises(SystemExit) as error:
        client.main([*arguments, "--target", "user@sys-agent"])
    assert error.value.code == 2


def test_low_space_stops_transfer_before_any_write(monkeypatch):
    paths = {"/nix/store/new": {"narSize": 1024**3}}
    monkeypatch.setattr(client, "command", lambda *args: json.dumps(paths))
    calls = []

    class Session:
        def run(self, script, **kwargs):
            calls.append(script)
            return "" if script == client.REMOTE_INVENTORY else str(1024**3)

    with pytest.raises(RuntimeError, match="No VMs or partitions changed"):
        client.transfer("usr-assistant-2", "usr-assistant", "/nix/store/release", Session())
    assert len(calls) == 2


def test_existing_closure_requires_no_upload(monkeypatch):
    monkeypatch.setattr(
        client,
        "command",
        lambda *args: json.dumps({"/nix/store/present": {"narSize": 100}}),
    )

    class Session:
        def run(self, script, **kwargs):
            assert script == client.REMOTE_INVENTORY
            return "/nix/store/present"

    client.transfer("usr-assistant-2", "usr-assistant", "/nix/store/present", Session())


def test_guest_receipt_is_read_from_usr_assistant_and_installed_for_dom0(monkeypatch):
    receipt = {
        "schema": "airlock.closure-store.admission.v1",
        "domain": "usr-home",
        "generation": 2,
    }
    monkeypatch.setattr(client, "command", lambda *args: json.dumps(receipt))
    assert (
        client.remote_receipt("usr-assistant", "/home/user/.local/share/airlock/receipt.json")
        == receipt
    )

    class Session:
        def __init__(self):
            self.scripts = []

        def run(self, script, **kwargs):
            self.scripts.append(script)
            return ""

    session = Session()
    destination = client.install_receipt(session, receipt)
    expected = hashlib.sha256(
        (json.dumps(receipt, sort_keys=True, indent=2) + "\n").encode()
    ).hexdigest()
    assert destination == f"/var/lib/airlock/machine/receipts/{expected}.json"
    assert len(session.scripts) == 1
    assert destination in session.scripts[0]
    assert "usr-home" not in session.scripts[0]


def test_large_receipt_survives_the_real_control_line_limit(tmp_path):
    transport = client.Dom0.__new__(client.Dom0)
    transport.process = subprocess.Popen(
        ["bash", "--noprofile", "--norc"], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
    )
    transport.buffer = b""
    local_directory = tmp_path / "receipts"

    class Session:
        def run(self, script):
            return transport.run(script.replace(
                "/var/lib/airlock/machine/receipts", str(local_directory),
            ))

    receipt = {"schema": "admission", "timings": {"details": "payload-'`$()" * 1000}}
    try:
        destination = client.install_receipt(Session(), receipt)
        actual = local_directory / Path(destination).name
        assert json.loads(actual.read_text()) == receipt
        assert actual.stat().st_mode & 0o777 == 0o600
        assert list(local_directory.iterdir()) == [actual]
    finally:
        transport.close()


@pytest.mark.parametrize(
    "report,error",
    [
        ("262144 4096", "private-volume space"),
        ("4080335 4096", "reached export"),
        ("-1 4096", "invalid usr-assistant"),
        ("100 0", "invalid usr-assistant"),
    ],
)
def test_bridge_space_check_does_not_require_python(monkeypatch, report, error):
    calls = []

    def command(*args):
        calls.append(args)
        if args[0] == "ssh":
            assert args == ("ssh", "usr-assistant", "stat -f -c '%a %S' /home/user")
            return report
        return json.dumps({"/nix/store/new": {"narSize": 1024**3}})

    class Session:
        def run(self, script, **kwargs):
            return "" if script == client.REMOTE_INVENTORY else str(30 * 1024**3)

    def stop_at_export(*args, **kwargs):
        raise RuntimeError("reached export")

    monkeypatch.setattr(client, "command", command)
    monkeypatch.setattr(client.subprocess, "Popen", stop_at_export)
    with pytest.raises(RuntimeError, match=error):
        client.transfer("usr-assistant", "usr-assistant", "/nix/store/release", Session())
    assert len(calls) == 2


def test_guest_plan_can_transfer_committed_coordinator(monkeypatch):
    events = []

    class Session:
        def __init__(self, target):
            assert target == "usr-assistant"

        def run(self, script, **kwargs):
            events.append(script)
            return "{}"

        def close(self):
            pass

    def command(*args, **kwargs):
        if args[0] == "git":
            return ""
        return json.dumps([{"outputs": {"out": "/nix/store/fixed-coordinator"}}])

    monkeypatch.setattr(client, "Dom0", Session)
    monkeypatch.setattr(client, "command", command)
    monkeypatch.setattr(client, "deployed_engine", lambda session: "/old-engine")
    monkeypatch.setattr(client, "remote_receipt", lambda *args: {})
    monkeypatch.setattr(client, "install_receipt", lambda *args: "/receipt.json")
    monkeypatch.setattr(
        client,
        "transfer",
        lambda target, bridge, package, session: events.append(package),
    )
    assert (
        client.main(
            [
                "guest-plan",
                "--target",
                "usr-assistant",
                "--receipt",
                "/receipt.json",
                "--update-coordinator",
            ]
        )
        == 0
    )
    assert events == [
        "/nix/store/fixed-coordinator",
        "/nix/store/fixed-coordinator/bin/airlock-machine-deploy guest-plan /receipt.json",
    ]


@pytest.mark.parametrize("controller", ["absent", "present", "broken", "failed"])
def test_client_status_before_and_after_first_deployment(
    monkeypatch, tmp_path, capsys, controller
):
    image_engine = tmp_path / "image-engine"
    image_engine.write_text('#!/bin/sh\necho "image-baseline"\n')
    image_engine.chmod(0o755)
    selected = tmp_path / "controller"
    if controller in ("present", "failed"):
        selected.write_text(
            '#!/bin/sh\necho "selected-release"\n'
            + ("exit 1\n" if controller == "failed" else "")
        )
        selected.chmod(0o755)
    elif controller == "broken":
        selected.symlink_to(tmp_path / "missing")

    class Session:
        def __init__(self, target):
            assert target == "usr-assistant"

        def run(self, script, **kwargs):
            script = script.replace(
                "/var/lib/airlock/machine/controller", str(selected)
            ).replace(
                "/run/current-system/sw/bin/airlock-machine-deploy", str(image_engine)
            )
            return subprocess.check_output(["bash", "-e", "-c", script], text=True)

        def close(self):
            pass

    monkeypatch.setattr(client, "Dom0", Session)
    result = client.main(["status", "--target", "usr-assistant", "--legacy-dom0"])
    output = capsys.readouterr().out
    if controller == "absent":
        assert result == 0 and "image-baseline" in output
    elif controller == "present":
        assert result == 0 and "selected-release" in output
    else:
        assert result == 1 and "image-baseline" not in output
