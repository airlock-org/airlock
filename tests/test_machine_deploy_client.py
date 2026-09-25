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


def test_client_refuses_uncommitted_sources(monkeypatch):
    monkeypatch.setattr(
        client, "command", lambda *args, **kwargs: " M configuration.nix"
    )
    with pytest.raises(RuntimeError, match="commit repository changes"):
        client.build("usb-lab")


def test_client_builds_the_pinned_airlock_machine_checkout(monkeypatch, tmp_path):
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
        if args[1:3] == ("build", "./airlock-machine#airlock-machine-release-usb-lab"):
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

    assert client.build("usb-lab") == (
        "/nix/store/release",
        "machine-revision",
        "dev-revision",
    )
    assert all("os" not in args for args, _ in calls)


def test_low_space_stops_transfer_before_any_write(monkeypatch):
    paths = {"/nix/store/new": {"narSize": 1024**3}}
    monkeypatch.setattr(client, "command", lambda *args: json.dumps(paths))
    calls = []

    class Session:
        def run(self, script, **kwargs):
            calls.append(script)
            return "" if script == client.REMOTE_INVENTORY else str(1024**3)

    with pytest.raises(RuntimeError, match="No VMs or partitions changed"):
        client.transfer("sys-dev-2", "sys-dev", "/nix/store/release", Session())
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

    client.transfer("sys-dev-2", "sys-dev", "/nix/store/present", Session())


def test_guest_receipt_is_read_from_sys_dev_and_installed_for_dom0(monkeypatch):
    receipt = {
        "schema": "airlock.closure-store.admission.v1",
        "domain": "usr-home",
        "generation": 2,
    }
    monkeypatch.setattr(client, "command", lambda *args: json.dumps(receipt))
    assert (
        client.remote_receipt("sys-dev", "/home/user/.local/share/airlock/receipt.json")
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


@pytest.mark.parametrize(
    "report,error",
    [
        ("262144 4096", "private-volume space"),
        ("4080335 4096", "reached export"),
        ("-1 4096", "invalid sys-dev"),
        ("100 0", "invalid sys-dev"),
    ],
)
def test_bridge_space_check_does_not_require_python(monkeypatch, report, error):
    calls = []

    def command(*args):
        calls.append(args)
        if args[0] == "ssh":
            assert args == ("ssh", "sys-dev", "stat -f -c '%a %S' /home/user")
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
        client.transfer("sys-dev", "sys-dev", "/nix/store/release", Session())
    assert len(calls) == 2


def test_guest_plan_can_transfer_committed_coordinator(monkeypatch):
    events = []

    class Session:
        def __init__(self, target):
            assert target == "sys-dev"

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
                "sys-dev",
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
            assert target == "sys-dev"

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
    result = client.main(["status", "--target", "sys-dev", "--legacy-dom0"])
    output = capsys.readouterr().out
    if controller == "absent":
        assert result == 0 and "image-baseline" in output
    elif controller == "present":
        assert result == 0 and "selected-release" in output
    else:
        assert result == 1 and "image-baseline" not in output
